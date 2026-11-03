use eda_core::*;
use eda_storage::fabrication::write_assembly_reports;

#[test]
fn reports_snapshot_and_incomplete_components_without_overwriting() {
    let mut engine = Engine::new(Project::empty("placa")).unwrap();
    let footprint =
        eda_library::catalog().unwrap().footprints["Resistor_SMD:R_0805_2012Metric"].clone();
    engine
        .execute(Command::Batch(vec![
            Command::PlaceComponent {
                kind: ComponentKind::Resistor,
                position: Point::new(0, 0),
            },
            Command::PlaceComponent {
                kind: ComponentKind::Capacitor,
                position: Point::new(20, 0),
            },
            Command::AssignFootprint {
                id: ComponentId(1),
                footprint: Box::new(footprint),
            },
            Command::SetBoard {
                board: Board::default(),
            },
            Command::PlaceOnBoard {
                id: ComponentId(1),
                placement: Placement {
                    position: NmPoint::new(12_000_000, 13_000_000),
                    rotation: 1,
                    side: BoardSide::Bottom,
                },
            },
        ]))
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("montagem");
    write_assembly_reports(engine.project(), &dest).unwrap();
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dest.join("assembly.json")).unwrap()).unwrap();
    assert_eq!(json["placements"][0]["component_id"], 1);
    assert_eq!(json["placements"][0]["x_nm"], 12_000_000);
    assert_eq!(json["placements"][0]["rotation_deg"], 90);
    assert_eq!(json["missing_footprint"][0]["component_id"], 2);
    assert_eq!(json["unplaced"][0]["component_id"], 2);
    assert_eq!(json["placements"].as_array().unwrap().len(), 1);
    let bom = std::fs::read_to_string(dest.join("bom.csv")).unwrap();
    assert!(bom.contains("R1"));
    assert!(bom.contains("C1"));
    let positions = std::fs::read_to_string(dest.join("placements.csv")).unwrap();
    assert!(positions.contains("bottom,12000000,13000000,90"));
    assert!(write_assembly_reports(engine.project(), &dest).is_err());
    assert_eq!(std::fs::read_to_string(dest.join("bom.csv")).unwrap(), bom);
}
