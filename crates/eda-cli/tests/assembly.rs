use std::{path::Path, process::Command};

fn source() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/555-led-1hz-pcb.protocraft")
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_protocraft-cli"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn assembly_exports_json_bom_and_positions_and_preserves_input() {
    let source = source();
    let before = std::fs::read(&source).unwrap();
    let output = run(&["pcb-assembly", source.to_str().unwrap()]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(!report["placements"].as_array().unwrap().is_empty());
    assert!(!report["bom"].as_array().unwrap().is_empty());
    let dir = tempfile::tempdir().unwrap();
    for (format, header) in [
        ("bom-csv", "references,quantity"),
        ("placements-csv", "reference,"),
    ] {
        let path = dir.path().join(format!("{format}.csv"));
        let args = [
            "pcb-assembly",
            source.to_str().unwrap(),
            "--format",
            format,
            "--output",
            path.to_str().unwrap(),
        ];
        assert!(run(&args).status.success());
        let bytes = std::fs::read(&path).unwrap();
        assert!(std::str::from_utf8(&bytes).unwrap().starts_with(header));
        assert!(!run(&args).status.success());
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
    assert_eq!(std::fs::read(source).unwrap(), before);
}

#[test]
fn assembly_rejects_bad_options_and_input_alias_without_writing() {
    let source = source();
    let source = source.to_str().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("unused.csv");
    for tail in [
        vec!["--format", "unknown"],
        vec!["--format", "json", "--format", "bom-csv"],
        vec!["--invalid", "json"],
        vec!["--output"],
    ] {
        let mut args = vec!["pcb-assembly", source, "--output", output.to_str().unwrap()];
        args.extend(tail);
        assert!(!run(&args).status.success());
        assert!(!output.exists());
    }
    let before = std::fs::read(source).unwrap();
    assert!(
        !run(&["pcb-assembly", source, "--output", source])
            .status
            .success()
    );
    assert_eq!(std::fs::read(source).unwrap(), before);
}
