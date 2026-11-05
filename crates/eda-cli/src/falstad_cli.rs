//! Importação explícita de circuitos Falstad para o formato nativo.
use std::{error::Error, path::Path, process::Command};

pub fn help() -> String {
    eda_i18n::text("falstad.cli.help")
}

pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args == ["--help"] {
        println!("{}", help());
        return Ok(());
    }
    let mut source = None;
    let mut output = None;
    let mut open = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--output" if output.is_none() => {
                output = Some(args.get(index + 1).ok_or_else(help)?.as_str());
                index += 2;
            }
            "--open" if !open => {
                open = true;
                index += 1;
            }
            value if source.is_none() && !value.starts_with("--") => {
                source = Some(value);
                index += 1;
            }
            _ => return Err(help().into()),
        }
    }
    let source = source.ok_or_else(help)?;
    let output = output.ok_or_else(help)?;
    let path = Path::new(output);
    if !path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("protocraft"))
    {
        return Err(eda_i18n::text("falstad.cli.extension").into());
    }
    if path.exists() {
        return Err(eda_i18n::text("falstad.cli.exists").into());
    }
    let gui = if open {
        Some(super::gui_executable()?)
    } else {
        None
    };
    let text = eda_storage::falstad_source::read_source(source)?;
    let name = if source.starts_with("https://") {
        "Falstad"
    } else {
        Path::new(source)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Falstad")
    };
    let imported = eda_import::falstad::import_text(&text, name)?;
    eda_storage::create_new(path, &imported.project)?;
    let mut opened = false;
    if let Some(gui) = gui {
        Command::new(gui)
            .arg(std::fs::canonicalize(path)?)
            .spawn()?;
        opened = true;
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "source_format": imported.report.format,
            "output": output,
            "editor_started": opened,
            "report": imported.report,
        }))?
    );
    Ok(())
}
