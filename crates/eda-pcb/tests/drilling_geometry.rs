use eda_core::*;
use eda_pcb::check;

#[test]
fn holes_touching_at_zero_clearance_are_detected_but_separated_are_not() {
    let mut engine = Engine::new(Project::empty("holes")).unwrap();
    let board = Board {
        clearance_nm: 0,
        ..Board::default()
    };
    engine.execute(Command::SetBoard { board }).unwrap();
    for (id, x) in [(1, 10_000_000), (2, 11_000_000)] {
        engine
            .execute(Command::AddHole {
                hole: MountingHole {
                    id,
                    position: NmPoint::new(x, 10_000_000),
                    drill_nm: 1_000_000,
                },
            })
            .unwrap();
    }
    assert!(
        check(engine.project())
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.code == "HOLE_CLEARANCE")
    );
    engine.undo();
    engine
        .execute(Command::AddHole {
            hole: MountingHole {
                id: 2,
                position: NmPoint::new(11_000_001, 10_000_000),
                drill_nm: 1_000_000,
            },
        })
        .unwrap();
    assert!(
        !check(engine.project())
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.code == "HOLE_CLEARANCE")
    );
}

#[test]
fn mechanical_hole_touching_track_is_detected_at_zero_clearance() {
    let mut e = Engine::new(Project::empty("hole and copper")).unwrap();
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
            board: Board {
                clearance_nm: 0,
                ..Board::default()
            },
        },
        Command::PlaceOnBoard {
            id: ComponentId(1),
            placement: Placement {
                position: NmPoint::new(20_000_000, 20_000_000),
                rotation: 0,
                side: BoardSide::Top,
            },
        },
        Command::AddTrack {
            track: Track {
                id: 1,
                start: NmPoint::new(10_000_000, 10_000_000),
                end: NmPoint::new(15_000_000, 10_000_000),
                width_nm: 200_000,
                layer: BoardSide::Top,
                net: PinRef {
                    component: ComponentId(1),
                    pin: 0,
                },
            },
        },
        Command::AddHole {
            hole: MountingHole {
                id: 1,
                position: NmPoint::new(12_000_000, 10_600_000),
                drill_nm: 1_000_000,
            },
        },
    ]))
    .unwrap();
    assert!(
        check(e.project())
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.code == "HOLE_COPPER")
    );
}
