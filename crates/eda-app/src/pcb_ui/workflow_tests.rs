use super::*;

fn fixture() -> ProtoCraft {
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
    ProtoCraft::new(e, None, None, None, false, None, false)
}
fn target() -> Placement {
    Placement {
        position: NmPoint::new(20_000_000, 21_000_000),
        rotation: 0,
        side: BoardSide::Top,
    }
}

#[test]
fn move_preserves_connected_track_and_undo_restores_project() {
    let mut app = fixture();
    let pad = eda_pcb::placed_pads(app.engine.project())
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
    app.engine
        .execute(Command::AddTrack {
            track: track.clone(),
        })
        .unwrap();
    let before = app.engine.project().clone();
    assert!(app.pcb.preserve_connections);
    app.move_board_component(ComponentId(1), target());
    let moved = &app.engine.project().board.as_ref().unwrap().tracks[&1];
    assert_eq!(
        moved.start,
        NmPoint::new(track.start.x, track.start.y + 1_000_000)
    );
    assert_eq!((moved.end, moved.net), (track.end, track.net));
    assert!(app.engine.undo());
    assert_eq!(*app.engine.project(), before);
}

#[test]
fn refuses_pad_with_via_and_reports_error_without_mutation() {
    let mut app = fixture();
    let pad = eda_pcb::placed_pads(app.engine.project())
        .unwrap()
        .into_iter()
        .find(|p| p.number == "1")
        .unwrap();
    app.engine
        .execute(Command::AddVia {
            via: Via {
                id: 1,
                position: pad.position,
                diameter_nm: 600_000,
                drill_nm: 300_000,
                net: pad.pin.unwrap(),
            },
        })
        .unwrap();
    let before = app.engine.project().clone();
    app.move_board_component(ComponentId(1), target());
    assert_eq!(*app.engine.project(), before);
    assert!(app.status_error);
}
