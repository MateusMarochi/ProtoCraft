mod automation;
mod physical_cli;

use automation::{Action, COMMANDS, parse_action, parse_actions};
use eda_simulation::{Analysis, SimulationControl, SimulationEngine, SimulationRequest};
use std::{
    env,
    error::Error,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("Erro: {error}");
        std::process::exit(1);
    }
}
fn help() {
    println!(
        "ProtoCraft — automação de circuitos\n\nUso:\n  protocraft-cli templates\n  protocraft-cli new --output projeto.protocraft [--template 555-led-1hz] [--name 'Meu circuito'] [--open]\n  protocraft-cli open <projeto.protocraft>\n  protocraft-cli devices [--filter termo] [--limit N] [--offset N]\n  protocraft-cli device <modelo>\n  protocraft-cli device-project <modelo> --output projeto.protocraft\n  protocraft-cli pcb-check <projeto>\n  protocraft-cli pcb-route <projeto> --output roteado.protocraft [--grid-mm 0.25] [--track-width-mm 0.25] [--max-expansions 600000] [--no-optimize]\n  protocraft-cli pcb-info <projeto>\n  protocraft-cli pcb-export <projeto> --format kicad|svg --output arquivo\n  protocraft-cli pcb-manufacture <projeto> --preset jlcpcb|standard --output pasta-nova --kicad caminho/kicad-cli.exe\n  protocraft-cli engines\n  protocraft-cli components\n  protocraft-cli examples <pasta>\n  protocraft-cli import-proteus <arquivo.pdsprj|arquivo.dsn> --output <arquivo.protocraft> [--report <relatorio.json>]\n  protocraft-cli info <arquivo.protocraft>\n  protocraft-cli commands [--filter <termo>]\n  protocraft-cli run <arquivo.protocraft> (--actions <acoes.json> | --cmd <id> --params '<json>' [...]) [--output <novo.protocraft>]\n  protocraft-cli validate <arquivo.protocraft>\n  protocraft-cli netlist <arquivo.protocraft>\n  protocraft-cli simulate <arquivo.protocraft> [--engine stable|dynamic|mixed] [--transient passo_s fim_s] [--csv arquivo.csv]\n  protocraft-cli spice <arquivo.protocraft> [--transient passo_s fim_s] [--output arquivo.cir]\n\nnew cria um projeto novo; --open inicia o editor desktop se estiver junto da CLI.\nrun aplica todos os comandos como uma edição atômica do Engine; use --output para salvar.\ninfo, commands e run escrevem JSON em stdout. Pontos usam coordenadas inteiras da grade.\nImportação Proteus experimental: consulte os avisos e o relatório de limitações.\nSimulação nativa em Rust: motores stable, dynamic e mixed, DC e transiente, modelos genéricos.\nUse engines e components para consultar as opções. C/L iniciam em zero; estados digitais são configuráveis no motor Misto."
    );
}
fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        help();
        return Ok(());
    }
    let command = args[0].as_str();
    if command == "--version" {
        println!("ProtoCraft {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.len() == 2
        && args[1] == "--help"
        && let Some(details) = command_help(command)
    {
        println!("{details}");
        return Ok(());
    }
    if command == "new" || command == "create" {
        return run_new_project(&args[1..]);
    }
    if command == "templates" {
        if args.len() != 1 {
            return Err("Uso: protocraft-cli templates".into());
        }
        return list_templates();
    }
    if command == "open" {
        return run_open_project(&args[1..]);
    }
    if physical_cli::handles(command) {
        return physical_cli::run(&args);
    }

    if args.len() == 2 && args[1] == "--help" {
        if let Some(details) = command_help(command) {
            println!("{details}");
            return Ok(());
        }
        return Err(format!("Comando desconhecido: {command}. Use --help").into());
    }
    if command == "engines" || command == "components" {
        if args.len() != 1 {
            return Err("Este comando não recebe argumentos".into());
        }
        let values: Vec<_> = if command == "engines" {
            SimulationEngine::all().iter().map(|e|serde_json::json!({"id":e.id(),"name":e.label(),"description":e.description()})).collect()
        } else {
            eda_core::ComponentKind::all().iter().map(|k|serde_json::json!({"kind":k,"name":k.label(),"pins":k.pin_count(),"unit":k.unit(),"value":k.default_value(),"parameters":eda_core::ModelParameters::default()})).collect()
        };
        println!("{}", serde_json::to_string_pretty(&values)?);
        return Ok(());
    }
    if command == "import-proteus" {
        return run_proteus_import(&args[1..]);
    }
    if command == "info" {
        return run_info(&args[1..]);
    }
    if command == "commands" {
        return run_commands(&args[1..]);
    }
    if command == "run" {
        return run_actions(&args[1..]);
    }
    if command == "examples" {
        if args.len() != 2 {
            return Err("Informe somente a pasta de destino dos exemplos".into());
        }
        let destination = Path::new(&args[1]);
        fs::create_dir_all(destination)?;
        let mut projects: Vec<(PathBuf, eda_core::Project)> = eda_core::examples::CATALOG
            .iter()
            .map(|(id, _, factory)| (destination.join(format!("{id}.protocraft")), factory()))
            .collect();
        projects.push((
            destination.join("stm32-esp32.protocraft"),
            eda_library::physical_example()?,
        ));
        if let Some((path, _)) = projects.iter().find(|(path, _)| path.exists()) {
            return Err(format!(
                "O destino já contém {}; nenhum exemplo foi sobrescrito",
                path.display()
            )
            .into());
        }
        for (path, project) in projects {
            eda_storage::create_new(&path, &project)?;
            println!("Exemplo criado: {}", path.display());
        }
        return Ok(());
    }
    if !["validate", "netlist", "simulate", "spice"].contains(&command) {
        return Err(format!("Comando desconhecido: {command}. Use --help").into());
    }
    let input = args.get(1).ok_or("Informe o arquivo .protocraft")?;
    let mut analysis = Analysis::Dc;
    let mut engine = SimulationEngine::default();
    let mut csv = None;
    let mut output = None;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--engine" if command == "simulate" => {
                engine = SimulationEngine::parse(
                    args.get(index + 1)
                        .ok_or("Informe stable, dynamic ou mixed")?,
                )?;
                index += 2;
            }
            "--transient" if command == "simulate" || command == "spice" => {
                let step: f64 = args
                    .get(index + 1)
                    .ok_or("Informe passo e fim do transiente")?
                    .parse()
                    .map_err(|_| "Passo de tempo inválido")?;
                let stop: f64 = args
                    .get(index + 2)
                    .ok_or("Informe o fim do transiente")?
                    .parse()
                    .map_err(|_| "Fim do transiente inválido")?;
                if !step.is_finite() || !stop.is_finite() || step <= 0.0 || stop < step {
                    return Err("Transiente requer 0 < passo <= fim, em segundos".into());
                }
                analysis = Analysis::Transient { step, stop };
                index += 3;
            }
            "--csv" if command == "simulate" => {
                csv = Some(args.get(index + 1).ok_or("Informe o arquivo CSV")?);
                index += 2;
            }
            "--output" if command == "spice" => {
                output = Some(args.get(index + 1).ok_or("Informe o arquivo SPICE")?);
                index += 2;
            }
            option => return Err(format!("Opção desconhecida para {command}: {option}").into()),
        }
    }
    for destination in [csv, output].into_iter().flatten() {
        if Path::new(destination).exists()
            && fs::canonicalize(destination)? == fs::canonicalize(input)?
        {
            return Err("O arquivo de exportação deve ser diferente do projeto de entrada".into());
        }
    }
    let project = eda_storage::load(input)?;
    match command {
        "validate" => {
            println!(
                "Projeto válido: {} ({} componentes, {} fios)",
                project.name,
                project.components.len(),
                project.wires.len()
            );
            let netlist = eda_netlist::build_netlist(&project);
            let diagnostics = eda_netlist::erc(&project, &netlist);
            for diagnostic in &diagnostics {
                println!(
                    "{} [{}]: {}",
                    match diagnostic.severity {
                        eda_netlist::Severity::Error => "Erro",
                        eda_netlist::Severity::Warning => "Aviso",
                    },
                    diagnostic.code,
                    diagnostic.message
                );
            }
            if diagnostics
                .iter()
                .any(|d| matches!(d.severity, eda_netlist::Severity::Error))
            {
                return Err("ERC encontrou erros elétricos".into());
            }
        }
        "netlist" => {
            let netlist = eda_netlist::build_netlist(&project);
            for net in &netlist.nets {
                println!("Nó {} — {} ({} pinos)", net.id.0, net.name, net.pins.len());
                for pin in &net.pins {
                    let reference = project
                        .components
                        .get(&pin.component)
                        .map(|c| c.reference.as_str())
                        .ok_or("Netlist contém componente ausente")?;
                    println!(
                        "  {reference} (ID {}) / pino {}",
                        pin.component.0,
                        project.components[&pin.component].pin_number(pin.pin)
                    );
                }
            }
        }
        "simulate" => {
            let result = eda_simulation::simulate_with_engine(
                &SimulationRequest {
                    project,
                    revision: 0,
                    analysis,
                },
                engine,
                &SimulationControl::default(),
            )?;
            println!("Simulação concluída: {}", result.backend);
            for warning in &result.warnings {
                println!("Aviso: {warning}");
            }
            for trace in &result.traces {
                if let Some(sample) = trace.samples.last() {
                    println!(
                        "{} = {:.9} {} (t={:.9} s, {} amostras)",
                        trace.name,
                        sample[1],
                        trace.unit,
                        sample[0],
                        trace.samples.len()
                    );
                }
            }
            if let Some(path) = csv {
                eda_storage::export_csv(path, &result)?;
                println!("CSV salvo: {path}");
            }
        }
        "spice" => {
            let text = eda_simulation::export_spice(&project, analysis)?;
            if let Some(path) = output {
                fs::write(path, text)?;
                println!("SPICE salvo: {path}");
            } else {
                print!("{text}");
            }
        }
        _ => return Err(format!("Comando não implementado: {command}").into()),
    }
    Ok(())
}

