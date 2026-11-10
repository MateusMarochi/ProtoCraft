use std::{fs, process::Command};

fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_protocraft-cli"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn imports_legacy_and_xml_files_as_native_projects_without_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    for (extension, source) in [
        (
            "txt",
            "$ 1 0.000005 10.2 50 5 50\nr 0 0 32 0 0 1000\nw 32 0 64 0 0\ng 64 0 64 16 0\n",
        ),
        (
            "xml",
            "<cir><r x=\"0 0 32 0\" f=\"0\" r=\"1000\"/><w x=\"32 0 64 0\" f=\"0\"/><g x=\"64 0 64 16\" f=\"0\"/></cir>",
        ),
    ] {
        let input = dir.path().join(format!("source.{extension}"));
        let output = dir.path().join(format!("out-{extension}.protocraft"));
        fs::write(&input, source).unwrap();
        let run = cli(&[
            "import-falstad",
            input.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
        ]);
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&run.stdout).unwrap();
        assert_eq!(report["report"]["components"], 2);
        assert_eq!(report["report"]["electrical_complete"], true);
        let project = eda_storage::load(&output).unwrap();
        assert_eq!(project.components.len(), 2);
        assert_eq!(fs::read_to_string(&input).unwrap(), source);
        let again = cli(&[
            "import-falstad",
            input.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
        ]);
        assert!(!again.status.success());
        assert_eq!(eda_storage::load(&output).unwrap().components.len(), 2);
    }
}

#[test]
fn compressed_url_imports_offline_and_partial_model_cannot_simulate() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("url.protocraft");
    // Fixture autoral compactada no formato LZ-String de URI.
    let ctz = "CQAgjCAMB0l3BWcMBMIGXeyAoATlIQMxqaZjw5A";
    let link = format!("https://falstad.com/circuit/circuitjs.html?ctz={ctz}");
    let run = cli(&[
        "import-falstad",
        &link,
        "--output",
        output.to_str().unwrap(),
    ]);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(eda_storage::load(&output).unwrap().components.len(), 1);

    let partial = dir.path().join("partial.txt");
    let blocked = dir.path().join("blocked.protocraft");
    fs::write(&partial, "$ 1 0.000005 10.2 50 5 50\nq 0 0 32 0 0\n").unwrap();
    let run = cli(&[
        "import-falstad",
        partial.to_str().unwrap(),
        "--output",
        blocked.to_str().unwrap(),
    ]);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        !eda_storage::load(&blocked)
            .unwrap()
            .import_info
            .unwrap()
            .electrical_complete
    );
    let simulate = cli(&["simulate", blocked.to_str().unwrap()]);
    assert!(!simulate.status.success());
}

#[test]
fn imported_txt_with_annotations_and_probes_runs_through_the_cli_simulator() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("divisor.txt");
    let project = dir.path().join("divisor.protocraft");
    let csv = dir.path().join("divisor.csv");
    let source = r#"<cir>
      <x x="0 0 32 0" f="0" te="divisor de teste"/>
      <v x="0 0 0 64" f="0" wf="0" maxv="10"/>
      <r x="0 0 64 0" f="0" r="1000"/>
      <r x="64 0 64 64" f="0" r="1000"/>
      <w x="0 64 64 64" f="0"/>
      <g x="64 64 64 80" f="0"/>
      <p x="64 0 64 64" f="0"/>
      <O x="64 0 64 0" f="0"/>
    </cir>"#;
    fs::write(&input, source).unwrap();
    let imported = cli(&[
        "import-falstad",
        input.to_str().unwrap(),
        "--output",
        project.to_str().unwrap(),
    ]);
    assert!(
        imported.status.success(),
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&imported.stdout).unwrap();
    assert_eq!(report["report"]["electrical_complete"], true);
    let simulation = cli(&[
        "simulate",
        project.to_str().unwrap(),
        "--engine",
        "stable",
        "--transient",
        "0.00001",
        "0.001",
        "--csv",
        csv.to_str().unwrap(),
    ]);
    assert!(
        simulation.status.success(),
        "{}",
        String::from_utf8_lossy(&simulation.stderr)
    );
    let output = String::from_utf8_lossy(&simulation.stdout);
    assert!(
        output.contains("Simulação concluída:") || output.contains("Simulation complete:"),
        "{output}"
    );
    assert!(output.contains("5.000000000 V"), "{output}");
    assert!(csv.is_file());
}

#[test]
fn imported_rail_internal_resistance_is_used_by_cli_dc_simulation() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("fonte-ir.txt");
    let project = dir.path().join("fonte-ir.protocraft");
    let source = r#"<cir>
      <R x="0 0 0 32" f="0" wf="0" maxv="10" ir="1"/>
      <r x="0 0 64 0" f="0" r="9"/>
      <g x="64 0 64 16" f="0"/>
    </cir>"#;
    fs::write(&input, source).unwrap();
    let imported = cli(&[
        "import-falstad",
        input.to_str().unwrap(),
        "--output",
        project.to_str().unwrap(),
    ]);
    assert!(
        imported.status.success(),
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );
    let simulation = cli(&["simulate", project.to_str().unwrap(), "--engine", "stable"]);
    assert!(
        simulation.status.success(),
        "{}",
        String::from_utf8_lossy(&simulation.stderr)
    );
    let output = String::from_utf8_lossy(&simulation.stdout);
    assert!(output.contains("V(N1) = 9.000000000 V"), "{output}");
    assert!(output.contains("I(V1) = -1.000000000 A"), "{output}");
}
