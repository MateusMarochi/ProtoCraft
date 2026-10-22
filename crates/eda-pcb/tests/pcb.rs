use eda_core::*;
use eda_library::catalog;
use eda_pcb::*;

fn fixture() -> Engine {
    let mut e = Engine::new(Project::empty("PCB de teste")).unwrap();
    let fp = catalog().unwrap().footprints["Resistor_SMD:R_0805_2012Metric"].clone();
    e.execute(Command::Batch(vec![
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
            footprint: Box::new(fp.clone()),
        },
        Command::AssignFootprint {
            id: ComponentId(2),
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
    e
}
#[test]
fn routing_resolves_airwire_and_tracks_are_undoable() {
    let mut e = fixture();
    let initial = check(e.project()).unwrap();
    assert_eq!(initial.airwires.len(), 1);
    let pads = placed_pads(e.project()).unwrap();
    let a = pads
        .iter()
        .find(|p| p.component == ComponentId(1) && p.number == "1")
        .unwrap();
    let b = pads
        .iter()
        .find(|p| p.component == ComponentId(2) && p.number == "1")
        .unwrap();
    let points = [
        a.position,
        NmPoint::new(a.position.x, 18_000_000),
        NmPoint::new(b.position.x, 18_000_000),
        b.position,
    ];
    let anchor = a.pin.unwrap();
    let commands = points
        .windows(2)
        .enumerate()
        .map(|(i, p)| Command::AddTrack {
            track: Track {
                id: i as u64 + 1,
                start: p[0],
                end: p[1],
                width_nm: 250000,
                layer: BoardSide::Top,
                net: anchor,
            },
        })
        .collect();
    e.execute(Command::Batch(commands)).unwrap();
    let routed = check(e.project()).unwrap();
    assert_eq!(routed.errors(), 0, "{:?}", routed.diagnostics);
    assert!(routed.airwires.is_empty());
    assert!(e.undo());
    assert_eq!(check(e.project()).unwrap().airwires.len(), 1);
    assert!(e.redo());
    let text = export_kicad(e.project()).unwrap();
    assert!(text.contains("(segment "));
    assert!(text.contains("Edge.Cuts"));
    assert!(export_svg(e.project()).unwrap().contains("width=\"100mm\""));
}
#[test]
fn short_clearance_edge_and_incompatible_footprint_are_detected() {
    let mut e = fixture();
    let before = e.project().clone();
    assert!(
        e.execute(Command::AssignFootprint {
            id: ComponentId(1),
            footprint: Box::new(
                catalog().unwrap().footprints["Package_QFP:LQFP-48_7x7mm_P0.5mm"].clone()
            )
        })
        .is_err()
    );
    assert_eq!(e.project(), &before);
    e.execute(Command::AddTrack {
        track: Track {
            id: 1,
            start: NmPoint::new(19_000_000, 20_000_000),
            end: NmPoint::new(31_000_000, 20_000_000),
            width_nm: 250000,
            layer: BoardSide::Top,
            net: PinRef {
                component: ComponentId(1),
                pin: 0,
            },
        },
    })
    .unwrap();
    assert!(
        check(e.project())
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.code == "COPPER_SHORT")
    );
    e.execute(Command::PlaceOnBoard {
        id: ComponentId(1),
        placement: Placement {
            position: NmPoint::new(0, 0),
            rotation: 0,
            side: BoardSide::Top,
        },
    })
    .unwrap();
    assert!(
        check(e.project())
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.code == "BOARD_EDGE")
    );
}
#[test]
fn bottom_face_and_via_connect_layers_and_transform_pad_numbers() {
    let mut e = fixture();
    let p = Placement {
        position: NmPoint::new(30_000_000, 20_000_000),
        rotation: 1,
        side: BoardSide::Bottom,
    };
    e.execute(Command::PlaceOnBoard {
        id: ComponentId(2),
        placement: p,
    })
    .unwrap();
    let pads = placed_pads(e.project()).unwrap();
    let a = pads
        .iter()
        .find(|p| p.component == ComponentId(1) && p.number == "1")
        .unwrap();
    let b = pads
        .iter()
        .find(|p| p.component == ComponentId(2) && p.number == "1")
        .unwrap();
    assert_eq!(b.layers, 2);
    assert_eq!(b.position, p.transform(b.pad.position));
    let anchor = a.pin.unwrap();
    let via = NmPoint::new(a.position.x, 15_000_000);
    let points = [
        a.position,
        via,
        NmPoint::new(30_000_000, 15_000_000),
        b.position,
    ];
    for (i, p) in points.windows(2).enumerate() {
        e.execute(Command::AddTrack {
            track: Track {
                id: i as u64 + 1,
                start: p[0],
                end: p[1],
                width_nm: 250000,
                layer: if i == 0 {
                    BoardSide::Top
                } else {
                    BoardSide::Bottom
                },
                net: anchor,
            },
        })
        .unwrap();
    }
    assert!(!check(e.project()).unwrap().airwires.is_empty());
    e.execute(Command::AddVia {
        via: Via {
            id: 1,
            position: via,
            diameter_nm: 600000,
            drill_nm: 300000,
            net: anchor,
        },
    })
    .unwrap();
    assert!(check(e.project()).unwrap().airwires.is_empty());
    e.execute(Command::Delete {
        object: ObjectId::Component(ComponentId(1)),
    })
    .unwrap();
    e.project().validate().unwrap();
    assert!(e.project().board.as_ref().unwrap().tracks.is_empty());
}
#[test]
fn antenna_keepout_survives_placement_rotation_and_blocks_tracks() {
    let mut e = Engine::new(Project::empty("Antena")).unwrap();
    e.execute(
        catalog()
            .unwrap()
            .place_command("ESP32-S3-WROOM-1", Point::new(0, 0))
            .unwrap(),
    )
    .unwrap();
    let place = Placement {
        position: NmPoint::new(50_000_000, 40_000_000),
        rotation: 1,
        side: BoardSide::Top,
    };
    e.execute(Command::Batch(vec![
        Command::SetBoard {
            board: Board::default(),
        },
        Command::PlaceOnBoard {
            id: ComponentId(1),
            placement: place,
        },
    ]))
    .unwrap();
    let fp = e.project().components[&ComponentId(1)]
        .footprint
        .as_ref()
        .unwrap();
    let points = &fp.keepouts[0].polygon;
    let center = NmPoint::new(
        points.iter().map(|p| p.x).sum::<i64>() / points.len() as i64,
        points.iter().map(|p| p.y).sum::<i64>() / points.len() as i64,
    );
    let start = place.transform(center);
    let end = place.transform(NmPoint::new(center.x + 100000, center.y));
    e.execute(Command::AddTrack {
        track: Track {
            id: 1,
            start,
            end,
            width_nm: 100000,
            layer: BoardSide::Bottom,
            net: PinRef {
                component: ComponentId(1),
                pin: 0,
            },
        },
    })
    .unwrap();
    assert!(
        check(e.project())
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.code == "KEEPOUT")
    );
}

