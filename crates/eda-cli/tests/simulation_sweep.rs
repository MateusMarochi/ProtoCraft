use std::{path::Path, process::Command};
fn cli(args: &[&str], lang: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_protocraft-cli"))
        .args(args)
        .env("PROTOCRAFT_LANGUAGE", lang)
        .output()
        .unwrap()
}
fn path(p: &Path) -> &str {
    p.to_str().unwrap()
}
#[test]
fn cli_sweep_preserves_source_and_existing_output_in_both_languages() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("divider.protocraft");
    eda_storage::save(&input, &eda_core::examples::voltage_divider()).unwrap();
    let original = std::fs::read(&input).unwrap();
    for language in ["en", "pt-BR"] {
        let args = [
            "dc-sweep",
            path(&input),
            "--component",
            "V1",
            "--start",
            "2",
            "--stop",
            "6",
            "--points",
            "3",
        ];
        let result = cli(&args, language);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(report["points"].as_array().unwrap().len(), 3);
        assert_eq!(report["points"][2]["value"], 6.0);
        let output = dir.path().join(format!("{language}.json"));
        let mut with_output = args.to_vec();
        with_output.extend(["--output", path(&output)]);
        assert!(cli(&with_output, language).status.success());
        let saved = std::fs::read(&output).unwrap();
        assert!(!cli(&with_output, language).status.success());
        assert_eq!(std::fs::read(&output).unwrap(), saved);
        for suffix in [
            vec!["--points", "4"],
            vec!["--unknown", "yes"],
            vec!["--scale", "invalid"],
        ] {
            let mut invalid = args.to_vec();
            invalid.extend(suffix);
            assert!(!cli(&invalid, language).status.success());
        }
        let help = cli(&["dc-sweep", "--help"], language);
        assert!(help.status.success());
        let text = String::from_utf8(help.stdout).unwrap();
        assert!(text.contains("dc-sweep"));
        assert!(!text.contains("sweep.help"));
    }
    assert_eq!(std::fs::read(input).unwrap(), original);
}

#[test]
fn invalid_numeric_arguments_are_translated_and_create_no_output() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("divider.protocraft");
    let output = dir.path().join("uncreated.json");
    eda_storage::save(&input, &eda_core::examples::voltage_divider()).unwrap();
    for (start, stop, points) in [
        ("abc", "6", "3"),
        ("2", "abc", "3"),
        ("2", "6", "abc"),
        ("2", "6", "-2"),
    ] {
        let result = cli(
            &[
                "dc-sweep",
                path(&input),
                "--component",
                "V1",
                "--start",
                start,
                "--stop",
                stop,
                "--points",
                points,
                "--output",
                path(&output),
            ],
            "pt-BR",
        );
        assert!(!result.status.success());
        let error = String::from_utf8(result.stderr).unwrap();
        assert!(
            error.contains("Configuração de varredura inválida"),
            "{error}"
        );
        assert!(!error.contains("invalid digit"));
        assert!(!error.contains("invalid float"));
        assert!(!output.exists());
    }
}
