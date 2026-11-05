use std::{collections::BTreeSet, error::Error, fs, io::Read, path::Path};

pub fn help() -> String {
    eda_i18n::text("assembly_variant.cli_help")
}

pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args == ["--help"] {
        println!("{}", help());
        return Ok(());
    }
    let input = args
        .first()
        .filter(|s| !s.starts_with('-'))
        .ok_or_else(help)?;
    let mut config = None;
    let mut output = None;
    let mut format = "json";
    let mut seen = BTreeSet::new();
    let mut i = 1;
    while i < args.len() {
        let flag = args[i].as_str();
        if !seen.insert(flag) {
            return Err(help().into());
        }
        let value = args
            .get(i + 1)
            .filter(|s| !s.starts_with('-'))
            .ok_or_else(help)?;
        match flag {
            "--config" => config = Some(value),
            "--output" => output = Some(value),
            "--format" => format = value,
            _ => return Err(help().into()),
        }
        i += 2;
    }
    if !matches!(format, "json" | "bom-csv" | "placements-csv") {
        return Err(help().into());
    }
    let mut bytes = Vec::new();
    fs::File::open(config.ok_or_else(help)?)?
        .take(65537)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 65536 {
        return Err(eda_i18n::text("assembly_variant.config_too_large").into());
    }
    let config = serde_json::from_slice(&bytes)
        .map_err(|_| eda_i18n::text("assembly_variant.invalid_json"))?;
    let project = eda_storage::load(Path::new(input))?;
    let report = eda_pcb::assembly_variants::variant_assembly_report(&project, &config)?;
    let data = match format {
        "bom-csv" => eda_pcb::assembly::bom_csv(&report.assembly),
        "placements-csv" => eda_pcb::assembly::placements_csv(&report.assembly),
        _ => serde_json::to_string_pretty(&report)?,
    };
    if let Some(output) = output {
        eda_storage::write_new_report(Path::new(output), data.as_bytes())?;
    } else {
        println!("{data}");
    }
    Ok(())
}
