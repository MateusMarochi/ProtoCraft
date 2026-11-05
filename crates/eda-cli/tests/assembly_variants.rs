use std::{fs, process::Command};
#[test]
fn variant_cli_preserves_source_and_rejects_existing_output() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("board.protocraft");
    let config = temp.path().join("variant.json");
    let output = temp.path().join("report.json");
    eda_storage::create_new(&source, &eda_core::Project::empty("variant")).unwrap();
    fs::write(&config, r#"{"name":"base","do_not_populate":[]}"#).unwrap();
    let before = fs::read(&source).unwrap();
    let invoke = || {
        Command::new(env!("CARGO_BIN_EXE_protocraft-cli"))
            .arg("pcb-assembly-variant")
            .arg(&source)
            .arg("--config")
            .arg(&config)
            .arg("--output")
            .arg(&output)
            .output()
            .unwrap()
    };
    let result = invoke();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
    assert_eq!(report["variant"], "base");
    let saved = fs::read(&output).unwrap();
    assert!(!invoke().status.success());
    assert_eq!(fs::read(output).unwrap(), saved);
    assert_eq!(fs::read(source).unwrap(), before);
}

#[test]
fn invalid_variant_options_do_not_create_output() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("board.protocraft");
    let config = temp.path().join("variant.json");
    let output = temp.path().join("report.json");
    eda_storage::create_new(&source, &eda_core::Project::empty("variant")).unwrap();
    for (json, extra) in [
        (r#"{"name":"x","unknown":true}"#.to_owned(), vec![]),
        (r#"{"name":"x"}"#.to_owned(), vec!["--format", "invalid"]),
        (
            r#"{"name":"x"}"#.to_owned(),
            vec!["--config", "another.json"],
        ),
        (r#"{"name":"x"}"#.to_owned(), vec!["--unknown", "value"]),
        (" ".repeat(65_537), vec![]),
        (
            r#"{"name":"x","do_not_populate":["R99"]}"#.to_owned(),
            vec![],
        ),
    ] {
        fs::write(&config, json).unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_protocraft-cli"))
            .arg("pcb-assembly-variant")
            .arg(&source)
            .arg("--config")
            .arg(&config)
            .arg("--output")
            .arg(&output)
            .args(extra)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(!output.exists());
    }
}

#[test]
fn invalid_variant_json_has_translated_error() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("invalid.json");
    let output = temp.path().join("report.json");
    fs::write(&config, r#"{"name":"x","unknown":true}"#).unwrap();
    for (language, expected) in [
        ("pt-BR", "Configuração de variante inválida"),
        ("en", "Invalid assembly variant configuration"),
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_protocraft-cli"))
            .args(["pcb-assembly-variant", "unused.protocraft", "--config"])
            .arg(&config)
            .arg("--output")
            .arg(&output)
            .env("PROTOCRAFT_LANGUAGE", language)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(String::from_utf8(result.stderr).unwrap().contains(expected));
        assert!(!output.exists());
    }
}
