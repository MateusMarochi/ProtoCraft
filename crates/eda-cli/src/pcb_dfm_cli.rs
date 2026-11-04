use eda_pcb::dfm::ManufacturingLimits;
use std::{
    collections::BTreeSet,
    error::Error,
    fs,
    io::{Read, Write},
    path::Path,
};

pub fn help() -> String {
    eda_i18n::text(
        "Uso: protocraft-cli pcb-dfm <projeto.protocraft> [--limits limites.json] [--output relatorio.json]\nUse pcb-dfm --default-limits para imprimir os limites genéricos em nanômetros. A revisão não certifica fabricação; ajuste os limites ao processo contratado.",
    )
}

pub fn read_limits(path: &Path) -> Result<ManufacturingLimits, Box<dyn Error>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?.take(65_537).read_to_end(&mut bytes)?;
    if bytes.len() > 65_536 {
        return Err(eda_i18n::text("O arquivo de limites DFM excede 64 KiB.").into());
    }
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args == ["--help"] {
        println!("{}", help());
        return Ok(());
    }
    if args == ["--default-limits"] {
        println!(
            "{}",
            serde_json::to_string_pretty(&ManufacturingLimits::default())?
        );
        return Ok(());
    }
    let input = args
        .first()
        .filter(|p| !p.starts_with('-'))
        .ok_or_else(help)?;
    let mut limits = ManufacturingLimits::default();
    let mut output = None;
    let mut seen = BTreeSet::new();
    let mut index = 1;
    while index < args.len() {
        let option = args[index].as_str();
        if !seen.insert(option) {
            return Err(eda_i18n::text("Opção DFM repetida.").into());
        }
        let value = args
            .get(index + 1)
            .filter(|p| !p.starts_with('-'))
            .ok_or_else(help)?;
        match option {
            "--limits" => {
                limits = read_limits(Path::new(value))?;
            }
            "--output" => output = Some(Path::new(value)),
            _ => return Err(help().into()),
        }
        index += 2;
    }
    let project = eda_storage::load(Path::new(input))?;
    let report = eda_pcb::dfm::check(&project, &limits)?;
    let data = serde_json::to_string_pretty(&report)?;
    if let Some(output) = output {
        if output
            .extension()
            .is_none_or(|ext| !ext.eq_ignore_ascii_case("json"))
        {
            return Err(eda_i18n::text("O relatório DFM deve usar a extensão .json.").into());
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?;
        file.write_all(data.as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()?;
    }
    println!("{data}");
    if report.errors() > 0 || !report.complete {
        return Err(
            eda_i18n::text("A revisão DFM encontrou pendências; consulte o relatório.").into(),
        );
    }
    Ok(())
}