fn list_templates() -> Result<(), Box<dyn Error>> {
    let mut templates = vec![serde_json::json!({
        "id": "empty",
        "name": "Projeto vazio",
        "description": "Folha esquemática em branco, pronta para edição."
    })];
    templates.extend(
        eda_core::examples::CATALOG
            .iter()
            .map(|(id, name, _)| serde_json::json!({"id": id, "name": name})),
    );
    templates.push(serde_json::json!({
        "id": "stm32-esp32",
        "name": "Demonstração STM32 + ESP32-S3",
        "description": "Esquema e footprints para estudo físico; não executa firmware."
    }));
    println!("{}", serde_json::to_string_pretty(&templates)?);
    Ok(())
}

fn run_new_project(args: &[String]) -> Result<(), Box<dyn Error>> {
    let mut output: Option<PathBuf> = None;
    let mut template: Option<String> = None;
    let mut name: Option<String> = None;
    let mut open = false;
    let mut ra_ohms = 47_000.0;
    let mut rb_ohms = 47_000.0;
    let mut capacitance_f = 10e-6;
    let mut led_resistance_ohms = 680.0;
    let mut frequency_hz = None;
    let mut timing_options_used = false;
    let mut capacitance_was_set = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--output" | "-o" if output.is_none() => {
                output = Some(PathBuf::from(
                    args.get(index + 1)
                        .filter(|value| !value.starts_with("--"))
                        .ok_or("Informe o caminho após --output")?,
                ));
                index += 2;
            }
            "--template" if template.is_none() => {
                template = Some(
                    args.get(index + 1)
                        .filter(|value| !value.starts_with("--"))
                        .ok_or("Informe o id após --template")?
                        .clone(),
                );
                index += 2;
            }
            "--name" if name.is_none() => {
                name = Some(
                    args.get(index + 1)
                        .filter(|value| !value.starts_with("--"))
                        .ok_or("Informe o texto após --name")?
                        .clone(),
                );
                index += 2;
            }
            "--open" if !open => {
                open = true;
                index += 1;
            }
            "--ra-ohms" => {
                ra_ohms = parse_new_number(args, index, "RA em ohms")?;
                timing_options_used = true;
                index += 2;
            }
            "--rb-ohms" => {
                rb_ohms = parse_new_number(args, index, "RB em ohms")?;
                timing_options_used = true;
                index += 2;
            }
            "--c-farads" if !capacitance_was_set => {
                capacitance_f = parse_new_number(args, index, "capacitância em farads")?;
                capacitance_was_set = true;
                timing_options_used = true;
                index += 2;
            }
            "--frequency-hz" if frequency_hz.is_none() => {
                frequency_hz = Some(parse_new_number(args, index, "frequência em hertz")?);
                timing_options_used = true;
                index += 2;
            }
            "--led-resistor-ohms" => {
                led_resistance_ohms = parse_new_number(args, index, "resistor do LED em ohms")?;
                timing_options_used = true;
                index += 2;
            }
            option => {
                return Err(format!("Opção desconhecida ou repetida para new: {option}").into());
            }
        }
    }

    let template = template.unwrap_or_else(|| "empty".into());
    let output = output.ok_or("Uso: protocraft-cli new --output projeto.protocraft")?;
    if !output
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("protocraft"))
    {
        return Err("O projeto novo deve usar a extensão .protocraft".into());
    }
    if output.exists() {
        return Err(format!(
            "O arquivo já existe; escolha outro caminho para preservar os dados: {}",
            output.display()
        )
        .into());
    }

    let mut project = if template == "empty" {
        let inferred_name = output
            .file_stem()
            .and_then(|value| value.to_str())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("Novo projeto");
        eda_core::Project::empty(name.as_deref().unwrap_or(inferred_name))
    } else if template == "stm32-esp32" {
        eda_library::physical_example()?
    } else if let Some((_, _, factory)) = eda_core::examples::CATALOG
        .iter()
        .find(|(id, _, _)| *id == template)
    {
        factory()
    } else {
        return Err(format!(
            "Modelo desconhecido: {template}. Consulte `protocraft-cli templates`."
        )
        .into());
    };
    if timing_options_used && template != "555-led-1hz" {
        return Err(
            "Os parâmetros RA/RB/C/frequência/resistor do LED só se aplicam ao modelo 555-led-1hz"
                .into(),
        );
    }
    let estimated_frequency_hz = if template == "555-led-1hz" {
        for (label, value) in [
            ("RA", ra_ohms),
            ("RB", rb_ohms),
            ("capacitância", capacitance_f),
            ("resistor do LED", led_resistance_ohms),
        ] {
            if !value.is_finite() || value <= 0.0 || value > 1e15 {
                return Err(format!("{label} deve ser finito e estar entre 0 e 1e15").into());
            }
        }
        if let Some(frequency) = frequency_hz {
            if capacitance_was_set {
                return Err("Use --frequency-hz ou --c-farads, não os dois".into());
            }
            capacitance_f = 1.44 / ((ra_ohms + 2.0 * rb_ohms) * frequency);
            if !capacitance_f.is_finite() || capacitance_f <= 0.0 || capacitance_f > 1e15 {
                return Err("A frequência escolhida resulta em uma capacitância inválida".into());
            }
        }
        set_component_value(&mut project, "RA", ra_ohms)?;
        set_component_value(&mut project, "RB", rb_ohms)?;
        set_component_value(&mut project, "C1", capacitance_f)?;
        set_component_value(&mut project, "RLED", led_resistance_ohms)?;
        let estimated = 1.44 / ((ra_ohms + 2.0 * rb_ohms) * capacitance_f);
        if timing_options_used && name.is_none() {
            project.name = format!("555 astable com LED — aproximadamente {estimated:.2} Hz");
        }
        Some(estimated)
    } else {
        None
    };
    if let Some(name) = name {
        project.name = name;
    }
    project.validate()?;

    let gui = if open { Some(gui_executable()?) } else { None };
    let parent = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    eda_storage::create_new(&output, &project)?;
    let saved_path = fs::canonicalize(&output)?;
    if let Some(gui) = gui {
        ProcessCommand::new(gui).arg(&saved_path).spawn()?;
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "ok": true,
            "output": saved_path,
            "template": template,
            "name": project.name,
            "format_version": project.format_version,
            "estimated_frequency_hz": estimated_frequency_hz,
            "opened": open,
        }))?
    );
    Ok(())
}

