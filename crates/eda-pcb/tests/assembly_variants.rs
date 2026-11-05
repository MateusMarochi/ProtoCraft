use eda_core::*;
use eda_pcb::{assembly::assembly_report, assembly_variants::*};

fn project() -> Project {
    let mut engine = Engine::new(Project::empty("variantes")).unwrap();
    for x in [0, 20] {
        engine
            .execute(Command::PlaceComponent {
                kind: ComponentKind::Resistor,
                position: Point::new(x, 0),
            })
            .unwrap();
    }
    engine.project().clone()
}
#[test]
fn filters_bom_without_modifying_original_or_retained_ids() {
    let mut engine = Engine::new(project()).unwrap();
    let footprint =
        eda_library::catalog().unwrap().footprints["Resistor_SMD:R_0805_2012Metric"].clone();
    engine
        .execute(Command::Batch(vec![
            Command::AssignFootprint {
                id: ComponentId(2),
                footprint: Box::new(footprint),
            },
            Command::SetBoard {
                board: Board::default(),
            },
            Command::PlaceOnBoard {
                id: ComponentId(2),
                placement: Placement {
                    position: NmPoint::new(12_000_000, 13_000_000),
                    rotation: 3,
                    side: BoardSide::Bottom,
                },
            },
        ]))
        .unwrap();
    let project = engine.project().clone();
    let before = serde_json::to_value(&project).unwrap();
    let base = assembly_report(&project);
    let report = variant_assembly_report(
        &project,
        &AssemblyVariant {
            name: "reduced".into(),
            do_not_populate: vec!["R1".into()],
        },
    )
    .unwrap();
    assert_eq!(report.omitted[0].component_id, 1);
    assert_eq!(
        report
            .assembly
            .bom
            .iter()
            .map(|r| r.quantity)
            .sum::<usize>(),
        1
    );
    assert_eq!(report.assembly.bom[0].references, vec!["R2"]);
    assert_eq!(report.assembly.placements[0].side, "bottom");
    assert_eq!(report.assembly.placements[0].rotation_deg, 270);
    assert_eq!(report.assembly.placements[0].x_nm, 12_000_000);
    assert_eq!(report.assembly.placements[0].component_id, 2);
    assert_eq!(serde_json::to_value(&project).unwrap(), before);
    let default = variant_assembly_report(
        &project,
        &AssemblyVariant {
            name: "base".into(),
            do_not_populate: vec![],
        },
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(default.assembly).unwrap(),
        serde_json::to_value(base).unwrap()
    );
}
#[test]
fn rejects_invalid_configuration_and_unknown_fields() {
    for (name, refs) in [("", vec![]), ("x", vec!["R9"]), ("x", vec!["R1", "R1"])] {
        let config = AssemblyVariant {
            name: name.into(),
            do_not_populate: refs.into_iter().map(String::from).collect(),
        };
        assert!(variant_assembly_report(&project(), &config).is_err());
    }
    assert!(serde_json::from_str::<AssemblyVariant>(r#"{"name":"x","typo":1}"#).is_err());
    let mut engine = Engine::new(project()).unwrap();
    engine
        .execute(Command::PlaceComponent {
            kind: ComponentKind::Ground,
            position: Point::new(40, 0),
        })
        .unwrap();
    let reference = engine
        .project()
        .components
        .values()
        .find(|c| c.kind == ComponentKind::Ground)
        .unwrap()
        .reference
        .clone();
    assert!(
        variant_assembly_report(
            engine.project(),
            &AssemblyVariant {
                name: "x".into(),
                do_not_populate: vec![reference]
            }
        )
        .is_err()
    );
}
