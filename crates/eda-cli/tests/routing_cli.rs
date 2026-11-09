use eda_core::*;
use std::{path::Path, process::Command as ProcessCommand};

fn path(p: &Path) -> &str {
    p.to_str().unwrap()
}
fn cli(args: &[&str]) -> std::process::Output {
    ProcessCommand::new(env!("CARGO_BIN_EXE_protocraft-cli"))
        .args(args)
        .output()
        .unwrap()
}

fn cli_in_language(args: &[&str], language: &str) -> std::process::Output {
    ProcessCommand::new(env!("CARGO_BIN_EXE_protocraft-cli"))
        .env("PROTOCRAFT_LANGUAGE", language)
        .args(args)
        .output()
        .unwrap()
}

fn fixture() -> Project {
    let mut engine = Engine::new(Project::empty("Roteamento")).unwrap();
    let footprint =
        eda_library::catalog().unwrap().footprints["Resistor_SMD:R_0805_2012Metric"].clone();
    engine
        .execute(Command::Batch(vec![
            Command::PlaceComponent {
                kind: ComponentKind::Resistor,
                position: Point::new(-20, 0),
            },
            Command::PlaceComponent {
                kind: ComponentKind::Resistor,
                position: Point::new(20, 0),
            },
            Command::AssignFootprint {
                id: ComponentId(1),
                footprint: Box::new(footprint.clone()),
            },
            Command::AssignFootprint {
                id: ComponentId(2),
                footprint: Box::new(footprint),
            },
            Command::SetBoard {
                board: Board::default(),
            },
            Command::PlaceOnBoard {
                id: ComponentId(1),
                placement: Placement {
                    position: NmPoint::new(20_000_000, 20_000_000),
                    rotation: 0,
                    side: BoardSide::Top,
                },
            },
            Command::PlaceOnBoard {
                id: ComponentId(2),
                placement: Placement {
                    position: NmPoint::new(30_000_000, 20_000_000),
                    rotation: 0,
                    side: BoardSide::Top,
                },
            },
            Command::AddLabel {
                position: Point::new(-23, 0),
                name: "SIGNAL".into(),
            },
            Command::AddLabel {
                position: Point::new(17, 0),
                name: "SIGNAL".into(),
            },
        ]))
        .unwrap();
    engine.project().clone()
}

#[test]
fn routes_an_unconnected_net_and_reports_budget_exhaustion() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.protocraft");
    let output = dir.path().join("routed.protocraft");
    let project = fixture();
    assert_eq!(eda_pcb::check(&project).unwrap().airwires.len(), 1);
    eda_storage::save(&input, &project).unwrap();
    let original = std::fs::read(&input).unwrap();
    let result = cli(&[
        "pcb-route",
        path(&input),
        "--output",
        path(&output),
        "--max-expansions",
        "600000",
        "--max-expansions-per-connection",
        "900000",
        "--prioritize-short",
        "--no-optimize",
        "--require-complete",
        "--ripup-all",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["status"], "complete");
    assert!(report["stats"]["track_count"].as_u64().unwrap() > 0);
    assert_eq!(report["stats"]["remaining_airwires"], 0);
    let routed = eda_storage::load(&output).unwrap();
    assert!(!routed.board.as_ref().unwrap().tracks.is_empty());
    let netlist = eda_netlist::build_netlist(&routed);
    let expected_net = netlist.net_for_pin(PinRef {
        component: ComponentId(1),
        pin: 0,
    });
    assert!(expected_net.is_some());
    assert!(
        routed
            .board
            .as_ref()
            .unwrap()
            .tracks
            .values()
            .all(|track| netlist.net_for_pin(track.net) == expected_net)
    );
    assert_eq!(eda_pcb::check(&routed).unwrap().errors(), 0);
    assert!(eda_pcb::check(&routed).unwrap().airwires.is_empty());
    assert!(cli(&["pcb-check", path(&output)]).status.success());
    assert_eq!(std::fs::read(&input).unwrap(), original);

    for (language, expected_error) in [
        ("pt-BR", "O autorroteamento ficou incompleto"),
        ("en", "Autorouting was incomplete"),
    ] {
        let output = dir.path().join(format!("low-{language}.protocraft"));
        let limited = cli_in_language(
            &[
                "pcb-route",
                path(&input),
                "--output",
                path(&output),
                "--max-expansions",
                "1",
                "--require-complete",
            ],
            language,
        );
        assert!(!limited.status.success());
        let incomplete: serde_json::Value = serde_json::from_slice(&limited.stdout).unwrap();
        assert_eq!(incomplete["status"], "incomplete");
        assert!(incomplete["stats"]["remaining_airwires"].as_u64().unwrap() > 0);
        assert!(incomplete["output"].is_null());
        assert!(!output.exists());
        assert!(String::from_utf8_lossy(&limited.stderr).contains(expected_error));
    }
    assert_eq!(std::fs::read(&input).unwrap(), original);
}

