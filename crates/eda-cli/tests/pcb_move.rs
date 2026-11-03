use eda_core::*;
use std::{path::Path, process::Command as ProcessCommand};

fn cli(args: &[&str]) -> std::process::Output {
    ProcessCommand::new(env!("CARGO_BIN_EXE_protocraft-cli"))
        .args(args)
        .output()
        .unwrap()
}
fn text(path: &Path) -> &str {
    path.to_str().unwrap()
}

#[test]
fn move_tracks_connected_pad_and_rejects_invalid_input() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("board.protocraft");
    let output = dir.path().join("moved.protocraft");
    let mut engine = Engine::new(Project::empty("PCB")).unwrap();
    let footprint =
        eda_library::catalog().unwrap().footprints["Resistor_SMD:R_0805_2012Metric"].clone();
    engine
        .execute(Command::Batch(vec![
            Command::PlaceComponent {
                kind: ComponentKind::Resistor,
                position: Point::new(0, 0),
            },
            Command::AssignFootprint {
                id: ComponentId(1),
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
        ]))
        .unwrap();
    let pad = eda_pcb::placed_pads(engine.project())
        .unwrap()
        .into_iter()
        .find(|pad| pad.number == "1")
        .unwrap();
    let track = Track {
        id: 1,
        start: pad.position,
        end: NmPoint::new(pad.position.x - 3_000_000, pad.position.y),
        width_nm: 200_000,
        layer: BoardSide::Top,
        net: pad.pin.unwrap(),
    };
    engine
        .execute(Command::AddTrack {
            track: track.clone(),
        })
        .unwrap();
    eda_storage::save(&input, engine.project()).unwrap();
    let original = std::fs::read(&input).unwrap();
    let result = cli(&[
        "pcb-move",
        text(&input),
        "--component",
        "1",
        "--x-mm",
        "20",
        "--y-mm",
        "21",
        "--output",
        text(&output),
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["updated_tracks"], 1);
    let moved = eda_storage::load(&output).unwrap();
    let board = moved.board.as_ref().unwrap();
    assert_eq!(
        board.tracks[&1].start,
        NmPoint::new(track.start.x, track.start.y + NM_PER_MM)
    );
    assert_eq!(board.tracks[&1].end, track.end);
    assert_eq!(board.tracks[&1].net, track.net);
    assert_eq!(std::fs::read(&input).unwrap(), original);
    let output_bytes = std::fs::read(&output).unwrap();
    assert!(
        !cli(&[
            "pcb-move",
            text(&input),
            "--component",
            "1",
            "--x-mm",
            "20",
            "--y-mm",
            "21",
            "--output",
            text(&output)
        ])
        .status
        .success()
    );
    assert_eq!(std::fs::read(&output).unwrap(), output_bytes);
    let missing = dir.path().join("missing.protocraft");
    assert!(
        !cli(&[
            "pcb-move",
            text(&input),
            "--component",
            "999",
            "--x-mm",
            "20",
            "--y-mm",
            "21",
            "--output",
            text(&missing)
        ])
        .status
        .success()
    );
    assert!(!missing.exists());
    for bad in ["NaN", "inf", "2001", "abc"] {
        let invalid = dir.path().join("invalid.protocraft");
        assert!(
            !cli(&[
                "pcb-move",
                text(&input),
                "--component",
                "1",
                "--x-mm",
                bad,
                "--y-mm",
                "21",
                "--output",
                text(&invalid)
            ])
            .status
            .success()
        );
        assert!(!invalid.exists());
    }
    assert!(
        !cli(&[
            "pcb-move",
            text(&input),
            "--component",
            "1",
            "--x-mm",
            "20",
            "--x-mm",
            "21",
            "--output",
            text(&output)
        ])
        .status
        .success()
    );
    assert!(
        !cli(&[
            "pcb-move",
            text(&input),
            "--component",
            "1",
            "--x-mm",
            "20",
            "--y-mm",
            "21",
            "--output",
            text(&input)
        ])
        .status
        .success()
    );
}
