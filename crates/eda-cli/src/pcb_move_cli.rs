use eda_core::{Command, ComponentId, Engine, NM_PER_MM, NmPoint, PHYSICAL_LIMIT};
use std::{error::Error, fs, path::Path};

const USAGE: &str = "Uso: protocraft-cli pcb-move <projeto.protocraft> --component <id> --x-mm <x> --y-mm <y> --output <novo.protocraft>";

fn coordinate(value: &str) -> Result<i64, Box<dyn Error>> {
    let number: f64 = value.parse().map_err(|_| "Coordenada física inválida.")?;
    let nm = number * NM_PER_MM as f64;
    if !nm.is_finite() || nm.abs() > PHYSICAL_LIMIT as f64 {
        return Err("Coordenada física inválida.".into());
    }
    Ok(nm.round() as i64)
}

pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args == ["--help"] {
        println!("{}", eda_i18n::text(USAGE));
        return Ok(());
    }
    if args.len() != 9 || args[0].starts_with('-') {
        return Err(USAGE.into());
    }
    let mut component = None;
    let mut x = None;
    let mut y = None;
    let mut output = None;
    for pair in args[1..].as_chunks::<2>().0 {
        match pair[0].as_str() {
            "--component" if component.is_none() => {
                component = Some(
                    pair[1]
                        .parse::<u64>()
                        .map_err(|_| "ID de componente inválido.")?,
                )
            }
            "--x-mm" if x.is_none() => x = Some(coordinate(&pair[1])?),
            "--y-mm" if y.is_none() => y = Some(coordinate(&pair[1])?),
            "--output" if output.is_none() => output = Some(Path::new(&pair[1])),
            _ => return Err(USAGE.into()),
        }
    }
    let (id, x, y, output) = match (component, x, y, output) {
        (Some(id), Some(x), Some(y), Some(output)) => (ComponentId(id), x, y, output),
        _ => return Err(USAGE.into()),
    };
    if output
        .extension()
        .is_none_or(|ext| !ext.eq_ignore_ascii_case("protocraft"))
    {
        return Err("Use a extensão .protocraft".into());
    }
    let input = Path::new(&args[0]);
    if super::same_path(
        &fs::canonicalize(input)?,
        &super::resolved_destination(output)?,
    ) || output.exists()
    {
        return Err("O arquivo de saída já existe; escolha outro nome.".into());
    }
    let project = eda_storage::load(input)?;
    let old = project
        .board
        .as_ref()
        .and_then(|b| b.placements.get(&id))
        .ok_or("Componente sem posicionamento PCB")?;
    let mut target = *old;
    target.position = NmPoint::new(x, y);
    let commands = eda_pcb::placement::plan_placement(&project, id, target)?;
    let updated_tracks = commands
        .iter()
        .filter(|c| matches!(c, Command::UpdateTrack { .. }))
        .count();
    let mut engine = Engine::new(project)?;
    engine.execute(Command::Batch(commands))?;
    eda_storage::save(output, engine.project())?;
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"output": output, "component": id.0, "placement": target, "updated_tracks": updated_tracks, "atomic": true})
        )?
    );
    Ok(())
}
