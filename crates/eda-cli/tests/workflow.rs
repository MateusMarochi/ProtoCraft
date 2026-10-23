use std::{
    path::Path,
    process::{Command, Output},
};

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_protocraft-cli"))
        .args(args)
        .output()
        .expect("CLI deve iniciar")
}
fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}
fn path_text(path: &Path) -> &str {
    path.to_str().unwrap()
}

#[test]
fn real_cli_workflow() {
    let dir = tempfile::tempdir().unwrap();
    success(cli(&["examples", path_text(dir.path())]));
    for name in ["divisor.protocraft", "rc.protocraft", "diodo.protocraft"] {
        assert!(dir.path().join(name).is_file());
    }
    let divider = dir.path().join("divisor.protocraft");
    assert!(
        !success(cli(&["validate", path_text(&divider)]))
            .trim()
            .is_empty()
    );
    let netlist = success(cli(&["netlist", path_text(&divider)]));
    assert!(netlist.contains("R1 (ID "));
    let pin_label = eda_i18n::text("{reference} (ID {id}) / pino {pin}");
    let pin_suffix = pin_label.split_once('/').unwrap().1.trim();
    assert!(netlist.contains(&pin_suffix.replace("{pin}", "1")));
    assert!(!netlist.contains(&pin_suffix.replace("{pin}", "0")));
    let original = std::fs::read(&divider).unwrap();
    assert!(
        !cli(&[
            "simulate",
            path_text(&divider),
            "--csv",
            path_text(&divider)
        ])
        .status
        .success()
    );
    assert!(
        !cli(&[
            "spice",
            path_text(&divider),
            "--output",
            path_text(&divider)
        ])
        .status
        .success()
    );
    assert_eq!(std::fs::read(&divider).unwrap(), original);
    let csv = dir.path().join("divisor.csv");
    let dc = success(cli(&[
        "simulate",
        path_text(&divider),
        "--csv",
        path_text(&csv),
    ]));
    assert!(dc.contains("V(VOUT) = 5.000000000 V"));
    assert!(
        std::fs::read_to_string(csv)
            .unwrap()
            .starts_with("serie,unidade,tempo_s,valor\n")
    );
    let rc = dir.path().join("rc.protocraft");
    let transient_csv = dir.path().join("rc.csv");
    success(cli(&[
        "simulate",
        path_text(&rc),
        "--transient",
        "0.0001",
        "0.002",
        "--csv",
        path_text(&transient_csv),
    ]));
    assert!(
        std::fs::read_to_string(transient_csv)
            .unwrap()
            .lines()
            .count()
            > 10
    );
    let spice = dir.path().join("rc.cir");
    success(cli(&[
        "spice",
        path_text(&rc),
        "--transient",
        "0.0001",
        "0.002",
        "--output",
        path_text(&spice),
    ]));
    assert!(std::fs::read_to_string(spice).unwrap().contains(".tran "));
    assert!(success(cli(&["spice", path_text(&divider)])).contains(".op"));
    assert!(!success(cli(&["--help"])).trim().is_empty());
    assert!(success(cli(&["run", "--help"])).contains("protocraft-cli run"));
}