fn parse_new_number(args: &[String], index: usize, label: &str) -> Result<f64, Box<dyn Error>> {
    let value: f64 = args
        .get(index + 1)
        .filter(|value| !value.starts_with("--"))
        .ok_or_else(|| format!("Informe {label} após {}", args[index]))?
        .parse()?;
    if !value.is_finite() || value <= 0.0 {
        return Err(format!("{label} deve ser finito e maior que zero").into());
    }
    Ok(value)
}

fn set_component_value(
    project: &mut eda_core::Project,
    reference: &str,
    value: f64,
) -> Result<(), Box<dyn Error>> {
    let component = project
        .components
        .values_mut()
        .find(|component| component.reference == reference)
        .ok_or_else(|| format!("O modelo não contém o componente {reference}"))?;
    component.value = value;
    Ok(())
}

fn run_open_project(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args.len() != 1 {
        return Err("Uso: protocraft-cli open <projeto.protocraft>".into());
    }
    let path = Path::new(&args[0]);
    if !path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("protocraft"))
    {
        return Err("O projeto deve usar a extensão .protocraft".into());
    }
    eda_storage::load(path)?;
    ProcessCommand::new(gui_executable()?)
        .arg(fs::canonicalize(path)?)
        .spawn()?;
    println!("Editor iniciado para {}", path.display());
    Ok(())
}

