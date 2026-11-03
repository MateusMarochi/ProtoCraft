use eda_core::*;
use eda_pcb::check;

fn fixture() -> Engine {
    let mut e = Engine::new(Project::empty("layers")).unwrap();
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
fn track(id: u64, start: NmPoint, end: NmPoint, layer: BoardSide, pin: u16) -> Command {
    Command::AddTrack {
        track: Track {
            id,
            start,
            end,
            width_nm: 200_000,
            layer,
            net: PinRef {
                component: ComponentId(1),
                pin,
            },
        },
    }
}
fn short(e: &Engine) -> bool {
    check(e.project())
        .unwrap()
        .diagnostics
        .iter()
        .any(|d| d.code == "COPPER_SHORT")
}

#[test]
fn crossing_layers_separate_until_via_contacts_other_net() {
    let mut e = fixture();
    e.execute(Command::Batch(vec![
        track(
            1,
            NmPoint::new(10_000_000, 12_000_000),
            NmPoint::new(14_000_000, 12_000_000),
            BoardSide::Top,
            0,
        ),
        track(
            2,
            NmPoint::new(12_000_000, 10_000_000),
            NmPoint::new(12_000_000, 14_000_000),
            BoardSide::Bottom,
            1,
        ),
    ]))
    .unwrap();
    assert!(!short(&e));
    e.execute(Command::AddVia {
        via: Via {
            id: 1,
            position: NmPoint::new(12_000_000, 12_000_000),
            diameter_nm: 600_000,
            drill_nm: 300_000,
            net: PinRef {
                component: ComponentId(1),
                pin: 0,
            },
        },
    })
    .unwrap();
    assert!(short(&e));
}

#[test]
fn clearance_boundary_and_tolerance() {
    let mut e = fixture();
    e.execute(track(
        1,
        NmPoint::new(10_000_000, 10_000_000),
        NmPoint::new(15_000_000, 10_000_000),
        BoardSide::Top,
        0,
    ))
    .unwrap();
    e.execute(track(
        2,
        NmPoint::new(10_000_000, 10_400_000),
        NmPoint::new(15_000_000, 10_400_000),
        BoardSide::Top,
        1,
    ))
    .unwrap();
    let has_clearance = |e: &Engine| {
        check(e.project())
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.code == "CLEARANCE")
    };
    assert!(!has_clearance(&e));
    assert!(e.undo());
    e.execute(track(
        2,
        NmPoint::new(10_000_000, 10_399_999),
        NmPoint::new(15_000_000, 10_399_999),
        BoardSide::Top,
        1,
    ))
    .unwrap();
    assert!(!has_clearance(&e)); // margem numérica de 1 nm, conforme critério atual
    assert!(e.undo());
    e.execute(track(
        2,
        NmPoint::new(10_000_000, 10_399_998),
        NmPoint::new(15_000_000, 10_399_998),
        BoardSide::Top,
        1,
    ))
    .unwrap();
    assert!(has_clearance(&e));
}
