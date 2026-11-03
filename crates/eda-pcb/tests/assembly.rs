use eda_core::*;
use eda_library::catalog;
use eda_pcb::assembly::{assembly_report, bom_csv, placements_csv};

#[test]
fn report_groups_orders_and_preserves_bottom_coordinates() {
    let mut engine = Engine::new(Project::empty("montagem")).unwrap();
    let fp = catalog().unwrap().footprints["Resistor_SMD:R_0805_2012Metric"].clone();
    engine
        .execute(Command::Batch(vec![
            Command::PlaceComponent {
                kind: ComponentKind::Resistor,
                position: Point::new(0, 0),
            },
            Command::PlaceComponent {
                kind: ComponentKind::Resistor,
                position: Point::new(20, 0),
            },
            Command::PlaceComponent {
                kind: ComponentKind::Capacitor,
                position: Point::new(40, 0),
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
                id: ComponentId(2),
                placement: Placement {
                    position: NmPoint::new(12_000_000, 13_000_000),
                    rotation: 3,
                    side: BoardSide::Bottom,
                },
            },
        ]))
        .unwrap();
    let report = assembly_report(engine.project());
    assert_eq!(
        report
            .bom
            .iter()
            .find(|r| r.kind == "Resistor")
            .unwrap()
            .references,
        ["R1", "R2"]
    );
    assert_eq!(report.missing_footprint.len(), 1);
    assert_eq!(report.unplaced.len(), 2);
    assert_eq!(report.placements.len(), 1);
    let row = &report.placements[0];
    assert_eq!(
        (
            row.reference.as_str(),
            row.side,
            row.x_nm,
            row.y_nm,
            row.rotation_deg
        ),
        ("R2", "bottom", 12_000_000, 13_000_000, 270)
    );
    assert!(placements_csv(&report).contains("bottom,12000000,13000000,270"));
    assert_eq!(
        bom_csv(&report),
        bom_csv(&assembly_report(engine.project()))
    );
}

#[test]
fn empty_project_and_csv_quotes() {
    let mut project = Project::empty("vazio");
    let report = assembly_report(&project);
    assert!(report.bom.is_empty());
    assert!(report.placements.is_empty());
    let mut engine = Engine::new(project.clone()).unwrap();
    engine
        .execute(Command::PlaceComponent {
            kind: ComponentKind::Resistor,
            position: Point::new(0, 0),
        })
        .unwrap();
    project = engine.project().clone();
    project
        .components
        .get_mut(&ComponentId(1))
        .unwrap()
        .reference = "R,\"1\"\nnext".into();
    assert!(bom_csv(&assembly_report(&project)).contains("\"R,\"\"1\"\"\nnext\""));
}

#[test]
fn distinct_devices_do_not_merge_and_units_are_explicit() {
    let catalog = catalog().unwrap();
    let mut engine = Engine::new(Project::empty("parts")).unwrap();
    engine
        .execute(
            catalog
                .place_command("STM32F103C8Tx", Point::new(0, 0))
                .unwrap(),
        )
        .unwrap();
    engine
        .execute(
            catalog
                .place_command("STM32F103C8Tx", Point::new(100, 0))
                .unwrap(),
        )
        .unwrap();
    engine
        .execute(Command::PlaceComponent {
            kind: ComponentKind::Resistor,
            position: Point::new(200, 0),
        })
        .unwrap();
    let mut project = engine.project().clone();
    project
        .components
        .get_mut(&ComponentId(2))
        .unwrap()
        .device
        .as_mut()
        .unwrap()
        .id = "another-device".into();
    let report = assembly_report(&project);
    let devices: Vec<_> = report.bom.iter().filter(|r| r.kind == "Device").collect();
    assert_eq!(devices.len(), 2);
    assert!(devices.iter().all(|r| r.quantity == 1));
    assert_eq!(
        report
            .bom
            .iter()
            .find(|r| r.kind == "Resistor")
            .unwrap()
            .unit,
        "Ω"
    );
    assert!(
        bom_csv(&report).starts_with("references,quantity,kind,value,unit,device,footprint\r\n")
    );
}
