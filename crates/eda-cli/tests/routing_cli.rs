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
    let low = dir.path().join("low.protocraft");
    let project = fixture();
    assert_eq!(eda_pcb::check(&project).unwrap().airwires.len(), 1);
    eda_storage::save(&input, &project).unwrap();
    let original = std::fs::read(&input).unwrap();
    let result = cli(&["pcb-route", path(&input), "--output", path(&output)]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
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

    let limited = cli(&[
        "pcb-route",
        path(&input),
        "--output",
        path(&low),
        "--max-expansions",
        "1",
    ]);
    if limited.status.success() {
        let stats: serde_json::Value = serde_json::from_slice(&limited.stdout).unwrap();
        let low_project = eda_storage::load(&low).unwrap();
        let drc = eda_pcb::check(&low_project).unwrap();
        assert!(
            drc.diagnostics.iter().all(|d| d.code == "UNROUTED"),
            "{:?}",
            drc.diagnostics
        );
        assert_eq!(
            stats["stats"]["remaining_airwires"].as_u64().unwrap(),
            drc.airwires.len() as u64
        );
        assert!(!drc.airwires.is_empty());
    } else {
        assert!(!low.exists());
    }
    assert_eq!(std::fs::read(&input).unwrap(), original);
}
