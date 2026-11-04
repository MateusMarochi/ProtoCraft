//! Conversão explícita entre o formato nativo, subconjuntos KiCad e a importação Proteus.
use eda_import::interchange::{InterchangeIssue, InterchangeReport};
use serde::Serialize;
use std::{
    error::Error,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

const MAX_SNIFF_BYTES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Protocraft,
    Proteus,
    KicadPcb,
    KicadSchematic,
}

impl Format {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "protocraft" => Some(Self::Protocraft),
            "proteus" => Some(Self::Proteus),
            "kicad-pcb" => Some(Self::KicadPcb),
            "kicad-sch" => Some(Self::KicadSchematic),
            _ => None,
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::Protocraft => "protocraft",
            Self::Proteus => "proteus_isis",
            Self::KicadPcb => "kicad_pcb",
            Self::KicadSchematic => "kicad_schematic",
        }
    }
}

#[derive(Debug, Serialize)]
struct ConversionIssue {
    code: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct ConversionReport {
    input: String,
    source_format: String,
    output: String,
    target_format: String,
    scope: String,
    lossless: bool,
    whole_project_preserved: bool,
    artifact_publication: &'static str,
    output_written: bool,
    losses: Vec<ConversionIssue>,
    warnings: Vec<String>,
}

#[derive(Debug)]
struct LoadedSource {
    project: eda_core::Project,
    format: Format,
    scope: &'static str,
    whole_project_preserved_on_import: bool,
    report: InterchangeReport,
}

fn tr(key: &str) -> String {
    eda_i18n::text(key)
}

fn issue(code: &str, message: &str) -> ConversionIssue {
    ConversionIssue {
        code: code.to_owned(),
        message: tr(message),
    }
}

pub fn handles(command: &str) -> bool {
    matches!(command, "conversion-formats" | "convert")
}

pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    match args.first().map(String::as_str) {
        Some("conversion-formats") => {
            if args.len() != 1 {
                return Err(tr("Uso: protocraft-cli conversion-formats").into());
            }
            println!("{}", serde_json::to_string_pretty(&format_matrix())?);
            Ok(())
        }
        Some("convert") => run_convert(&args[1..]),
        _ => Err(tr("Comando de conversão desconhecido.").into()),
    }
}

fn format_matrix() -> serde_json::Value {
    serde_json::json!({
        "formats": [
            {"id":"protocraft", "read":tr("completo para versões nativas suportadas"), "write":tr("nativo"), "notes":tr("Persistência versionada do ProtoCraft; conversões externas podem carregar perdas explícitas.")},
            {"id":"proteus_isis", "read":tr("experimental e parcial"), "write":tr("não suportado"), "notes":tr("PDSPrj/PDSBak/DSN reconhecidos pelo importador parcial; não inclui reexportação Proteus.")},
            {"id":"kicad_pcb", "read":tr("subconjunto PCB com relatório de perdas"), "write":tr("exportador PCB atual"), "notes":tr("Não inclui esquemático, configuração do projeto, regras completas ou todas as geometrias KiCad.")},
            {"id":"kicad_schematic", "read":tr("subconjunto de esquemático com relatório de perdas"), "write":tr("exportador de esquemático atual"), "notes":tr("Não inclui PCB, modelos de simulação ou todos os recursos hierárquicos e gráficos.")},
            {"id":"kicad_pro", "read":tr("não é um desenho convertível"), "write":tr("não suportado"), "notes":tr("Arquivo de configurações do projeto KiCad; converta o .kicad_pcb e/ou .kicad_sch correspondente.")},
            {"id":"specctra_dsn", "read":tr("não suportado"), "write":tr("não suportado"), "notes":tr("DSN Specctra é uma troca de roteamento e não é o DSN binário do Proteus ISIS.")},
            {"id":"legacy_kicad", "read":tr("não suportado"), "write":tr("não suportado"), "notes":tr("Formatos legados .sch/.brd precisam de conversão explícita no KiCad para formatos modernos.")}
        ],
        "loss_policy": tr("Por padrão, qualquer conversão com perdas é recusada. --allow-lossy autoriza a gravação, mantendo as perdas no relatório JSON.")
    })
}

