use eda_simulation::{
    Analysis, SimulationControl, SimulationEngine, SimulationRequest,
    measurements::{MeasurementOptions, measure_difference, measure_trace},
};
use std::{collections::BTreeMap, error::Error, path::Path};

pub fn help() -> String {
    eda_i18n::text("measurement.cli_help")
}

pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args == ["--help"] {
        println!("{}", help());
        return Ok(());
    }
    let input = args
        .first()
        .filter(|v| !v.starts_with('-'))
        .ok_or_else(help)?;
    let mut options = BTreeMap::new();
    let mut index = 1;
    while index < args.len() {
        let key = args[index].as_str();
        if ![
            "--trace",
            "--reference",
            "--step",
            "--stop",
            "--from",
            "--to",
            "--threshold",
            "--engine",
            "--output",
        ]
        .contains(&key)
        {
            return Err(help().into());
        }
        let value = args
            .get(index + 1)
            .filter(|v| !v.starts_with("--"))
            .ok_or_else(help)?;
        if options.insert(key, value.as_str()).is_some() {
            return Err(help().into());
        }
        index += 2;
    }
    let name = options.get("--trace").ok_or_else(help)?;
    let invalid_number = || eda_i18n::text("measurement.invalid_number");
    let step: f64 = options
        .get("--step")
        .ok_or_else(help)?
        .parse()
        .map_err(|_| invalid_number())?;
    let stop: f64 = options
        .get("--stop")
        .ok_or_else(help)?
        .parse()
        .map_err(|_| invalid_number())?;
    let engine = SimulationEngine::parse(options.get("--engine").copied().unwrap_or("stable"))?;
    let measurement_options = MeasurementOptions {
        from_s: options
            .get("--from")
            .map(|v| v.parse().map_err(|_| invalid_number()))
            .transpose()?,
        to_s: options
            .get("--to")
            .map(|v| v.parse().map_err(|_| invalid_number()))
            .transpose()?,
        threshold: options
            .get("--threshold")
            .map(|v| v.parse().map_err(|_| invalid_number()))
            .transpose()?,
    };
    let project = eda_storage::load(Path::new(input))?;
    let result = eda_simulation::simulate_with_engine(
        &SimulationRequest {
            project,
            revision: 0,
            analysis: Analysis::Transient { step, stop },
        },
        engine,
        &SimulationControl::default(),
    )?;
    let find_trace = |name: &str| {
        result
            .traces
            .iter()
            .find(|t| t.name == name)
            .ok_or_else(|| {
                eda_i18n::format("measurement.trace_missing", &[("trace", name.to_owned())])
            })
    };
    let trace = find_trace(name)?;
    let measurement = if let Some(reference) = options.get("--reference") {
        measure_difference(trace, find_trace(reference)?, &measurement_options)?
    } else {
        measure_trace(trace, &measurement_options)?
    };
    let warnings: Vec<_> = result.warnings.iter().map(|w| eda_i18n::text(w)).collect();
    let mut data = serde_json::to_vec_pretty(
        &serde_json::json!({"revision":result.revision,"engine":engine.id(),"warnings":warnings,"measurement":measurement}),
    )?;
    data.push(b'\n');
    if let Some(output) = options.get("--output") {
        eda_storage::write_new_report(output, &data)?;
    } else {
        print!("{}", String::from_utf8(data)?);
    }
    Ok(())
}
