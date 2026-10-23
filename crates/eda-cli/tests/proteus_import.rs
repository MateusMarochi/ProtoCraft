use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_protocraft-cli"))
        .args(args)
        .output()
        .unwrap()
}
fn text(path: &Path) -> &str {
    path.to_str().unwrap()
}

#[test]
fn protects_input_output_report_and_backup_paths_before_import() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("source.protocraft");
    let original = b"fonte autoral para testar protecao de caminhos";
    fs::write(&input, original).unwrap();
    let output = dir.path().join("out.protocraft");
    let cases = [
        (
            vec!["import-proteus", text(&input), "--output", text(&input)],
            "Saída e backup devem ser diferentes do arquivo Proteus de entrada",
        ),
        (
            vec![
                "import-proteus",
                text(&input),
                "--output",
                text(&output),
                "--report",
                text(&input),
            ],
            "O relatório deve ser diferente da entrada, da saída e do backup",
        ),
        (
            vec![
                "import-proteus",
                text(&input),
                "--output",
                text(&output),
                "--report",
                text(&output),
            ],
            "O relatório deve ser diferente da entrada, da saída e do backup",
        ),
    ];
    for (args, expected_source) in cases {
        let result = cli(&args);
        assert!(!result.status.success());
        let expected_error = eda_i18n::text(expected_source);
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            stderr.contains(&expected_error),
            "stderr: {stderr}; expected: {expected_error}"
        );
        assert_eq!(fs::read(&input).unwrap(), original);
        assert!(!output.exists());
    }
    let report = dir.path().join("report.json");
    fs::write(&report, b"nao substituir").unwrap();
    let result = cli(&[
        "import-proteus",
        text(&input),
        "--output",
        text(&output),
        "--report",
        text(&report),
    ]);
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains(&eda_i18n::text(
            "O relatório já existe; escolha um novo caminho para preservá-lo"
        ))
    );
    assert_eq!(fs::read(&report).unwrap(), b"nao substituir");
    let backup = eda_storage::backup_path(&output);
    fs::write(&backup, original).unwrap();
    let result = cli(&["import-proteus", text(&backup), "--output", text(&output)]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("backup"));
    assert_eq!(fs::read(&backup).unwrap(), original);
}

#[test]
fn rejects_missing_and_repeated_options_and_preserves_sources_on_parse_errors() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("invalid.dsn");
    fs::write(&input, b"fixture autoral invalida").unwrap();
    let output = dir.path().join("out.protocraft");
    for args in [
        vec!["import-proteus"],
        vec!["import-proteus", text(&input)],
        vec!["import-proteus", text(&input), "--output"],
        vec![
            "import-proteus",
            text(&input),
            "--output",
            text(&output),
            "--output",
            text(&output),
        ],
        vec![
            "import-proteus",
            text(&input),
            "--output",
            text(&output),
            "--unknown",
            "x",
        ],
        vec!["import-proteus", text(&input), "--output", text(&output)],
    ] {
        assert!(!cli(&args).status.success());
        assert!(!output.exists());
    }
    assert_eq!(fs::read(&input).unwrap(), b"fixture autoral invalida");
}

#[test]
fn imports_original_synthetic_dsn_and_reports_partial_electrical_status() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("synthetic.dsn");
    let output = dir.path().join("synthetic.protocraft");
    let report = dir.path().join("synthetic.json");
    // Fixture autoral de fio, compartilhando apenas o layout público do parser.
    let mut bytes =
        b"ISIS SCHEMATIC FILE\x1aISIS CIRCUIT FILE\x1aOBJECT DATA\0\x02\x7fWIRE\0\0\0".to_vec();
    bytes.extend(2u16.to_le_bytes());
    for coordinate in [-300i32, 400, 500, 400] {
        bytes.extend(coordinate.to_le_bytes());
    }
    fs::write(&input, &bytes).unwrap();
    let result = cli(&[
        "import-proteus",
        text(&input),
        "--output",
        text(&output),
        "--report",
        text(&report),
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let project = eda_storage::load(&output).unwrap();
    assert_eq!(project.wires.len(), 1);
    let summary: serde_json::Value = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
    assert_eq!(summary["wires"], 1);
    assert_eq!(summary["electrical_complete"], false);
    assert!(
        String::from_utf8_lossy(&result.stdout)
            .contains(&eda_i18n::text(
                "Aviso: a importação não preservou todo o significado elétrico; revise o relatório antes de simular."
            ))
    );
    assert_eq!(fs::read(&input).unwrap(), bytes);
}

#[test]
fn imports_real_examples_opt_in_without_modifying_sources() {
    let Some(root) = std::env::var_os("PROTOCRAFT_PROTEUS_SAMPLES") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    for folder in [
        "01_Circuito_RC",
        "02_Portas_Logicas",
        "03_Multiplexador",
        "04_FlipFlop_RS",
        "05_Inversor_CMOS",
        "06_Somador_Completo",
    ] {
        let input = Path::new(&root)
            .join(folder)
            .join("projeto_original.pdsprj");
        let original = fs::read(&input).unwrap();
        let output = dir.path().join(format!("{folder}.protocraft"));
        let report = dir.path().join(format!("{folder}.json"));
        let result = cli(&[
            "import-proteus",
            text(&input),
            "--output",
            text(&output),
            "--report",
            text(&report),
        ]);
        assert!(
            result.status.success(),
            "{folder}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let project = eda_storage::load(&output).unwrap();
        let summary: serde_json::Value =
            serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
        assert_eq!(
            summary["components"].as_u64().unwrap() as usize,
            project.components.len()
        );
        assert_eq!(
            summary["wires"].as_u64().unwrap() as usize,
            project.wires.len()
        );
        assert!(summary["electrical_complete"].is_boolean());
        assert!(!project.components.is_empty());
        assert_eq!(fs::read(&input).unwrap(), original);
        // O DSN solto também deve importar sem substituir o ZIP original.
        let dsn = input.parent().unwrap().join("ROOT.DSN");
        let output_dsn = dir.path().join(format!("{folder}-dsn.protocraft"));
        let result = cli(&["import-proteus", text(&dsn), "--output", text(&output_dsn)]);
        assert!(
            result.status.success(),
            "{folder} DSN: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            eda_storage::load(&output_dsn).unwrap().components.len(),
            project.components.len()
        );
    }
}