fn run_convert(args: &[String]) -> Result<(), Box<dyn Error>> {
    let options = parse_options(args)?;
    let source_path = &options.input;
    let output_path = &options.output;
    let source_canonical = fs::canonicalize(source_path)?;
    let output_resolved = resolved_destination(output_path)?;
    if same_path(&source_canonical, &output_resolved) {
        return Err(tr("O destino não pode substituir o arquivo de entrada.").into());
    }
    if output_path.try_exists()? {
        return Err(tr("O arquivo de saída já existe; ele foi preservado.").into());
    }

    let sniff = read_prefix(source_path)?;
    if let Some(message) = unsupported_input_message(source_path, &sniff) {
        return Err(tr(message).into());
    }
    let source_format = options
        .from
        .or_else(|| detect_format(source_path, &sniff))
        .ok_or_else(|| tr("Formato de entrada não reconhecido ou não suportado."))?;
    let target_format = options.target;
    if source_format == target_format {
        return Err(tr(
            "A origem e o destino têm o mesmo formato; escolha outro destino de conversão.",
        )
        .into());
    }
    if target_format == Format::Proteus {
        return Err(tr(
            "A exportação para Proteus não é suportada; não existe gravador PDS/DSN fiel.",
        )
        .into());
    }
    validate_extension(output_path, target_format)?;

    let report_resolved = if let Some(report) = &options.report {
        let resolved = resolved_destination(report)?;
        if same_path(&resolved, &source_canonical) || same_path(&resolved, &output_resolved) {
            return Err(
                tr("O caminho do relatório deve ser diferente da entrada e da saída.").into(),
            );
        }
        if report.try_exists()? {
            return Err(tr("O arquivo de relatório já existe; ele foi preservado.").into());
        }
        Some(resolved)
    } else {
        None
    };

    let loaded = load_source(source_path, source_format)?;
    let mut losses: Vec<ConversionIssue> = loaded
        .report
        .losses
        .iter()
        .map(|loss| ConversionIssue {
            code: loss.code.clone(),
            message: loss.message.clone(),
        })
        .collect();
    let mut warnings = loaded.report.warnings.clone();
    if report_resolved.is_some() {
        warnings.push(tr("O projeto e o relatório JSON são gravados separadamente; uma falha do sistema de arquivos pode impedir a criação do segundo arquivo."));
    }
    let mut text_output = None;
    let scope = match target_format {
        Format::Protocraft => loaded.scope.to_owned(),
        Format::KicadPcb => {
            let board_present = loaded.project.board.is_some();
            if !board_present {
                return Err(tr("A exportação KiCad PCB requer uma placa no projeto; esquemáticos não são convertidos em placas automaticamente.").into());
            }
            losses.push(issue(
                "WHOLE_PROJECT_REDUCED_TO_PCB",
                "A saída contém somente PCB; esquemático, modelos de simulação e demais dados do projeto não são preservados.",
            ));
            warnings.push(tr("O exportador PCB representa apenas o subconjunto documentado; verifique recursos de placa não representados antes de fabricar."));
            text_output = Some(eda_pcb::export_kicad(&loaded.project)?);
            "pcb".to_owned()
        }
        Format::KicadSchematic => {
            let (text, target_report) =
                eda_import::kicad_schematic::export_kicad_schematic(&loaded.project)?;
            losses.extend(target_report.losses.iter().map(|loss| ConversionIssue {
                code: loss.code.clone(),
                message: loss.message.clone(),
            }));
            warnings.extend(target_report.warnings);
            text_output = Some(text);
            "schematic".to_owned()
        }
        Format::Proteus => return Err(tr("A exportação para Proteus não é suportada.").into()),
    };

    let whole_project_preserved =
        target_format == Format::Protocraft && loaded.whole_project_preserved_on_import;
    if !whole_project_preserved {
        let loss_code = match target_format {
            Format::Protocraft => "SOURCE_PROJECT_SCOPE_PARTIAL",
            Format::KicadPcb => "WHOLE_PROJECT_REDUCED_TO_PCB",
            Format::KicadSchematic => "WHOLE_PROJECT_REDUCED_TO_SCHEMATIC",
            Format::Proteus => "PROTEUS_EXPORT_UNSUPPORTED",
        };
        if !losses.iter().any(|loss| loss.code == loss_code) {
            let message = match loss_code {
                "SOURCE_PROJECT_SCOPE_PARTIAL" => {
                    "A origem representa somente um domínio importado; não contém o projeto completo original."
                }
                "WHOLE_PROJECT_REDUCED_TO_PCB" => {
                    "A saída contém somente PCB; esquemático, modelos de simulação e demais dados do projeto não são preservados."
                }
                "WHOLE_PROJECT_REDUCED_TO_SCHEMATIC" => {
                    "A saída contém somente esquemático; PCB, modelos de simulação e demais dados do projeto não são preservados."
                }
                _ => "A exportação para Proteus não é suportada.",
            };
            losses.push(issue(loss_code, message));
        }
    }
    let lossless = losses.is_empty()
        && loaded.report.electrical_complete
        && loaded.report.geometry_complete
        && whole_project_preserved;
    let mut report = ConversionReport {
        input: source_path.display().to_string(),
        source_format: loaded.format.id().to_owned(),
        output: output_path.display().to_string(),
        target_format: target_format.id().to_owned(),
        scope,
        lossless,
        whole_project_preserved,
        artifact_publication: if options.report.is_some() {
            "separate_files_non_atomic"
        } else {
            "single_output_file"
        },
        output_written: false,
        losses,
        warnings,
    };

    if let Some(report_path) = &report_resolved {
        // A recusa por perda ainda deixa o diagnóstico solicitado, sem criar o arquivo de saída.
        if !lossless && !options.allow_lossy {
            write_new(report_path, &serde_json::to_vec_pretty(&report)?)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            return Err(tr("Conversão recusada porque o relatório indica perdas; consulte o JSON ou use --allow-lossy após revisar as limitações.").into());
        }
    } else if !lossless && !options.allow_lossy {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Err(tr("Conversão recusada porque o relatório indica perdas; use --report para salvar o diagnóstico ou --allow-lossy após revisar as limitações.").into());
    }

    if target_format == Format::Protocraft {
        eda_storage::create_new(output_path, &loaded.project)?;
    } else if let Some(text) = text_output {
        write_new(&output_resolved, text.as_bytes())?;
    }
    report.output_written = true;
    let report_bytes = serde_json::to_vec_pretty(&report)?;
    if let Some(report_path) = report_resolved {
        write_new(&report_path, &report_bytes)?;
    }
    println!("{}", String::from_utf8(report_bytes)?);
    Ok(())
}

