use std::{error::Error, fs, path::Path};
const USAGE: &str = "Uso: protocraft-cli pcb-cleanup <projeto.protocraft> --output <novo.protocraft> [--merge-collinear]";

pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args == ["--help"] {
        println!("{}", eda_i18n::text(USAGE));
        return Ok(());
    }
    let input = args
        .first()
        .filter(|value| !value.starts_with('-'))
        .ok_or_else(|| eda_i18n::text(USAGE))?;
    let mut output = None;
    let mut options = eda_pcb::cleanup::CleanupOptions::default();
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--merge-collinear" if !options.merge_collinear => {
                options.merge_collinear = true;
                index += 1;
            }
            "--output" if output.is_none() => {
                output = Some(
                    args.get(index + 1)
                        .filter(|value| !value.starts_with('-'))
                        .ok_or_else(|| eda_i18n::text(USAGE))?,
                );
                index += 2;
            }
            _ => return Err(eda_i18n::text(USAGE).into()),
        }
    }
    let input = Path::new(input);
    let output = Path::new(output.ok_or_else(|| eda_i18n::text(USAGE))?);
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
    let plan = eda_pcb::cleanup::plan_with_options(&project, &options)?;
    let mut engine = eda_core::Engine::new(project)?;
    if !plan.commands.is_empty() {
        engine.execute(eda_core::Command::Batch(plan.commands))?;
    }
    eda_storage::create_new(output, engine.project())?;
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"output": output, "removed_tracks": plan.removed_tracks,
                "merged_tracks": plan.merged_tracks, "atomic": true})
        )?
    );
    Ok(())
}