#[test]
fn dense_overlap_has_a_bounded_geometry_budget() {
    let mut e = fixture();
    let commands = (1..=1200)
        .map(|id| Command::AddTrack {
            track: Track {
                id,
                start: NmPoint::new(10_000_000, 10_000_000),
                end: NmPoint::new(11_000_000, 10_000_000),
                width_nm: 250000,
                layer: BoardSide::Top,
                net: PinRef {
                    component: ComponentId(1),
                    pin: 0,
                },
            },
        })
        .collect();
    e.execute(Command::Batch(commands)).unwrap();
    assert!(check(e.project()).unwrap_err().contains("orçamento"));
}

#[test]
fn nc_pads_are_preserved_without_creating_phantom_nets_or_routes() {
    let mut e = Engine::new(Project::empty("NC")).unwrap();
    e.execute(
        catalog()
            .unwrap()
            .place_command("ESP32-WROOM-32E", Point::new(0, 0))
            .unwrap(),
    )
    .unwrap();
    e.execute(Command::Batch(vec![
        Command::SetBoard {
            board: Board::default(),
        },
        Command::PlaceOnBoard {
            id: ComponentId(1),
            placement: Placement {
                position: NmPoint::new(50_000_000, 40_000_000),
                rotation: 0,
                side: BoardSide::Top,
            },
        },
    ]))
    .unwrap();
    let pads = placed_pads(e.project()).unwrap();
    let nc = pads.iter().find(|p| p.number == "17").unwrap();
    assert!(nc.net.is_none());
    let anchor = nc.pin.unwrap();
    assert!(
        e.execute(Command::AddTrack {
            track: Track {
                id: 1,
                start: nc.position,
                end: NmPoint::new(nc.position.x + 100000, nc.position.y),
                width_nm: 100000,
                layer: BoardSide::Top,
                net: anchor
            }
        })
        .is_err()
    );
    let pin = e.project().components[&ComponentId(1)]
        .pins()
        .into_iter()
        .find(|p| p.reference == anchor)
        .unwrap();
    e.execute(Command::AddLabel {
        position: pin.position,
        name: "LIGACAO_INVALIDA".into(),
    })
    .unwrap();
    let netlist = eda_netlist::build_netlist(e.project());
    assert!(
        eda_netlist::erc(e.project(), &netlist)
            .iter()
            .any(|d| d.code == "NC_CONNECTED")
    );
    assert!(
        check(e.project())
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.code == "NC_CONNECTED")
    );
    let mut partial = e.project().clone();
    partial.import_info = Some(ImportedProjectInfo {
        source_format: "teste".into(),
        electrical_complete: false,
        notes: vec![],
    });
    assert!(
        check(&partial)
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.code == "INCOMPLETE_IMPORT")
    );
}
