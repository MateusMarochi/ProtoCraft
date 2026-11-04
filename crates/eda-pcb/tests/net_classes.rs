use eda_core::*;
use eda_pcb::{AutorouteConfig, autoroute, check};
use std::collections::BTreeSet;

fn routed_fixture() -> Project {
    let catalog = eda_library::catalog().expect("catálogo original disponível");
    let footprint = catalog.footprints["Resistor_SMD:R_0805_2012Metric"].clone();
    let mut engine = Engine::new(Project::empty("fixture-netclass")).expect("projeto vazio válido");
    engine
        .execute(Command::Batch(vec![
            Command::PlaceComponent {
                kind: ComponentKind::Resistor,
                position: Point::new(0, 0),
            },
            Command::PlaceComponent {
                kind: ComponentKind::Resistor,
                position: Point::new(50, 0),
            },
            Command::AssignFootprint {
                id: ComponentId(1),
                footprint: Box::new(footprint.clone()),
            },
            Command::AssignFootprint {
                id: ComponentId(2),
                footprint: Box::new(footprint),
            },
        ]))
        .expect("componentes e footprints válidos");
    let pin_a = engine.project().components[&ComponentId(1)].pins()[0];
    let pin_b = engine.project().components[&ComponentId(2)].pins()[0];
    let mut board = Board {
        width_nm: 70 * NM_PER_MM,
        height_nm: 40 * NM_PER_MM,
        ..Board::default()
    };
    board.net_classes.insert(
        "SINAL".into(),
        NetClass {
            net_names: BTreeSet::from(["ROTA".into()]),
            clearance_nm: 250_000,
            track_width_nm: 600_000,
            via_diameter_nm: 800_000,
            via_drill_nm: 400_000,
            pad_escape: None,
        },
    );
    engine
        .execute(Command::Batch(vec![
            Command::AddLabel {
                position: pin_a.position,
                name: "ROTA".into(),
            },
            Command::AddLabel {
                position: pin_b.position,
                name: "ROTA".into(),
            },
            Command::SetBoard { board },
            Command::PlaceOnBoard {
                id: ComponentId(1),
                placement: Placement {
                    position: NmPoint::new(10 * NM_PER_MM, 20 * NM_PER_MM),
                    rotation: 0,
                    side: BoardSide::Top,
                },
            },
            Command::PlaceOnBoard {
                id: ComponentId(2),
                placement: Placement {
                    position: NmPoint::new(60 * NM_PER_MM, 20 * NM_PER_MM),
                    rotation: 0,
                    side: BoardSide::Top,
                },
            },
        ]))
        .expect("rede e posicionamento válidos");
    engine.project().clone()
}

#[test]
fn autorouter_uses_the_netclass_minimum_track_width() {
    let project = routed_fixture();
    let report = check(&project).expect("DRC da fixture");
    assert!(report.airwires.iter().any(|airwire| airwire.net.0 != 0));
    let plan = autoroute(
        &project,
        &AutorouteConfig {
            optimize_routes: false,
            ..AutorouteConfig::default()
        },
    )
    .expect("plano de roteamento");
    assert!(!plan.commands.is_empty());
    assert!(plan.commands.iter().all(|command| match command {
        Command::AddTrack { track } => track.width_nm == 600_000,
        Command::AddVia { via } => {
            via.diameter_nm == 800_000 && via.drill_nm == 400_000
        }
        _ => false,
    }));
}

#[test]
fn drc_reports_tracks_below_the_assigned_netclass_width() {
    let mut project =
        eda_library::load_pcb_example("pcb-555-led-1hz").expect("fixture interna 555");
    let board = project.board.as_mut().expect("PCB da fixture");
    board
        .net_classes
        .get_mut("ALIMENTACAO")
        .expect("classe de alimentação")
        .track_width_nm = 500_000;
    let report = check(&project).expect("DRC da fixture");
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NETCLASS_TRACK_WIDTH")
    );
}

#[test]
fn drc_reports_netclass_names_that_no_longer_exist() {
    let mut project = routed_fixture();
    project
        .board
        .as_mut()
        .expect("PCB da fixture")
        .net_classes
        .get_mut("SINAL")
        .expect("classe de sinal")
        .net_names
        .insert("REDE_REMOVIDA".into());
    let report = check(&project).expect("DRC da fixture");
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NETCLASS_UNUSED_NET")
    );
}

#[test]
fn pad_escape_allows_only_a_short_track_and_its_connected_via() {
    let mut project = routed_fixture();
    let pad = eda_pcb::placed_pads(&project)
        .unwrap()
        .into_iter()
        .find(|pad| pad.component == ComponentId(1) && pad.number == "1")
        .unwrap();
    let end = NmPoint::new(pad.position.x - 650_000, pad.position.y);
    let net = PinRef {
        component: ComponentId(1),
        pin: 0,
    };
    let board = project.board.as_mut().unwrap();
    board.net_classes.get_mut("SINAL").unwrap().pad_escape = Some(PadEscapeRule {
        track_width_nm: 250_000,
        max_length_nm: 1_000_000,
        via_diameter_nm: 400_000,
        via_drill_nm: 200_000,
    });
    board.tracks.insert(
        1,
        Track {
            id: 1,
            start: pad.position,
            end,
            width_nm: 250_000,
            layer: BoardSide::Top,
            net,
        },
    );
    board.vias.insert(
        1,
        Via {
            id: 1,
            position: end,
            diameter_nm: 400_000,
            drill_nm: 200_000,
            net,
        },
    );
    let report = check(&project).unwrap();
    assert!(!report.diagnostics.iter().any(|item| matches!(
        item.code.as_str(),
        "NETCLASS_TRACK_WIDTH" | "NETCLASS_VIA_SIZE"
    )));

    // Um segmento pequeno solto próximo ao pad não ganha a exceção.
    let mut detached = project.clone();
    detached
        .board
        .as_mut()
        .unwrap()
        .tracks
        .get_mut(&1)
        .unwrap()
        .start
        .x -= 50_000;
    let report = check(&detached).unwrap();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|item| item.code == "NETCLASS_TRACK_WIDTH")
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|item| item.code == "NETCLASS_VIA_SIZE")
    );

    let board = project.board.as_mut().unwrap();
    let long_end = NmPoint::new(pad.position.x - 1_500_000, pad.position.y);
    board.tracks.get_mut(&1).unwrap().end = long_end;
    board.vias.get_mut(&1).unwrap().position = long_end;
    let report = check(&project).unwrap();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|item| item.code == "NETCLASS_TRACK_WIDTH")
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|item| item.code == "NETCLASS_VIA_SIZE")
    );
}
