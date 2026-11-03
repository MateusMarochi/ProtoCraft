use eda_core::*;
use eda_library::catalog;
use eda_pcb::{check, placed_pads, placement::plan_placement};

fn fixture() -> Engine {
    let mut engine = Engine::new(Project::empty("PCB")).unwrap();
    let footprint = catalog().unwrap().footprints["Resistor_SMD:R_0805_2012Metric"].clone();
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
    engine
}

#[test]
fn unchanged_position_accepts_shared_junction() {
    let mut engine = fixture();
    let pad = placed_pads(engine.project())
        .unwrap()
        .into_iter()
        .find(|p| p.number == "1")
        .unwrap();
    let pin = pad.pin.unwrap();
    engine
        .execute(Command::AddVia {
            via: Via {
                id: 42,
                position: pad.position,
                diameter_nm: 600_000,
                drill_nm: 300_000,
                net: pin,
            },
        })
        .unwrap();
    let position = engine.project().board.as_ref().unwrap().placements[&ComponentId(1)];
    assert!(
        plan_placement(engine.project(), ComponentId(1), position)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn bottom_side_rotation_and_mirrored_pad_translate_together() {
    let mut engine = fixture();
    let old = Placement {
        position: NmPoint::new(20_000_000, 20_000_000),
        rotation: 1,
        side: BoardSide::Bottom,
    };
    engine
        .execute(Command::PlaceOnBoard {
            id: ComponentId(1),
            placement: old,
        })
        .unwrap();
    let pad = placed_pads(engine.project())
        .unwrap()
        .into_iter()
        .find(|p| p.number == "1")
        .unwrap();
    let track = Track {
        id: 4,
        start: pad.position,
        end: NmPoint::new(pad.position.x, pad.position.y + 3_000_000),
        width_nm: 200_000,
        layer: BoardSide::Bottom,
        net: pad.pin.unwrap(),
    };
    engine
        .execute(Command::AddTrack {
            track: track.clone(),
        })
        .unwrap();
    let target = Placement {
        position: NmPoint::new(21_000_000, 20_000_000),
        ..old
    };
    let commands = plan_placement(engine.project(), ComponentId(1), target).unwrap();
    engine.execute(Command::Batch(commands)).unwrap();
    let moved = &engine.project().board.as_ref().unwrap().tracks[&4];
    assert_eq!(
        moved.start,
        NmPoint::new(track.start.x + 1_000_000, track.start.y)
    );
    assert_eq!(moved.end, track.end);
    assert_eq!(moved.layer, BoardSide::Bottom);
    assert_eq!(
        engine.project().board.as_ref().unwrap().placements[&ComponentId(1)],
        target
    );
    let new_pad = placed_pads(engine.project())
        .unwrap()
        .into_iter()
        .find(|p| p.number == "1")
        .unwrap();
    assert_eq!(new_pad.position, moved.start);
}

#[test]
fn move_preserves_track_identity_and_undo() {
    let mut engine = fixture();
    let pad = placed_pads(engine.project())
        .unwrap()
        .into_iter()
        .find(|p| p.component == ComponentId(1) && p.number == "1")
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
    let original = engine.project().clone();
    let placement = Placement {
        position: NmPoint::new(20_000_000, 21_000_000),
        rotation: 0,
        side: BoardSide::Top,
    };
    let commands = plan_placement(engine.project(), ComponentId(1), placement).unwrap();
    engine.execute(Command::Batch(commands)).unwrap();
    let moved = &engine.project().board.as_ref().unwrap().tracks[&1];
    assert_eq!(
        moved.start,
        NmPoint::new(track.start.x, track.start.y + 1_000_000)
    );
    assert_eq!(moved.end, track.end);
    assert_eq!(moved.net, track.net);
    assert_eq!(check(engine.project()).unwrap().errors(), 0);
    assert!(engine.undo());
    assert_eq!(engine.project(), &original);
    assert!(engine.redo());
    assert_eq!(
        engine.project().board.as_ref().unwrap().tracks[&1].start.y,
        track.start.y + 1_000_000
    );
}

#[test]
fn refuses_shared_junction_without_modifying_project() {
    let mut engine = fixture();
    let pad = placed_pads(engine.project())
        .unwrap()
        .into_iter()
        .find(|p| p.number == "1")
        .unwrap();
    let point = pad.position;
    let pin = pad.pin.unwrap();
    for (id, end) in [
        (1, NmPoint::new(point.x - 3_000_000, point.y)),
        (2, NmPoint::new(point.x, point.y - 3_000_000)),
    ] {
        engine
            .execute(Command::AddTrack {
                track: Track {
                    id,
                    start: point,
                    end,
                    width_nm: 200_000,
                    layer: BoardSide::Top,
                    net: pin,
                },
            })
            .unwrap();
    }
    let before = engine.project().clone();
    let target = Placement {
        position: NmPoint::new(21_000_000, 20_000_000),
        rotation: 0,
        side: BoardSide::Top,
    };
    assert!(plan_placement(engine.project(), ComponentId(1), target).is_err());
    assert_eq!(engine.project(), &before);
}

#[test]
fn moves_endpoint_inside_pad_away_from_its_center() {
    let mut engine = fixture();
    let pad = placed_pads(engine.project())
        .unwrap()
        .into_iter()
        .find(|p| p.number == "1")
        .unwrap();
    let point = NmPoint::new(pad.position.x - 200_000, pad.position.y);
    let track = Track {
        id: 7,
        start: point,
        end: NmPoint::new(point.x - 3_000_000, point.y),
        width_nm: 200_000,
        layer: BoardSide::Top,
        net: pad.pin.unwrap(),
    };
    engine.execute(Command::AddTrack { track }).unwrap();
    let target = Placement {
        position: NmPoint::new(20_000_000, 21_000_000),
        rotation: 0,
        side: BoardSide::Top,
    };
    let commands = plan_placement(engine.project(), ComponentId(1), target).unwrap();
    engine.execute(Command::Batch(commands)).unwrap();
    assert_eq!(
        engine.project().board.as_ref().unwrap().tracks[&7].start,
        NmPoint::new(point.x, point.y + 1_000_000)
    );
}
