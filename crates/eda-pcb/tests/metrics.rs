use eda_core::*;
use eda_pcb::metrics::routing_metrics;

#[test]
fn named_net_measures_segments_and_via_by_layer_deterministically() {
    let mut engine = Engine::new(Project::empty("metrics")).unwrap();
    engine
        .execute(Command::Batch(vec![
            Command::PlaceComponent {
                kind: ComponentKind::Resistor,
                position: Point::new(0, 0),
            },
            Command::AddLabel {
                position: Point::new(-3, 0),
                name: "SIGNAL".into(),
            },
            Command::AssignFootprint {
                id: ComponentId(1),
                footprint: Box::new(
                    eda_library::catalog().unwrap().footprints["Resistor_SMD:R_0805_2012Metric"]
                        .clone(),
                ),
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
            Command::AddTrack {
                track: Track {
                    id: 1,
                    start: NmPoint::new(10_000_000, 10_000_000),
                    end: NmPoint::new(13_000_000, 14_000_000),
                    width_nm: 250_000,
                    layer: BoardSide::Top,
                    net: PinRef {
                        component: ComponentId(1),
                        pin: 0,
                    },
                },
            },
            Command::AddVia {
                via: Via {
                    id: 1,
                    position: NmPoint::new(13_000_000, 14_000_000),
                    diameter_nm: 600_000,
                    drill_nm: 300_000,
                    net: PinRef {
                        component: ComponentId(1),
                        pin: 0,
                    },
                },
            },
        ]))
        .unwrap();
    let report = routing_metrics(engine.project()).unwrap();
    let net = report.nets.iter().find(|n| n.name == "SIGNAL").unwrap();
    assert_eq!((net.top_tracks, net.bottom_tracks, net.vias), (1, 0, 1));
    assert!((net.length_mm - 5.0).abs() < 1e-12);
    assert!((net.top_length_mm - 5.0).abs() < 1e-12);
    assert_eq!(
        serde_json::to_string(&report).unwrap(),
        serde_json::to_string(&routing_metrics(engine.project()).unwrap()).unwrap()
    );
}

#[test]
fn absent_board_returns_stable_error() {
    assert_eq!(
        routing_metrics(&Project::empty("empty")).unwrap_err(),
        "NO_BOARD"
    );
}
