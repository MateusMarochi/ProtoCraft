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
fn autorouter_generates_drc_checked_escapes_for_dense_power_pads() {
    let mut project = eda_library::buck_pi4_project().expect("projeto buck válido");
    let board = project.board.as_mut().expect("PCB do buck");
    board.tracks.clear();
    board.vias.clear();
    let pads = eda_pcb::placed_pads(&project).expect("pads posicionados");
    let controller_pads: Vec<_> = pads
        .iter()
        .filter(|pad| pad.component == ComponentId(2))
        .map(|pad| pad.position)
        .collect();

    let plan = autoroute(
        &project,
        &AutorouteConfig {
            // O orçamento baixo força uma execução parcial e exercita os escapes
            // de potência sem transformar este teste em benchmark de roteamento.
            max_total_expansions: 5,
            max_expansions_per_connection: 1,
            optimize_routes: false,
            ..AutorouteConfig::default()
        },
    )
    .expect("roteador produz um plano parcial com fanouts");
    let escape_tracks: Vec<_> = plan
        .commands
        .iter()
        .filter_map(|command| match command {
            Command::AddTrack { track }
                if track.net.component == ComponentId(2)
                    && track.width_nm == 250_000
                    && controller_pads.contains(&track.start)
                    && track
                        .start
                        .x
                        .abs_diff(track.end.x)
                        .max(track.start.y.abs_diff(track.end.y))
                        <= (2 * NM_PER_MM) as u64 =>
            {
                Some(track)
            }
            _ => None,
        })
        .collect();
    assert!(!escape_tracks.is_empty(), "deve gerar saídas dos pads QFN");
    let escape_ends: Vec<_> = escape_tracks.iter().map(|track| track.end).collect();
    assert!(plan.commands.iter().any(|command| matches!(
        command,
        Command::AddVia { via }
            if via.net.component == ComponentId(2)
                && escape_ends.contains(&via.position)
                && via.diameter_nm == 400_000
                && via.drill_nm == 200_000
    )));

    let fanout_commands: Vec<_> = plan
        .commands
        .iter()
        .filter(|command| match command {
            Command::AddTrack { track } => escape_tracks.iter().any(|escape| escape.id == track.id),
            Command::AddVia { via } => escape_ends.contains(&via.position),
            _ => false,
        })
        .cloned()
        .collect();
    let mut engine = Engine::new(project).expect("projeto válido");
    engine
        .execute(Command::Batch(fanout_commands))
        .expect("fanouts aplicados atomicamente");
    let report = check(engine.project()).expect("DRC após fanout");
    assert!(
        report
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == "error")
            .all(|diagnostic| diagnostic.code == "UNROUTED"),
        "fanout não pode criar erro geométrico ou violação de classe: {:?}",
        report
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == "error")
            .map(|diagnostic| (&diagnostic.code, &diagnostic.message))
            .collect::<Vec<_>>()
    );
    assert!(
        report.diagnostics.iter().all(|diagnostic| {
            diagnostic.severity != "error"
                || !matches!(
                    diagnostic.code.as_str(),
                    "COPPER_SHORT" | "CLEARANCE" | "BOARD_EDGE"
                )
        }),
        "fanout não pode introduzir curto, violação de afastamento ou borda"
    );
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
