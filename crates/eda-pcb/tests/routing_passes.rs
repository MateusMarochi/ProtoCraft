use eda_core::*;
use eda_pcb::{AutorouteConfig, autoroute};
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
fn passes_validate_limits_and_respect_global_budget() {
    let project = routed_fixture();
    for invalid in [0, 9] {
        assert!(
            autoroute(
                &project,
                &AutorouteConfig {
                    max_routing_passes: invalid,
                    ..Default::default()
                }
            )
            .is_err()
        );
    }
    let plan = autoroute(
        &project,
        &AutorouteConfig {
            max_routing_passes: 8,
            max_total_expansions: 8,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(plan.stats.search_expansions <= 8);
    assert!((1..=8).contains(&plan.stats.routing_passes_attempted));
    assert!((1..=plan.stats.routing_passes_attempted).contains(&plan.stats.selected_pass));
}
#[test]
fn retry_is_deterministic_and_leaves_input_copper_untouched() {
    let mut project = routed_fixture();
    let board = project.board.as_mut().unwrap();
    let anchor = PinRef {
        component: ComponentId(1),
        pin: 0,
    };
    board.tracks.insert(
        99,
        Track {
            id: 99,
            start: NmPoint::new(5_000_000, 5_000_000),
            end: NmPoint::new(7_000_000, 5_000_000),
            width_nm: 600_000,
            layer: BoardSide::Top,
            net: anchor,
        },
    );
    board.vias.insert(
        99,
        Via {
            id: 99,
            position: NmPoint::new(5_000_000, 5_000_000),
            diameter_nm: 800_000,
            drill_nm: 400_000,
            net: anchor,
        },
    );
    let before = serde_json::to_value(&project).unwrap();
    let config = AutorouteConfig {
        max_routing_passes: 4,
        max_total_expansions: 40,
        ..Default::default()
    };
    let a = autoroute(&project, &config).unwrap();
    let b = autoroute(&project, &config).unwrap();
    assert_eq!(format!("{:?}", a.commands), format!("{:?}", b.commands));
    assert_eq!(before, serde_json::to_value(&project).unwrap());
    let mut engine = Engine::new(project).unwrap();
    engine.execute(Command::Batch(a.commands)).unwrap();
    assert!(
        engine
            .project()
            .board
            .as_ref()
            .unwrap()
            .tracks
            .contains_key(&99)
    );
    assert!(
        engine
            .project()
            .board
            .as_ref()
            .unwrap()
            .vias
            .contains_key(&99)
    );
}

#[test]
fn alternate_order_selects_better_completion_under_limited_work() {
    let project = routed_fixture();
    let fp = project.components[&ComponentId(1)]
        .footprint
        .clone()
        .unwrap();
    let mut engine = Engine::new(project).unwrap();
    engine
        .execute(Command::PlaceComponent {
            kind: ComponentKind::Resistor,
            position: Point::new(100, 0),
        })
        .unwrap();
    let id = *engine.project().components.keys().last().unwrap();
    engine
        .execute(Command::AssignFootprint { id, footprint: fp })
        .unwrap();
    let position = engine.project().components[&id].pins()[0].position;
    engine
        .execute(Command::AddLabel {
            position,
            name: "ROTA".into(),
        })
        .unwrap();
    engine
        .execute(Command::PlaceOnBoard {
            id,
            placement: Placement {
                position: NmPoint::new(15_000_000, 20_000_000),
                rotation: 0,
                side: BoardSide::Top,
            },
        })
        .unwrap();
    let project = engine.project();
    let mut improvement = None;
    for budget in [100, 200, 400, 800, 1600, 3200] {
        let config = AutorouteConfig {
            max_total_expansions: budget,
            optimize_routes: false,
            ..Default::default()
        };
        let long = autoroute(project, &config).unwrap();
        let short = autoroute(
            project,
            &AutorouteConfig {
                prioritize_short_connections: true,
                ..config.clone()
            },
        )
        .unwrap();
        if short.stats.remaining_airwires < long.stats.remaining_airwires {
            let retry = autoroute(
                project,
                &AutorouteConfig {
                    max_routing_passes: 2,
                    max_total_expansions: budget * 2,
                    ..config
                },
            )
            .unwrap();
            assert!(retry.stats.remaining_airwires <= short.stats.remaining_airwires);
            assert_eq!(retry.stats.selected_pass, 2);
            improvement = Some(budget);
            break;
        }
    }
    assert!(
        improvement.is_some(),
        "fixture deve demonstrar benefício da ordem alternativa"
    );
}