#[test]
fn cli_inspects_projects_lists_commands_and_runs_atomic_headless_edits() {
    let dir = tempfile::tempdir().unwrap();
    success(cli(&["examples", path_text(dir.path())]));
    let input = dir.path().join("divisor.protocraft");
    let output_path = dir.path().join("editado.protocraft");

    let info: serde_json::Value =
        serde_json::from_slice(success(cli(&["info", path_text(&input)])).as_bytes()).unwrap();
    assert_eq!(info["format_version"], eda_core::FORMAT_VERSION);
    assert_eq!(info["components"], 4);
    assert_eq!(info["nets"], 3);
    assert_eq!(info["erc"]["errors"], 0);

    let commands: serde_json::Value =
        serde_json::from_slice(success(cli(&["commands", "--filter", "component"])).as_bytes())
            .unwrap();
    assert!(
        commands
            .as_array()
            .unwrap()
            .iter()
            .any(|command| command["id"] == "place_component")
    );

    let actions = dir.path().join("acoes.json");
    std::fs::write(
        &actions,
        r#"[
          {"command":"set_component","params":{"id":2,"reference":"R1","value":2200}},
          {"command":"rename_project","params":{"name":"Editado por CLI"}}
        ]"#,
    )
    .unwrap();
    let result = success(cli(&[
        "run",
        path_text(&input),
        "--actions",
        path_text(&actions),
        "--output",
        path_text(&output_path),
    ]));
    let result: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(result["ok"], true);
    assert_eq!(result["actions"], 2);
    assert_eq!(result["project"]["name"], "Editado por CLI");
    let project = eda_storage::load(&output_path).unwrap();
    assert_eq!(project.name, "Editado por CLI");
    assert_eq!(project.components[&eda_core::ComponentId(2)].value, 2_200.0);
    assert_eq!(
        eda_storage::load(&input).unwrap().components[&eda_core::ComponentId(2)].value,
        1_000.0
    );

    let command_output = dir.path().join("editado-por-comando.protocraft");
    let command_result = success(cli(&[
        "run",
        path_text(&input),
        "--cmd",
        "rename_project",
        "--params",
        r#"{"name":"Comando direto"}"#,
        "--out",
        path_text(&command_output),
    ]));
    let command_result: serde_json::Value = serde_json::from_str(&command_result).unwrap();
    assert_eq!(command_result["actions"], 1);
    assert_eq!(
        eda_storage::load(&command_output).unwrap().name,
        "Comando direto"
    );

    let invalid_actions = dir.path().join("acoes-invalidas.json");
    std::fs::write(
        &invalid_actions,
        r#"[
          {"command":"rename_project","params":{"name":"Nao salvar"}},
          {"command":"set_component","params":{"id":999,"reference":"R9","value":1000}}
        ]"#,
    )
    .unwrap();
    let rejected = cli(&[
        "run",
        path_text(&input),
        "--actions",
        path_text(&invalid_actions),
        "--output",
        path_text(&dir.path().join("rejeitado.protocraft")),
    ]);
    assert!(!rejected.status.success());
    assert!(!dir.path().join("rejeitado.protocraft").exists());
}

#[test]
fn real_cli_rejects_unknown_commands_bad_files_and_options() {
    let unknown = cli(&["inexistente"]);
    assert!(!unknown.status.success());
    let unknown_error = String::from_utf8_lossy(&unknown.stderr);
    assert!(unknown_error.contains("inexistente"));
    assert!(unknown_error.contains("--help"));
    let dir = tempfile::tempdir().unwrap();
    let empty = dir.path().join("vazio.protocraft");
    eda_storage::save(&empty, &eda_core::Project::empty("Sem componentes")).unwrap();
    let erc = cli(&["validate", path_text(&empty)]);
    assert!(!erc.status.success());
    let diagnostics = String::from_utf8_lossy(&erc.stdout);
    assert!(diagnostics.contains("NO_GROUND"));
    assert!(diagnostics.contains("NO_SOURCE"));
    let bad = dir.path().join("corrompido.protocraft");
    std::fs::write(&bad, "isto não é JSON").unwrap();
    let invalid = cli(&["validate", path_text(&bad)]);
    assert!(!invalid.status.success());
    assert!(!String::from_utf8_lossy(&invalid.stderr).trim().is_empty());
    let invalid_time = cli(&["simulate", path_text(&bad), "--transient", "NaN", "1"]);
    assert!(!invalid_time.status.success());
    assert!(
        String::from_utf8_lossy(&invalid_time.stderr).contains(&eda_i18n::text(
            "Transiente requer 0 < passo <= fim, em segundos"
        ))
    );
    assert!(
        !cli(&["validate", path_text(&bad), "--csv", "saida.csv"])
            .status
            .success()
    );
}

