use std::error::Error;

pub fn run(command: &str, args: &[String]) -> Result<(), Box<dyn Error>> {
    if command == "pcb-sync" {
        return sync(args);
    }
    if args == ["--help"] {
        println!(
            "{}",
            tr!(
                "Uso: protocraft-cli pcb-sync-report <projeto.protocraft>\nInspeciona footprints e posicionamentos sem alterar o projeto."
            )
        );
        return Ok(());
    }
    if args.len() != 1 || args[0].starts_with('-') {
        return Err("Uso: protocraft-cli pcb-sync-report <projeto.protocraft>".into());
    }
    let project = eda_storage::load(&args[0])?;
    let mut components = Vec::with_capacity(project.components.len());
    let (mut placed, mut unplaced, mut without_footprint) = (0, 0, 0);
    for component in project.components.values() {
        let placement = project
            .board
            .as_ref()
            .and_then(|board| board.placements.get(&component.id));
        let status = if component.footprint.is_none() {
            without_footprint += 1;
            "without_footprint"
        } else if placement.is_some() {
            placed += 1;
            "placed"
        } else {
            unplaced += 1;
            "unplaced"
        };
        components.push(serde_json::json!({
            "id": component.id.0,
            "reference": component.reference,
            "footprint": component.footprint.as_ref().map(|footprint| &footprint.id),
            "placement": placement,
            "status": status,
        }));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "board_present": project.board.is_some(),
            "counts": {"placed": placed, "unplaced": unplaced, "without_footprint": without_footprint},
            "components": components,
        }))?
    );
    Ok(())
}

fn sync(args: &[String]) -> Result<(), Box<dyn Error>> {
    use std::{fs, path::Path};
    if args == ["--help"] {
        println!(
            "{}",
            tr!(
                "Uso: protocraft-cli pcb-sync <projeto.protocraft> --output <novo.protocraft>\nAdiciona footprints não posicionados e preserva o projeto original. Confira o DRC."
            )
        );
        return Ok(());
    }
    if args.len() != 3
        || args[0].starts_with('-')
        || args[1] != "--output"
        || args[2].starts_with('-')
    {
        return Err(
            "Uso: protocraft-cli pcb-sync <projeto.protocraft> --output <novo.protocraft>".into(),
        );
    }
    let input = Path::new(&args[0]);
    let output = Path::new(&args[2]);
    if output
        .extension()
        .is_none_or(|extension| !extension.eq_ignore_ascii_case("protocraft"))
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
    let plan = eda_pcb::sync::plan(&project)?;
    let mut engine = eda_core::Engine::new(project)?;
    if !plan.commands.is_empty() {
        engine.execute(eda_core::Command::Batch(plan.commands))?;
    }
    let report = eda_pcb::check(engine.project())?;
    eda_storage::save(output, engine.project())?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "output": output, "added": plan.added, "without_footprint": plan.without_footprint,
            "drc": report, "atomic": true,
        }))?
    );
    Ok(())
}