fn gui_executable() -> Result<PathBuf, Box<dyn Error>> {
    let executable_name = if cfg!(windows) {
        "protocraft.exe"
    } else {
        "protocraft"
    };
    let gui = env::current_exe()?.with_file_name(executable_name);
    if !gui.is_file() {
        return Err(format!(
            "Aplicativo desktop não encontrado ao lado da CLI: {}. Compile/instale os dois executáveis juntos.",
            gui.display()
        )
        .into());
    }
    Ok(gui)
}

fn command_help(command: &str) -> Option<&'static str> {
    match command {
        "templates" => Some("Uso: protocraft-cli templates\nLista os modelos disponíveis em JSON."),
        "new" | "create" => Some(
            "Uso: protocraft-cli new --output <arquivo.protocraft> [--template <id>] [--name <nome>] [--open]\nPara 555-led-1hz: [--ra-ohms R] [--rb-ohms R] [--c-farads F | --frequency-hz Hz] [--led-resistor-ohms R]. Não substitui arquivos existentes; valores em unidades SI.",
        ),
        "open" => Some(
            "Uso: protocraft-cli open <arquivo.protocraft>\nAbre o projeto no aplicativo desktop distribuído ao lado da CLI.",
        ),
        "engines" => {
            Some("Uso: protocraft-cli engines\nLista os motores próprios de simulação em JSON.")
        }
        "components" => {
            Some("Uso: protocraft-cli components\nLista componentes e parâmetros padrão em JSON.")
        }
        "info" => Some(
            "Uso: protocraft-cli info <arquivo.protocraft>\nImprime um resumo JSON do projeto, redes, ERC e estado de importação.",
        ),
        "commands" => Some(
            "Uso: protocraft-cli commands [--filter <termo>]\nLista os comandos de edição disponíveis para run.",
        ),
        "run" => Some(
            "Uso: protocraft-cli run <arquivo.protocraft> (--actions <acoes.json> | --cmd <id> --params '<json>' [...]) [--output <novo.protocraft>]\nAplica um lote atômico pelo Engine compartilhado com a interface. Sem --output, apenas informa o resultado em JSON.",
        ),
        "validate" => Some(
            "Uso: protocraft-cli validate <arquivo.protocraft>\nValida o formato e executa o ERC básico; códigos de saída diferentes de zero indicam erro.",
        ),
        "netlist" => Some(
            "Uso: protocraft-cli netlist <arquivo.protocraft>\nExibe redes e pinos conectados.",
        ),
        "simulate" => Some(
            "Uso: protocraft-cli simulate <arquivo.protocraft> [--engine stable|dynamic|mixed] [--transient passo_s fim_s] [--csv arquivo.csv]\nSem --transient executa análise DC.",
        ),
        "spice" => Some(
            "Uso: protocraft-cli spice <arquivo.protocraft> [--transient passo_s fim_s] [--output arquivo.cir]\nExporta uma netlist SPICE; não executa um simulador externo.",
        ),
        "examples" => Some(
            "Uso: protocraft-cli examples <pasta>\nGrava os exemplos originais do ProtoCraft na pasta indicada.",
        ),
        "import-proteus" => Some(
            "Uso: protocraft-cli import-proteus <arquivo.pdsprj|arquivo.dsn> --output <arquivo.protocraft> [--report <relatorio.json>]\nImportação experimental e parcial; confira sempre os avisos.",
        ),
        _ => None,
    }
}

