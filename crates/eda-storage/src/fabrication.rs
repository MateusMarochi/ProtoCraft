//! Exportação de fabricação por ferramenta externa explícita, com DRC e timeout.
use eda_core::Project;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
mod jlcpcb;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManufacturingPreset {
    Standard,
    #[default]
    Jlcpcb,
}
impl ManufacturingPreset {
    pub fn id(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Jlcpcb => "jlcpcb",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => "Padrão KiCad",
            Self::Jlcpcb => "JLCPCB — 2 camadas",
        }
    }
    pub fn archive_name(self) -> Option<&'static str> {
        match self {
            Self::Standard => None,
            Self::Jlcpcb => Some("JLCPCB.zip"),
        }
    }
}

fn run(tool: &Path, args: &[&std::ffi::OsStr], log: &Path) -> Result<(), String> {
    let file = fs::File::create(log).map_err(|e| e.to_string())?;
    let mut command = Command::new(tool);
    command
        .args(args)
        .stdout(Stdio::from(file.try_clone().map_err(|e| e.to_string())?))
        .stderr(Stdio::from(file));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Falha ao iniciar KiCad CLI: {e}"))?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            if status.success() {
                return Ok(());
            }
            return Err(format!(
                "KiCad CLI retornou {}: {}",
                status,
                fs::read_to_string(log)
                    .unwrap_or_default()
                    .chars()
                    .take(2000)
                    .collect::<String>()
            ));
        }
        if started.elapsed() > Duration::from_secs(60) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("KiCad CLI excedeu 60 segundos; execução encerrada.".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
pub fn generate(project: &Project, destination: &Path, tool: &Path) -> Result<PathBuf, String> {
    generate_with_preset(project, destination, tool, ManufacturingPreset::Standard)
}
pub fn generate_with_preset(
    project: &Project,
    destination: &Path,
    tool: &Path,
    preset: ManufacturingPreset,
) -> Result<PathBuf, String> {
    if destination.exists() {
        return Err("Escolha uma pasta de saída nova; arquivos existentes são preservados.".into());
    }
    if !tool.is_file() {
        return Err("Informe o caminho de kicad-cli.exe, instalado ou portátil.".into());
    }
    let report = eda_pcb::check(project)?;
    if report.errors() != 0 {
        return Err(format!(
            "Fabricação bloqueada: {} erros DRC e {} conexões pendentes. Corrija a PCB antes de gerar Gerber.",
            report.errors(),
            report.airwires.len()
        ));
    }
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let stage = tempfile::tempdir_in(parent).map_err(|e| e.to_string())?;
    let board = stage.path().join("projeto.kicad_pcb");
    fs::write(&board, eda_pcb::export_kicad(project)?).map_err(|e| e.to_string())?;
    fs::write(
        stage.path().join("drc-protocraft.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        stage.path().join("pin-map.json"),
        serde_json::to_vec_pretty(&eda_pcb::pin_map(project)?).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let drc = stage.path().join("drc-kicad.json");
    let log = stage.path().join("kicad-drc.log");
    let result = run(
        tool,
        &[
            "pcb".as_ref(),
            "drc".as_ref(),
            "--format".as_ref(),
            "json".as_ref(),
            "--severity-error".as_ref(),
            "--exit-code-violations".as_ref(),
            "-o".as_ref(),
            drc.as_os_str(),
            board.as_os_str(),
        ],
        &log,
    );
    if let Err(e) = result {
        let path = stage.keep();
        fs::rename(&path, destination)
            .map_err(|err| format!("{e}; falha ao preservar diagnóstico: {err}"))?;
        return Err(format!(
            "{e}. Diagnósticos preservados em {}.",
            destination.display()
        ));
    }
    let output = stage.path().to_path_buf();
    let log = stage.path().join("kicad-gerber.log");
    let mut gerber_args: Vec<&std::ffi::OsStr> = vec![
        "pcb".as_ref(),
        "export".as_ref(),
        "gerbers".as_ref(),
        "--layers".as_ref(),
        "F.Cu,B.Cu,F.Mask,B.Mask,F.Paste,B.Paste,F.SilkS,B.SilkS,Edge.Cuts".as_ref(),
        "-o".as_ref(),
        output.as_os_str(),
    ];
    if preset == ManufacturingPreset::Jlcpcb {
        // Sem --board-plot-params: mantém escala 1:1, origem absoluta e Protel.
        gerber_args.extend(jlcpcb::GERBER_OPTIONS.iter().map(std::ffi::OsStr::new));
    }
    gerber_args.push(board.as_os_str());
    run(tool, &gerber_args, &log)?;
    let log = stage.path().join("kicad-drill.log");
    let mut drill_args: Vec<&std::ffi::OsStr> = vec![
        "pcb".as_ref(),
        "export".as_ref(),
        "drill".as_ref(),
        "-o".as_ref(),
        output.as_os_str(),
    ];
    if preset == ManufacturingPreset::Jlcpcb {
        drill_args.extend(jlcpcb::DRILL_OPTIONS.iter().map(std::ffi::OsStr::new));
    }
    drill_args.push(board.as_os_str());
    run(tool, &drill_args, &log)?;
    let files: Vec<_> = fs::read_dir(stage.path())
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    let gerbers_complete = files.iter().filter(|f| f.ends_with(".gbr")).count() >= 9
        || [
            ".gtl", ".gbl", ".gtp", ".gbp", ".gto", ".gbo", ".gts", ".gbs", ".gm1",
        ]
        .iter()
        .all(|ext| files.iter().any(|f| f.ends_with(ext)));
    if !gerbers_complete || !files.iter().any(|f| f.ends_with(".drl")) {
        return Err("KiCad não produziu Gerbers e Excellon esperados.".into());
    }
    if preset == ManufacturingPreset::Jlcpcb {
        jlcpcb::package(stage.path(), project)?;
    }
    fs::write(stage.path().join("LEIA-ME.txt"),"Gerado pelo ProtoCraft com KiCad CLI externo.\nDRC nativo e DRC externo sem erros de severidade error.\nConfira avisos, empilhamento, máscara, pasta, furos, antena e regras do fabricante antes de produzir a placa.\nNão há aprovação automática de engenharia ou teste físico.\n").map_err(|e|e.to_string())?;
    let stage = stage.keep();
    fs::rename(stage, destination).map_err(|e| e.to_string())?;
    Ok(destination.to_path_buf())
}
