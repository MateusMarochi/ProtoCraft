use eda_simulation::{
    Analysis, SimulationControl, SimulationEngine, SimulationRequest,
    sweep::{SweepConfig, SweepScale, dc_sweep},
};
use std::{collections::BTreeMap, error::Error};
pub fn help() -> String {
    eda_i18n::text("sweep.help")
}
pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args == ["--help"] {
        println!("{}", help());
        return Ok(());
    }
    let input = args
        .first()
        .filter(|v| !v.starts_with("--"))
        .ok_or_else(help)?;
    let mut options = BTreeMap::new();
    let mut index = 1;
    while index < args.len() {
        let key = &args[index];
        if ![
            "--component",
            "--start",
            "--stop",
            "--points",
            "--scale",
            "--engine",
            "--output",
        ]
        .contains(&key.as_str())
            || options.contains_key(key)
        {
            return Err(help().into());
        }
        let value = args
            .get(index + 1)
            .filter(|v| !v.starts_with("--"))
            .ok_or_else(help)?;
        options.insert(key.clone(), value.clone());
        index += 2;
    }
    let required = |key: &str| options.get(key).ok_or_else(help);
    let project = eda_storage::load(input)?;
    let reference = required("--component")?;
    let component = project
        .components
        .values()
        .find(|c| &c.reference == reference)
        .ok_or_else(|| eda_i18n::text("sweep.invalid"))?
        .id;
    let config = SweepConfig {
        component,
        start: required("--start")?
            .parse()
            .map_err(|_| eda_i18n::text("sweep.invalid"))?,
        stop: required("--stop")?
            .parse()
            .map_err(|_| eda_i18n::text("sweep.invalid"))?,
        points: required("--points")?
            .parse()
            .map_err(|_| eda_i18n::text("sweep.invalid"))?,
        scale: match options
            .get("--scale")
            .map(String::as_str)
            .unwrap_or("linear")
        {
            "linear" => SweepScale::Linear,
            "log" => SweepScale::Log,
            _ => return Err(help().into()),
        },
    };
    let engine = SimulationEngine::parse(
        options
            .get("--engine")
            .map(String::as_str)
            .unwrap_or("stable"),
    )?;
    let report = dc_sweep(
        &SimulationRequest {
            project,
            revision: 0,
            analysis: Analysis::Dc,
        },
        &config,
        engine,
        &SimulationControl::default(),
    )?;
    let bytes = serde_json::to_vec_pretty(&report)?;
    if let Some(path) = options.get("--output") {
        eda_storage::write_new_report(std::path::Path::new(path), &bytes)?;
    } else {
        println!("{}", String::from_utf8(bytes)?);
    }
    Ok(())
}