fn reference_example(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name)
}

#[test]
fn bundled_reference_boards_pass_cli_drc() {
    for name in [
        "555-led-1hz-pcb.protocraft",
        "raspberry-pi-4-shield.protocraft",
        "pcb-pi4-12v-buck.protocraft",
    ] {
        let source = reference_example(name);
        let result = cli(&["pcb-check", path(&source)]);
        assert!(
            result.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(report["airwires"].as_array().unwrap().len(), 0, "{name}");
        assert_eq!(
            report["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|diagnostic| diagnostic["severity"] == "error")
                .count(),
            0,
            "{name}"
        );
    }
}

#[test]
#[ignore = "roteamento integral do buck a partir de cobre limpo pode levar vários minutos"]
fn autoroutes_reference_boards_without_new_physical_violations() {
    let dir = tempfile::tempdir().unwrap();
    for name in [
        "555-led-1hz-pcb.protocraft",
        "raspberry-pi-4-shield.protocraft",
        "pcb-pi4-12v-buck.protocraft",
    ] {
        let source = reference_example(name);
        let output = dir.path().join(format!("routed-{name}"));
        let source_bytes = std::fs::read(&source).unwrap();
        let mut project = eda_storage::load(&source).unwrap();
        let board = project.board.as_mut().expect("exemplo precisa ter PCB");
        let source_tracks = board.tracks.len();
        let source_vias = board.vias.len();
        board.tracks.clear();
        board.vias.clear();
        let initial_airwires = eda_pcb::check(&project).unwrap().airwires.len();
        assert!(initial_airwires > 0, "{name}");

        let routed = cli(&[
            "pcb-route",
            path(&source),
            "--output",
            path(&output),
            "--max-expansions",
            "5000000",
            "--max-expansions-per-connection",
            "900000",
            "--prioritize-short",
            "--no-optimize",
            "--ripup-all",
        ]);
        assert!(
            routed.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&routed.stderr)
        );
        let route_report: serde_json::Value = serde_json::from_slice(&routed.stdout).unwrap();
        assert_eq!(route_report["ripped_up_tracks"], source_tracks, "{name}");
        assert_eq!(route_report["ripped_up_vias"], source_vias, "{name}");
        assert!(
            route_report["stats"]["routed_connections"]
                .as_u64()
                .unwrap()
                > 0,
            "{name}"
        );
        assert_eq!(
            route_report["stats"]["initial_airwires"], initial_airwires,
            "{name}"
        );

        let check = cli(&["pcb-check", path(&output)]);
        let check_report: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
        let remaining = check_report["airwires"].as_array().unwrap().len();
        if remaining > 0 {
            assert!(
                name == "pcb-pi4-12v-buck.protocraft",
                "{name}: roteamento parcial não esperado"
            );
        }
        assert_eq!(check.status.success(), remaining == 0, "{name}");
        assert_eq!(
            route_report["stats"]["remaining_airwires"], remaining,
            "{name}"
        );
        assert_eq!(
            route_report["status"],
            if remaining == 0 {
                "complete"
            } else {
                "partial"
            },
            "{name}"
        );
        assert!(
            check_report["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .all(|diagnostic| diagnostic["code"] == "UNROUTED"),
            "{name}: novos erros geométricos após autorroteamento"
        );
        assert_eq!(std::fs::read(&source).unwrap(), source_bytes, "{name}");
    }
}

#[test]
fn route_help_documents_strict_completion() {
    for (language, expected_message) in [
        ("pt-BR", "arquivo de entrada permanece intacto"),
        ("en", "input file stays unchanged"),
    ] {
        let result = cli_in_language(&["pcb-route", "--help"], language);
        assert!(result.status.success());
        let help = String::from_utf8(result.stdout).unwrap();
        assert!(help.contains("--require-complete"));
        assert!(help.contains("--ripup-all"));
        assert!(help.contains("--max-expansions-per-connection"));
        assert!(help.contains("--prioritize-short"));
        assert!(help.contains(expected_message));
    }
}