#[derive(Debug)]
struct Options {
    input: PathBuf,
    output: PathBuf,
    target: Format,
    from: Option<Format>,
    report: Option<PathBuf>,
    allow_lossy: bool,
}

fn parse_options(args: &[String]) -> Result<Options, Box<dyn Error>> {
    let input = args
        .first()
        .filter(|value| !value.starts_with("--"))
        .map(PathBuf::from)
        .ok_or_else(|| tr("Uso: protocraft-cli convert <entrada> --output <destino> --to <protocraft|kicad-pcb|kicad-sch> [--from <formato>] [--report <json>] [--allow-lossy]"))?;
    let mut output = None;
    let mut target = None;
    let mut from = None;
    let mut from_set = false;
    let mut report = None;
    let mut allow_lossy = false;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--output" if output.is_none() => {
                output = Some(PathBuf::from(option_value(args, &mut index)?));
            }
            "--to" if target.is_none() => {
                let value = option_value(args, &mut index)?;
                target = Some(
                    Format::parse(&value)
                        .filter(|f| *f != Format::Proteus)
                        .ok_or_else(|| {
                            tr("Destino inválido; use protocraft, kicad-pcb ou kicad-sch.")
                        })?,
                );
            }
            "--from" if !from_set => {
                let value = option_value(args, &mut index)?;
                from = if value == "auto" {
                    None
                } else {
                    Some(Format::parse(&value).ok_or_else(|| tr("Formato de origem inválido."))?)
                };
                from_set = true;
            }
            "--report" if report.is_none() => {
                report = Some(PathBuf::from(option_value(args, &mut index)?));
            }
            "--allow-lossy" if !allow_lossy => {
                allow_lossy = true;
                index += 1;
            }
            option => {
                return Err(eda_i18n::format(
                    "Opção desconhecida ou repetida para convert: {option}",
                    &[("option", option.to_owned())],
                )
                .into());
            }
        }
    }
    Ok(Options {
        input,
        output: output.ok_or_else(|| tr("Informe --output <destino>."))?,
        target: target.ok_or_else(|| tr("Informe --to <formato>."))?,
        from,
        report,
        allow_lossy,
    })
}