#[test]
fn cli_selects_real_engines_and_edits_persisted_model_parameters() {
    let engines: serde_json::Value = serde_json::from_str(&success(cli(&["engines"]))).unwrap();
    assert_eq!(engines.as_array().unwrap().len(), 3);
    let components: serde_json::Value =
        serde_json::from_str(&success(cli(&["components"]))).unwrap();
    assert!(
        components
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["kind"] == "Npn")
    );
    let dir = tempfile::tempdir().unwrap();
    success(cli(&["examples", path_text(dir.path())]));
    let input = dir.path().join("rc-senoidal.protocraft");
    let output = dir.path().join("configurado.protocraft");
    success(cli(&[
        "run",
        path_text(&input),
        "--cmd",
        "set_model",
        "--params",
        r#"{"id":1,"parameters":{"frequency_hz":250.0,"phase_deg":45.0}}"#,
        "--output",
        path_text(&output),
    ]));
    let project = eda_storage::load(&output).unwrap();
    assert_eq!(
        project.components[&eda_core::ComponentId(1)]
            .parameters
            .frequency_hz,
        250.0
    );
    let dynamic = success(cli(&[
        "simulate",
        path_text(&output),
        "--engine",
        "dynamic",
        "--transient",
        "0.00001",
        "0.001",
    ]));
    let stable = success(cli(&[
        "simulate",
        path_text(&output),
        "--engine",
        "stable",
        "--transient",
        "0.00001",
        "0.001",
    ]));
    assert!(dynamic.contains(&eda_i18n::text("Dinâmico")));
    assert!(stable.contains(&eda_i18n::text("Estável")));
    assert!(
        !cli(&["simulate", path_text(&output), "--engine", "desconhecido"])
            .status
            .success()
    );
}

#[test]
fn cli_runs_mixed_timer_and_rejects_unsupported_engine() {
    let dir = tempfile::tempdir().unwrap();
    success(cli(&["examples", path_text(dir.path())]));
    let components: serde_json::Value =
        serde_json::from_str(&success(cli(&["components"]))).unwrap();
    assert_eq!(components.as_array().unwrap().len(), 36);
    let input = dir.path().join("555-astavel.protocraft");
    let csv = dir.path().join("timer.csv");
    let output = success(cli(&[
        "simulate",
        path_text(&input),
        "--engine",
        "mixed",
        "--transient",
        "0.00001",
        "0.005",
        "--csv",
        path_text(&csv),
    ]));
    assert!(output.contains(&eda_i18n::text("Misto")));
    let contents = std::fs::read_to_string(csv).unwrap();
    assert!(contents.contains("V(VOUT)"));
    let result = cli(&["simulate", path_text(&input), "--engine", "stable"]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains(&eda_i18n::text("Misto")));
}

#[test]
fn physical_catalog_cli_places_real_mcu_and_exports_pad_mapping_and_pcb() {
    let devices: serde_json::Value = serde_json::from_str(&success(cli(&[
        "devices",
        "--filter",
        "STM32F103",
        "--limit",
        "5",
    ])))
    .unwrap();
    assert!(devices["total"].as_u64().unwrap() > 5);
    assert_eq!(devices["devices"].as_array().unwrap().len(), 5);
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("stm32.protocraft");
    success(cli(&[
        "device-project",
        "STM32F103C8T6",
        "--output",
        path_text(&file),
    ]));
    let info: serde_json::Value =
        serde_json::from_str(&success(cli(&["pcb-info", path_text(&file)]))).unwrap();
    let map = info["pin_map"].as_array().unwrap();
    assert_eq!(map.len(), 48);
    assert!(
        map.iter()
            .any(|p| p["pin"] == "34" && p["name"] == "PA13" && p["pads"][0] == "34")
    );
    let pcb = dir.path().join("stm32.kicad_pcb");
    success(cli(&[
        "pcb-export",
        path_text(&file),
        "--format",
        "kicad",
        "--output",
        path_text(&pcb),
    ]));
    assert!(
        std::fs::read_to_string(pcb)
            .unwrap()
            .contains("LQFP-48_7x7mm")
    );
    let edited = dir.path().join("duplo.protocraft");
    success(cli(&[
        "run",
        path_text(&file),
        "--cmd",
        "place_device",
        "--params",
        r#"{"device":"ESP32-S3-WROOM-1","position":{"x":60,"y":0}}"#,
        "--output",
        path_text(&edited),
    ]));
    assert_eq!(eda_storage::load(edited).unwrap().components.len(), 2);
    assert!(!cli(&["simulate", path_text(&file)]).status.success());
    let original = std::fs::read(&file).unwrap();
    assert!(
        !cli(&[
            "pcb-export",
            path_text(&file),
            "--format",
            "svg",
            "--output",
            path_text(&file)
        ])
        .status
        .success()
    );
    assert_eq!(std::fs::read(file).unwrap(), original);
}
