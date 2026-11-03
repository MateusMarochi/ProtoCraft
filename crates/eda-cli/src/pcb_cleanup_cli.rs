use std::{error::Error, fs, path::Path};
const USAGE: &str =
    "Uso: protocraft-cli pcb-cleanup <projeto.protocraft> --output <novo.protocraft>";

pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args == ["--help"] {
        println!("{}", eda_i18n::text(USAGE));
        return Ok(());
    }
    if args.len() != 3
        || args[0].starts_with('-')
        || args[1] != "--output"
        || args[2].starts_with('-')
    {
        return Err(USAGE.into());
    }
    let input = Path::new(&args[0]);
    let output = Path::new(&args[2]);
    if output
        .extension()
        .is_none_or(|ext| !ext.eq_ignore_ascii_case("protocraft"))
    {
        return Err("Use a extensão .protocraft".into());
    }
    if super::same_path(
        &fs::canonicalize(input)?,
        &super::resolved_destination(output)?,
    ) || output.exists()
    {
        return Err("O arquivo de saída já existe; escolha outro nome.".into());
    }
    let project = eda_storage::load(input)?;
    let commands = eda_pcb::cleanup::plan(&project)?;
    let removed = commands.len();
    let mut engine = eda_core::Engine::new(project)?;
    if !commands.is_empty() {
        engine.execute(eda_core::Command::Batch(commands))?;
    }
    eda_storage::save(output, engine.project())?;
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"output": output, "removed_tracks": removed, "atomic": true})
        )?
    );
    Ok(())
}