fn option_value(args: &[String], index: &mut usize) -> Result<String, Box<dyn Error>> {
    let value = args
        .get(*index + 1)
        .filter(|value| !value.starts_with("--"))
        .cloned()
        .ok_or_else(|| tr("Falta o valor de uma opção de conversão."))?;
    *index += 2;
    Ok(value)
}

fn read_prefix(path: &Path) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_SNIFF_BYTES as u64)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn detect_format(path: &Path, prefix: &[u8]) -> Option<Format> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "protocraft" => return Some(Format::Protocraft),
        "pdsprj" | "pdsbak" => return Some(Format::Proteus),
        "dsn" => {
            return (eda_import::dsn::detect_dsn_format(prefix)
                == eda_import::dsn::DsnFormat::ProteusIsis)
                .then_some(Format::Proteus);
        }
        _ => {}
    }
    let text = std::str::from_utf8(prefix)
        .ok()?
        .trim_start_matches('\u{feff}')
        .trim_start();
    match extension.as_str() {
        "kicad_pcb" if text.starts_with("(kicad_pcb") => Some(Format::KicadPcb),
        "kicad_sch" if text.starts_with("(kicad_sch") => Some(Format::KicadSchematic),
        "kicad_pro" => None,
        "sch" | "brd" => None,
        _ if text.starts_with("(kicad_pcb") => Some(Format::KicadPcb),
        _ if text.starts_with("(kicad_sch") => Some(Format::KicadSchematic),
        _ if serde_json::from_slice::<serde_json::Value>(prefix)
            .ok()
            .and_then(|value| {
                value
                    .get("format_version")
                    .and_then(serde_json::Value::as_u64)
            })
            .is_some() =>
        {
            Some(Format::Protocraft)
        }
        _ => None,
    }
}

fn unsupported_input_message(path: &Path, prefix: &[u8]) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "kicad_pro" => Some(
            "Arquivo de configurações do projeto KiCad; converta o .kicad_pcb e/ou .kicad_sch correspondente.",
        ),
        "dsn"
            if eda_import::dsn::detect_dsn_format(prefix)
                == eda_import::dsn::DsnFormat::Specctra =>
        {
            Some("DSN Specctra é uma troca de roteamento e não é o DSN binário do Proteus ISIS.")
        }
        "sch" | "brd" => Some(
            "Formatos legados .sch/.brd precisam de conversão explícita no KiCad para formatos modernos.",
        ),
        _ => None,
    }
}

