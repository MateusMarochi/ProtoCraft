use super::*;
use eda_core::{
    Board, Command, ComponentId, Engine, NM_PER_MM, NmPoint, Placement, Point, Project,
};
pub fn handles(cmd: &str) -> bool {
    matches!(
        cmd,
        "devices"
            | "device"
            | "device-project"
            | "pcb-info"
            | "pcb-check"
            | "pcb-export"
            | "pcb-manufacture"
    )
}
fn json(value: &impl serde::Serialize) -> Result<(), Box<dyn Error>> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    let cmd = &args[0];
    if args.get(1).is_some_and(|a| a == "--help") {
        println!(
            "devices [--filter texto] [--limit N] [--offset N]\ndevice <modelo>\ndevice-project <modelo> --output arquivo.protocraft\npcb-info <projeto>\npcb-check <projeto>\npcb-export <projeto> --format kicad|svg --output arquivo\npcb-manufacture <projeto> --output pasta-nova --kicad caminho/kicad-cli.exe [--preset jlcpcb|standard]\nPreset padrão: jlcpcb. Produz JLCPCB.zip, RS-274X/Protel, mm, 1:1, furação absoluta Excellon.\nCoordenadas físicas em nanômetros; 1 mm = 1.000.000 nm. Catálogo físico, sem emulação de firmware."
        );
        return Ok(());
    }
    if cmd == "devices" {
        let mut filter = "";
        let mut limit = 100usize;
        let mut offset = 0usize;
        let mut i = 1;
        while i < args.len() {
            let value = args.get(i + 1).ok_or("Opção sem valor")?;
            match args[i].as_str() {
                "--filter" => filter = value,
                "--limit" => limit = value.parse()?,
                "--offset" => offset = value.parse()?,
                _ => return Err("Opção desconhecida".into()),
            }
            i += 2;
        }
        if limit == 0 || limit > 5000 {
            return Err("Limite deve estar entre 1 e 5000".into());
        }
        let c = eda_library::catalog()?;
        let matches = c.search(filter);
        let rows:Vec<_>=matches.iter().skip(offset).take(limit).map(|d|{let f=&c.footprints[&d.footprint];serde_json::json!({"id":d.id,"family":d.family,"manufacturer":d.manufacturer,"form":d.form,"pins":c.symbols[&d.symbol].len(),"footprint":d.footprint,"body_mm":{"width":f.body_bounds.width() as f64/1e6,"height":f.body_bounds.height() as f64/1e6},"source":d.source})}).collect();
        return json(&serde_json::json!({"total":matches.len(),"offset":offset,"devices":rows}));
    }
    let input = args.get(1).ok_or("Informe o modelo ou projeto")?;
    if cmd == "device" {
        if args.len() != 2 {
            return Err("Informe somente o modelo".into());
        }
        let (definition, footprint) = eda_library::catalog()?.definition(input)?;
        return json(
            &serde_json::json!({"device":definition,"footprint":footprint,"firmware_simulation":false}),
        );
    }
    if cmd == "device-project" {
        if args.len() != 4 || args[2] != "--output" {
            return Err("Uso: device-project <modelo> --output arquivo.protocraft".into());
        }
        let path = Path::new(&args[3]);
        if path
            .extension()
            .is_none_or(|e| !e.eq_ignore_ascii_case("protocraft"))
        {
            return Err("Use a extensão .protocraft".into());
        }
        let c = eda_library::catalog()?;
        let mut engine = Engine::new(Project::empty(format!(
            "{} — projeto físico",
            c.find(input)?.id
        )))?;
        engine.execute(Command::Batch(vec![
            c.place_command(input, Point::new(0, 0))?,
            Command::SetBoard {
                board: Board::default(),
            },
            Command::PlaceOnBoard {
                id: ComponentId(1),
                placement: Placement {
                    position: NmPoint::new(50 * NM_PER_MM, 40 * NM_PER_MM),
                    rotation: 0,
                    side: Default::default(),
                },
            },
        ]))?;
        eda_storage::save(path, engine.project())?;
        return json(
            &serde_json::json!({"output":path,"device":c.find(input)?.id,"format_version":eda_core::FORMAT_VERSION}),
        );
    }
    let project = eda_storage::load(input)?;
    if cmd == "pcb-info" {
        if args.len() != 2 {
            return Err("Opções inesperadas".into());
        }
        return json(
            &serde_json::json!({"board":project.board,"pin_map":eda_pcb::pin_map(&project)?}),
        );
    }
    if cmd == "pcb-check" {
        if args.len() != 2 {
            return Err("Opções inesperadas".into());
        }
        let report = eda_pcb::check(&project)?;
        json(&report)?;
        if report.errors() > 0 {
            return Err("DRC encontrou violações; consulte os diagnósticos".into());
        }
        return Ok(());
    }
    if cmd == "pcb-manufacture" {
        use eda_storage::fabrication::ManufacturingPreset;
        let mut output = None;
        let mut tool = None;
        let mut preset = None;
        let mut i = 2;
        while i < args.len() {
            match args[i].as_str() {
                "--output" if output.is_none() => {
                    output = Some(args.get(i + 1).ok_or("Informe a pasta nova")?)
                }
                "--kicad" if tool.is_none() => {
                    tool = Some(args.get(i + 1).ok_or("Informe kicad-cli.exe")?)
                }
                "--preset" if preset.is_none() => {
                    preset = Some(match args.get(i + 1).map(String::as_str) {
                        Some("jlcpcb") => ManufacturingPreset::Jlcpcb,
                        Some("standard") => ManufacturingPreset::Standard,
                        _ => return Err("Use --preset jlcpcb ou standard".into()),
                    });
                }
                _ => return Err("Opção desconhecida ou repetida".into()),
            }
            i += 2;
        }
        let destination = Path::new(output.ok_or("Informe --output pasta-nova")?);
        let tool = Path::new(tool.ok_or("Informe --kicad caminho/kicad-cli.exe")?);
        let preset = preset.unwrap_or_default();
        let path =
            eda_storage::fabrication::generate_with_preset(&project, destination, tool, preset)?;
        let archive = preset.archive_name().map(|name| path.join(name));
        return json(
            &serde_json::json!({"output":path,"preset":preset.id(),"archive":archive,"gerber":true,"excellon":true,"external_drc":true}),
        );
    }
    if cmd == "pcb-export" {
        let mut format = None;
        let mut output = None;
        let mut i = 2;
        while i < args.len() {
            match args[i].as_str() {
                "--format" if format.is_none() => {
                    format = Some(args.get(i + 1).ok_or("Informe o formato")?.as_str())
                }
                "--output" if output.is_none() => {
                    output = Some(args.get(i + 1).ok_or("Informe a saída")?)
                }
                _ => return Err("Opção desconhecida ou repetida".into()),
            }
            i += 2;
        }
        let output = output.ok_or("Informe --output")?;
        if same_path(
            &fs::canonicalize(input)?,
            &resolved_destination(Path::new(output))?,
        ) {
            return Err("A exportação deve preservar o projeto de entrada".into());
        }
        let text = match format.ok_or("Informe --format kicad ou svg")? {
            "kicad" => eda_pcb::export_kicad(&project)?,
            "svg" => eda_pcb::export_svg(&project)?,
            _ => return Err("Formato deve ser kicad ou svg".into()),
        };
        fs::write(output, text)?;
        println!("PCB exportada: {output}");
        return Ok(());
    }
    Err("Comando físico desconhecido".into())
}
