use eda_core::*;
use std::{path::Path, process::Command as ProcessCommand};

fn path(p: &Path) -> &str {
    p.to_str().unwrap()
}
fn fixture() -> Engine {
    let mut engine = Engine::new(Project::empty("PCB")).unwrap();
    let fp = eda_library::catalog().unwrap().footprints["Resistor_SMD:R_0805_2012Metric"].clone();
    engine
        .execute(Command::Batch(vec![
            Command::PlaceComponent {
                kind: ComponentKind::Resistor,
                position: Point::new(0, 0),
            },
            Command::AssignFootprint {
                id: ComponentId(1),
                footprint: Box::new(fp),
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
        ]))
        .unwrap();
    let pad = eda_pcb::placed_pads(engine.project())
        .unwrap()
        .into_iter()
        .find(|p| p.number == "1")
        .unwrap();
    let track = Track {
        id: 1,
        start: pad.position,
        end: NmPoint::new(pad.position.x - 3_000_000, pad.position.y),
        width_nm: 200_000,
        layer: BoardSide::Top,
        net: pad.pin.unwrap(),
    };
    let mut reverse = track.clone();
    reverse.id = 2;
    std::mem::swap(&mut reverse.start, &mut reverse.end);
    let mut other_layer = track.clone();
    other_layer.id = 3;
    other_layer.layer = BoardSide::Bottom;
    engine
        .execute(Command::Batch(vec![
            Command::AddTrack { track },
            Command::AddTrack { track: reverse },
            Command::AddTrack { track: other_layer },
        ]))
        .unwrap();
    engine
}

#[test]
fn cleanup_is_atomic_undoable_and_cli_preserves_input() {
    let mut engine = fixture();
    let original_project = engine.project().clone();
    let commands = eda_pcb::cleanup::plan(engine.project()).unwrap();
    assert_eq!(commands.len(), 1);
    engine.execute(Command::Batch(commands)).unwrap();
    let board = engine.project().board.as_ref().unwrap();
    assert!(board.tracks.contains_key(&1));
    assert!(!board.tracks.contains_key(&2));
    assert!(board.tracks.contains_key(&3));
    assert!(engine.undo());
    assert_eq!(engine.project(), &original_project);
    assert!(engine.redo());
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.protocraft");
    let output = dir.path().join("output.protocraft");
    eda_storage::save(&input, &original_project).unwrap();
    let original_bytes = std::fs::read(&input).unwrap();
    let call = |dest: &Path| {
        ProcessCommand::new(env!("CARGO_BIN_EXE_protocraft-cli"))
            .args(["pcb-cleanup", path(&input), "--output", path(dest)])
            .output()
            .unwrap()
    };
    let result = call(&output);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["removed_tracks"], 1);
    assert_eq!(eda_storage::load(&output).unwrap(), *engine.project());
    assert_eq!(std::fs::read(&input).unwrap(), original_bytes);
    assert!(!call(&input).status.success());
    assert!(!call(&output).status.success());
}