fn run_info(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args.len() != 1 {
        return Err("Uso: protocraft-cli info <arquivo.protocraft>".into());
    }
    let project = eda_storage::load(&args[0])?;
    println!(
        "{}",
        serde_json::to_string_pretty(&project_summary(&project))?
    );
    Ok(())
}

fn project_summary(project: &eda_core::Project) -> serde_json::Value {
    let netlist = eda_netlist::build_netlist(project);
    let diagnostics = eda_netlist::erc(project, &netlist);
    let errors = diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.severity, eda_netlist::Severity::Error))
        .count();
    let warnings = diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.severity, eda_netlist::Severity::Warning))
        .count();
    let import = project.import_info.as_ref().map(|info| {
        serde_json::json!({
            "source_format": info.source_format,
            "electrical_complete": info.electrical_complete,
            "notes": info.notes,
        })
    });
    serde_json::json!({
        "name": project.name,
        "format_version": project.format_version,
        "components": project.components.len(),
        "wires": project.wires.len(),
        "junctions": project.junctions.len(),
        "labels": project.labels.len(),
        "nets": netlist.nets.len(),
        "erc": { "errors": errors, "warnings": warnings },
        "import": import,
    })
}

fn run_commands(args: &[String]) -> Result<(), Box<dyn Error>> {
    let filter = match args {
        [] => None,
        [flag, term] if flag == "--filter" && !term.trim().is_empty() => Some(term.to_lowercase()),
        _ => return Err("Uso: protocraft-cli commands [--filter <termo>]".into()),
    };
    let commands: Vec<_> = COMMANDS
        .iter()
        .filter(|command| {
            filter.as_ref().is_none_or(|filter| {
                command.id.contains(filter) || command.description.to_lowercase().contains(filter)
            })
        })
        .map(|command| {
            serde_json::json!({
                "id": command.id,
                "description": command.description,
                "params": command.params,
            })
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&commands)?);
    Ok(())
}

