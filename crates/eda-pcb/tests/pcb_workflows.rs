use eda_core::*;
use eda_pcb::{placed_pads, placement::plan_placement, sync};

fn fixture() -> Engine {
    let mut e = Engine::new(Project::empty("PCB")).unwrap();
    let fp = eda_library::catalog().unwrap().footprints["Resistor_SMD:R_0805_2012Metric"].clone();
    e.execute(Command::Batch(vec![
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
    e
}
fn target() -> Placement {
    Placement {
        position: NmPoint::new(20_000_000, 21_000_000),
        rotation: 0,
        side: BoardSide::Top,
    }
}

#[test]
fn refuses_track_crossing_pad_and_preserves_project() {
    let mut e = fixture();
    let pad = placed_pads(e.project())
        .unwrap()
        .into_iter()
        .find(|p| p.number == "1")
        .unwrap();
    e.execute(Command::AddTrack {
        track: Track {
            id: 1,
            start: NmPoint::new(pad.position.x, pad.position.y - 2_000_000),
            end: NmPoint::new(pad.position.x, pad.position.y + 2_000_000),
            width_nm: 200_000,
            layer: BoardSide::Top,
            net: pad.pin.unwrap(),
        },
    })
    .unwrap();
    let before = e.project().clone();
    assert!(plan_placement(e.project(), ComponentId(1), target()).is_err());
    assert_eq!(*e.project(), before);
}

#[test]
fn refuses_via_touching_pad_and_preserves_project() {
    let mut e = fixture();
    let pad = placed_pads(e.project())
        .unwrap()
        .into_iter()
        .find(|p| p.number == "1")
        .unwrap();
    e.execute(Command::AddVia {
        via: Via {
            id: 1,
            position: pad.position,
            diameter_nm: 600_000,
            drill_nm: 300_000,
            net: pad.pin.unwrap(),
        },
    })
    .unwrap();
    let before = e.project().clone();
    assert!(plan_placement(e.project(), ComponentId(1), target()).is_err());
    assert_eq!(*e.project(), before);
}

#[test]
fn insufficient_board_space_does_not_partially_apply_sync() {
    let mut e = Engine::new(Project::empty("small")).unwrap();
    let fp = eda_library::catalog().unwrap().footprints["Resistor_SMD:R_0805_2012Metric"].clone();
    let board = Board {
        width_nm: 10_000_000,
        height_nm: 10_000_000,
        ..Board::default()
    };
    e.execute(Command::Batch(vec![
        Command::PlaceComponent {
            kind: ComponentKind::Resistor,
            position: Point::new(0, 0),
        },
        Command::AssignFootprint {
            id: ComponentId(1),
            footprint: Box::new(fp),
        },
        Command::SetBoard { board },
    ]))
    .unwrap();
    let before = e.project().clone();
    assert!(sync::plan(e.project()).is_err());
    assert_eq!(*e.project(), before);
}
