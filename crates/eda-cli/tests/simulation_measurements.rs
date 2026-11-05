use std::{path::Path, process::Command};
fn cli(args: &[&str], language: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_protocraft-cli"))
        .args(args)
        .env("PROTOCRAFT_LANGUAGE", language)
        .output()
        .unwrap()
}
fn path(p: &Path) -> &str {
    p.to_str().unwrap()
}

#[test]
fn measurement_cli_is_analytical_differential_and_preserves_files() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("divider.protocraft");
    eda_storage::create_new(&input, &eda_core::examples::voltage_divider()).unwrap();
    let original = std::fs::read(&input).unwrap();
    for language in ["en", "pt-BR"] {
        let args = [
            "simulate-measure",
            path(&input),
            "--trace",
            "V(VOUT)",
            "--step",
            "0.00001",
            "--stop",
            "0.001",
            "--from",
            "0.00001",
            "--threshold",
            "2.5",
        ];
        let output = cli(&args, language);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!((json["measurement"]["mean"].as_f64().unwrap() - 5.).abs() < 1e-9);
        assert!((json["measurement"]["rms"].as_f64().unwrap() - 5.).abs() < 1e-9);
        assert!(json["measurement"]["frequency_hz"].is_null());
        let mut differential = args.to_vec();
        differential.extend(["--reference", "V(VIN)"]);
        let output = cli(&differential, language);
        assert!(output.status.success());
        let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!((json["measurement"]["mean"].as_f64().unwrap() + 5.).abs() < 1e-9);
        let destination = temp.path().join(format!("{language}.json"));
        let mut saved = args.to_vec();
        saved.extend(["--output", path(&destination)]);
        assert!(cli(&saved, language).status.success());
        let bytes = std::fs::read(&destination).unwrap();
        assert!(!cli(&saved, language).status.success());
        assert_eq!(std::fs::read(destination).unwrap(), bytes);
        for suffix in [["--step", "0.01"], ["--bogus", "1"], ["--to", "NaN"]] {
            let mut invalid = args.to_vec();
            invalid.extend(suffix);
            assert!(!cli(&invalid, language).status.success());
        }
        let bad = cli(
            &[
                "simulate-measure",
                path(&input),
                "--trace",
                "V(VOUT)",
                "--step",
                "abc",
                "--stop",
                "0.001",
            ],
            language,
        );
        assert!(!bad.status.success());
        let error = String::from_utf8(bad.stderr).unwrap();
        assert!(!error.contains("invalid float literal"));
        assert!(!error.contains("measurement.invalid_number"));
        let help = cli(&["simulate-measure", "--help"], language);
        assert!(help.status.success());
        assert!(
            !String::from_utf8(help.stdout)
                .unwrap()
                .contains("measurement.cli_help")
        );
    }
    assert_eq!(std::fs::read(input).unwrap(), original);
}

#[test]
fn mixed_timer_measurement_matches_analytical_frequency() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("timer.protocraft");
    eda_storage::create_new(&input, &eda_core::examples::timer_astable()).unwrap();
    let result = cli(
        &[
            "simulate-measure",
            path(&input),
            "--trace",
            "V(VOUT)",
            "--step",
            "0.00001",
            "--stop",
            "0.03",
            "--from",
            "0.005",
            "--threshold",
            "2.5",
            "--engine",
            "mixed",
        ],
        "pt-BR",
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    let frequency = report["measurement"]["frequency_hz"].as_f64().unwrap();
    let expected = 1.0 / (std::f64::consts::LN_2 * 30_000.0 * 1e-7);
    assert!((frequency - expected).abs() / expected < 0.015);
    assert!(
        report["measurement"]["rising_crossings_s"]
            .as_array()
            .unwrap()
            .len()
            >= 2
    );
}