fn load_source(path: &Path, format: Format) -> Result<LoadedSource, Box<dyn Error>> {
    match format {
        Format::Protocraft => {
            let project = eda_storage::load(path)?;
            Ok(LoadedSource {
                project,
                format,
                scope: "whole_project",
                whole_project_preserved_on_import: true,
                report: InterchangeReport {
                    format: format.id().to_owned(),
                    electrical_complete: true,
                    geometry_complete: true,
                    losses: vec![],
                    warnings: vec![],
                },
            })
        }
        Format::Proteus => {
            let imported = eda_import::import_proteus(path)?;
            let losses = if imported.report.electrical_complete {
                vec![]
            } else {
                vec![InterchangeIssue {
                    code: "PROTEUS_PARTIAL_IMPORT".into(),
                    message: tr(
                        "O importador Proteus não confirma preservação elétrica completa e não importa integralmente PCB, regras e modelos.",
                    ),
                }]
            };
            Ok(LoadedSource {
                project: imported.project,
                format,
                scope: "schematic_import",
                whole_project_preserved_on_import: false,
                report: InterchangeReport {
                    format: format.id().to_owned(),
                    electrical_complete: imported.report.electrical_complete,
                    geometry_complete: false,
                    losses,
                    warnings: imported.report.warnings,
                },
            })
        }
        Format::KicadPcb => {
            let imported = eda_import::kicad::import_kicad_pcb(path)?;
            Ok(LoadedSource {
                project: imported.project,
                format,
                scope: "pcb",
                whole_project_preserved_on_import: false,
                report: imported.report,
            })
        }
        Format::KicadSchematic => {
            let imported = eda_import::kicad_schematic::import_kicad_schematic(path)?;
            Ok(LoadedSource {
                project: imported.project,
                format,
                scope: "schematic",
                whole_project_preserved_on_import: false,
                report: imported.report,
            })
        }
    }
}

fn validate_extension(path: &Path, target: Format) -> Result<(), Box<dyn Error>> {
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let valid = match target {
        Format::Protocraft => extension.eq_ignore_ascii_case("protocraft"),
        Format::KicadPcb => extension.eq_ignore_ascii_case("kicad_pcb"),
        Format::KicadSchematic => extension.eq_ignore_ascii_case("kicad_sch"),
        Format::Proteus => false,
    };
    if !valid {
        return Err(tr("A extensão do destino não corresponde ao formato solicitado.").into());
    }
    Ok(())
}

fn resolved_destination(path: &Path) -> Result<PathBuf, Box<dyn Error>> {
    if path.exists() {
        return Ok(fs::canonicalize(path)?);
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| tr("Destino deve conter um nome de arquivo."))?;
    Ok(fs::canonicalize(parent)?.join(name))
}