const MAX_ACTION_FILE_BYTES: u64 = 1024 * 1024;
const MAX_ACTIONS: usize = 1_000;

fn read_actions(path: &Path) -> Result<Vec<Action>, Box<dyn Error>> {
    let file = fs::File::open(path)?;
    if file.metadata()?.len() > MAX_ACTION_FILE_BYTES {
        return Err("O arquivo de ações excede 1 MiB".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_ACTION_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_ACTION_FILE_BYTES {
        return Err("O arquivo de ações excede 1 MiB".into());
    }
    Ok(parse_actions(&bytes)?)
}

fn run_actions(args: &[String]) -> Result<(), Box<dyn Error>> {
    let input = args
        .first()
        .filter(|arg| !arg.starts_with("--"))
        .ok_or("Informe o projeto .protocraft de entrada")?;
    let mut action_file = None;
    let mut output = None;
    let mut actions = Vec::new();
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--actions" if action_file.is_none() => {
                action_file = Some(PathBuf::from(
                    args.get(index + 1)
                        .filter(|value| !value.starts_with("--"))
                        .ok_or("Informe o arquivo JSON de ações")?,
                ));
                index += 2;
            }
            "--cmd" => {
                let id = args
                    .get(index + 1)
                    .filter(|value| !value.starts_with("--"))
                    .ok_or("Informe o identificador após --cmd")?;
                if args.get(index + 2).map(String::as_str) != Some("--params") {
                    return Err("Cada --cmd requer --params '<objeto JSON>'".into());
                }
                let params = args.get(index + 3).ok_or("Informe os parâmetros JSON")?;
                actions.push(parse_action(id, params)?);
                index += 4;
            }
            "--params" => return Err("--params só pode aparecer após --cmd <id>".into()),
            "--output" | "--out" if output.is_none() => {
                output = Some(PathBuf::from(
                    args.get(index + 1)
                        .filter(|value| !value.starts_with("--"))
                        .ok_or("Informe o arquivo .protocraft de saída")?,
                ));
                index += 2;
            }
            option => {
                return Err(format!("Opção desconhecida ou repetida para run: {option}").into());
            }
        }
    }
    if action_file.is_some() && !actions.is_empty() {
        return Err("Use --actions ou --cmd, não os dois formatos no mesmo run".into());
    }
    if let Some(path) = &action_file {
        actions = read_actions(path)?;
    }
    if actions.is_empty() {
        return Err("run requer ao menos uma ação".into());
    }
    if actions.len() > MAX_ACTIONS {
        return Err(format!("run aceita até {MAX_ACTIONS} ações por vez").into());
    }

    let source = fs::canonicalize(input)?;
    let output_resolved = output.as_deref().map(resolved_destination).transpose()?;
    if let (Some(output_path), Some(output_resolved)) = (&output, &output_resolved) {
        if !output_path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("protocraft"))
        {
            return Err("O arquivo de saída deve usar a extensão .protocraft".into());
        }
        let backup_resolved = resolved_destination(&eda_storage::backup_path(output_path))?;
        if same_path(&source, output_resolved) || same_path(&source, &backup_resolved) {
            return Err("A saída e o backup devem preservar o projeto de entrada".into());
        }
        if let Some(action_path) = &action_file {
            let action_source = fs::canonicalize(action_path)?;
            if same_path(&action_source, output_resolved)
                || same_path(&action_source, &backup_resolved)
            {
                return Err("A saída não pode substituir o arquivo JSON de ações".into());
            }
        }
    }

    let project = eda_storage::load(input)?;
    let mut engine = eda_core::Engine::new(project)?;
    let count = actions.len();
    engine.execute(eda_core::Command::Batch(
        actions
            .into_iter()
            .map(Action::into_command)
            .collect::<Result<Vec<_>, _>>()?,
    ))?;
    if let Some(path) = &output {
        eda_storage::save(path, engine.project())?;
    }
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "ok": true,
            "actions": count,
            "output": output,
            "project": project_summary(engine.project()),
        }))?
    );
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
        .ok_or("Destino deve ter um nome de arquivo")?;
    Ok(fs::canonicalize(parent)?.join(name))
}