fn same_path(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        left.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(&right.as_os_str().to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

fn write_new(path: &Path, contents: &[u8]) -> Result<(), Box<dyn Error>> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(contents)?;
    file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn detecta_specctra_como_formato_nao_suportado_e_nao_como_proteus() {
        let path = Path::new("route.dsn");
        assert_eq!(
            detect_format(path, b"\xef\xbb\xbf; Comentario\n (pcb routed\n"),
            None
        );
        assert_eq!(
            unsupported_input_message(path, b"; Comentario\n(pcb routed\n"),
            Some("DSN Specctra é uma troca de roteamento e não é o DSN binário do Proteus ISIS.")
        );
        assert_eq!(
            detect_format(path, b"ISIS SCHEMATIC FILE\x1a"),
            Some(Format::Proteus)
        );
    }

    #[test]
    fn detecta_arquivos_moderos_kicad_pelo_conteudo() {
        assert_eq!(
            detect_format(Path::new("board.any"), b"(kicad_pcb "),
            Some(Format::KicadPcb)
        );
        assert_eq!(
            detect_format(Path::new("design.any"), b"(kicad_sch "),
            Some(Format::KicadSchematic)
        );
        assert!(unsupported_input_message(Path::new("project.kicad_pro"), b"{}").is_some());
        assert!(unsupported_input_message(Path::new("legacy.brd"), b"").is_some());
    }

    #[test]
    fn escrita_de_novo_arquivo_preserva_destino_existente() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input.protocraft");
        let output = directory.path().join("output.protocraft");
        eda_storage::create_new(&input, &eda_core::Project::empty("teste")).unwrap();
        let input_before = fs::read(&input).unwrap();
        fs::write(&output, b"preserve").unwrap();
        let result = write_new(&output, b"replacement");
        assert!(result.is_err());
        assert_eq!(fs::read(&output).unwrap(), b"preserve");
        assert_eq!(fs::read(&input).unwrap(), input_before);
    }

    #[test]
    fn recusa_perdas_por_padrao_e_salva_relatorio_sem_criar_saida() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input.protocraft");
        let output = directory.path().join("output.kicad_sch");
        let report = directory.path().join("report.json");
        eda_storage::create_new(&input, &eda_core::Project::empty("teste")).unwrap();
        let args = vec![
            input.to_string_lossy().into_owned(),
            "--output".into(),
            output.to_string_lossy().into_owned(),
            "--to".into(),
            "kicad-sch".into(),
            "--report".into(),
            report.to_string_lossy().into_owned(),
        ];
        assert!(run_convert(&args).is_err());
        assert!(!output.exists());
        let diagnostic: serde_json::Value =
            serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
        assert_eq!(diagnostic["output_written"], false);
        assert_eq!(
            diagnostic["artifact_publication"],
            "separate_files_non_atomic"
        );
        assert!(!diagnostic["losses"].as_array().unwrap().is_empty());
    }

    #[test]
    fn grava_saida_e_relatorio_quando_perdas_sao_autorizadas() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input.protocraft");
        let output = directory.path().join("output.kicad_sch");
        let report = directory.path().join("report.json");
        eda_storage::create_new(&input, &eda_core::Project::empty("teste")).unwrap();
        let args = vec![
            input.to_string_lossy().into_owned(),
            "--output".into(),
            output.to_string_lossy().into_owned(),
            "--to".into(),
            "kicad-sch".into(),
            "--report".into(),
            report.to_string_lossy().into_owned(),
            "--allow-lossy".into(),
        ];
        run_convert(&args).unwrap();
        assert!(
            fs::read_to_string(output)
                .unwrap()
                .starts_with("(kicad_sch")
        );
        let diagnostic: serde_json::Value =
            serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
        assert_eq!(diagnostic["output_written"], true);
        assert_eq!(diagnostic["lossless"], false);
    }

    #[test]
    fn preserva_destinos_e_relatorios_existentes() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input.protocraft");
        let output = directory.path().join("output.kicad_sch");
        let report = directory.path().join("report.json");
        eda_storage::create_new(&input, &eda_core::Project::empty("teste")).unwrap();
        fs::write(&output, b"saida existente").unwrap();
        let args = vec![
            input.to_string_lossy().into_owned(),
            "--output".into(),
            output.to_string_lossy().into_owned(),
            "--to".into(),
            "kicad-sch".into(),
        ];
        assert!(run_convert(&args).is_err());
        assert_eq!(fs::read(&output).unwrap(), b"saida existente");

        fs::remove_file(&output).unwrap();
        fs::write(&report, b"relatorio existente").unwrap();
        let mut args_with_report = args;
        args_with_report.extend(["--report".into(), report.to_string_lossy().into_owned()]);
        assert!(run_convert(&args_with_report).is_err());
        assert!(!output.exists());
        assert_eq!(fs::read(&report).unwrap(), b"relatorio existente");
    }

    #[test]
    fn importa_e_reexporta_pcb_kicad_pelo_fluxo_convert() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("board.kicad_pcb");
        let native = directory.path().join("board.protocraft");
        let import_report = directory.path().join("import.json");
        let output = directory.path().join("roundtrip.kicad_pcb");
        let export_report = directory.path().join("export.json");
        fs::write(
            &input,
            r#"(kicad_pcb (version 20240108) (generator "pcbnew")
              (net 0 "") (net 1 "GND") (net 2 "VCC")
              (footprint "Test:Resistor" (layer "F.Cu") (at 10 10 0.000000)
                (property "Reference" "R1") (property "Value" "10k")
                (fp_line (start -2 -1) (end 2 -1) (stroke (width 0.05) (type solid)) (layer "F.CrtYd"))
                (fp_line (start 2 -1) (end 2 1) (stroke (width 0.05) (type solid)) (layer "F.CrtYd"))
                (fp_line (start 2 1) (end -2 1) (stroke (width 0.05) (type solid)) (layer "F.CrtYd"))
                (fp_line (start -2 1) (end -2 -1) (stroke (width 0.05) (type solid)) (layer "F.CrtYd"))
                (pad "1" smd rect (at -1 0 0.000000) (size 0.8 0.8) (layers "F.Cu" "F.Mask") (net 1 "GND"))
                (pad "2" smd rect (at 1 0 0.000000) (size 0.8 0.8) (layers "F.Cu" "F.Mask") (net 2 "VCC")))
              (gr_rect (start 0 0) (end 30 20)
                (stroke (width 0.05) (type default)) (fill none) (layer "Edge.Cuts")))"#,
        )
        .unwrap();

        let mut import_args = vec![
            input.to_string_lossy().into_owned(),
            "--output".into(),
            native.to_string_lossy().into_owned(),
            "--to".into(),
            "protocraft".into(),
        ];
        assert!(run_convert(&import_args).is_err());
        assert!(!native.exists());
        import_args.extend([
            "--report".into(),
            import_report.to_string_lossy().into_owned(),
            "--allow-lossy".into(),
        ]);
        run_convert(&import_args).unwrap();
        let imported = eda_storage::load(&native).unwrap();
        assert!(imported.board.is_some());
        assert_eq!(imported.components.len(), 1);
        let diagnostic: serde_json::Value =
            serde_json::from_slice(&fs::read(import_report).unwrap()).unwrap();
        assert_eq!(diagnostic["output_written"], true);
        assert_eq!(diagnostic["lossless"], false);

        run_convert(&[
            native.to_string_lossy().into_owned(),
            "--output".into(),
            output.to_string_lossy().into_owned(),
            "--to".into(),
            "kicad-pcb".into(),
            "--report".into(),
            export_report.to_string_lossy().into_owned(),
            "--allow-lossy".into(),
        ])
        .unwrap();
        let exported = fs::read_to_string(&output).unwrap();
        assert!(exported.starts_with("(kicad_pcb"));
        let reparsed = eda_import::kicad::parse_kicad_pcb("roundtrip", &exported).unwrap();
        assert!(reparsed.project.board.is_some());
        assert_eq!(reparsed.project.components.len(), 1);
    }

    #[test]
    fn parser_rejeita_origem_auto_repetida() {
        let args = vec![
            "input.protocraft".into(),
            "--output".into(),
            "output.kicad_sch".into(),
            "--to".into(),
            "kicad-sch".into(),
            "--from".into(),
            "auto".into(),
            "--from".into(),
            "auto".into(),
        ];
        assert!(parse_options(&args).is_err());
    }

    #[test]
    fn parser_aceita_conversao_com_perdas_explicitamente_autorizada() {
        let args = vec![
            "input.protocraft".into(),
            "--output".into(),
            "output.kicad_sch".into(),
            "--to".into(),
            "kicad-sch".into(),
            "--from".into(),
            "auto".into(),
            "--allow-lossy".into(),
        ];
        let options = parse_options(&args).unwrap();
        assert_eq!(options.target, Format::KicadSchematic);
        assert!(options.allow_lossy);
    }
}