fn same_path(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        left.as_os_str().to_string_lossy().to_lowercase()
            == right.as_os_str().to_string_lossy().to_lowercase()
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

fn run_proteus_import(args: &[String]) -> Result<(), Box<dyn Error>> {
    let input = args
        .first()
        .filter(|s| !s.starts_with("--"))
        .ok_or("Informe o projeto Proteus .pdsprj ou .dsn")?;
    let mut output = None;
    let mut report = None;
    let mut index = 1;
    while index < args.len() {
        let value = args.get(index + 1).filter(|s| !s.starts_with("--"));
        match args[index].as_str() {
            "--output" if output.is_none() => {
                output = Some(value.ok_or("Informe o arquivo .protocraft de saída")?)
            }
            "--report" if report.is_none() => {
                report = Some(value.ok_or("Informe o arquivo JSON do relatório")?)
            }
            option => {
                return Err(format!(
                    "Opção desconhecida ou repetida para import-proteus: {option}"
                )
                .into());
            }
        }
        index += 2;
    }
    let output = Path::new(output.ok_or("Importação requer --output <arquivo.protocraft>")?);
    if !output
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("protocraft"))
    {
        return Err("O arquivo de saída deve usar a extensão .protocraft".into());
    }
    let source = fs::canonicalize(input)?;
    let destination = resolved_destination(output)?;
    let backup = resolved_destination(&eda_storage::backup_path(output))?;
    if same_path(&source, &destination) || same_path(&source, &backup) {
        return Err("Saída e backup devem ser diferentes do arquivo Proteus de entrada".into());
    }
    let report_path = report.map(Path::new);
    if let Some(path) = report_path {
        let resolved = resolved_destination(path)?;
        if same_path(&resolved, &source)
            || same_path(&resolved, &destination)
            || same_path(&resolved, &backup)
        {
            return Err("O relatório deve ser diferente da entrada, da saída e do backup".into());
        }
        if path.exists() {
            return Err("O relatório já existe; escolha um novo caminho para preservá-lo".into());
        }
    }
    let imported = eda_import::import_proteus(input)?;
    let report_json = serde_json::to_vec_pretty(&imported.report)?;
    eda_storage::save(output, &imported.project)?;
    println!("Projeto importado salvo: {}", output.display());
    println!(
        "Importação experimental {}: {} componentes, {} fios, {} junções e {} rótulos",
        imported.report.format,
        imported.report.components,
        imported.report.wires,
        imported.report.junctions,
        imported.report.labels
    );
    if !imported.report.electrical_complete {
        println!(
            "Aviso: a importação não preservou todo o significado elétrico; revise o relatório antes de simular."
        );
    }
    if !imported.report.unsupported_components.is_empty() {
        println!(
            "Componentes sem suporte completo: {}",
            imported.report.unsupported_components.join(", ")
        );
    }
    for warning in &imported.report.warnings {
        println!("Aviso: {warning}");
    }
    if let Some(path) = report_path {
        let result = (|| -> Result<(), std::io::Error> {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?;
            file.write_all(&report_json)?;
            file.write_all(b"\n")?;
            file.sync_all()
        })();
        if let Err(error) = result {
            return Err(
                format!("Projeto salvo, mas não foi possível gravar o relatório: {error}").into(),
            );
        }
        println!("Relatório salvo: {}", path.display());
    }
    Ok(())
}
