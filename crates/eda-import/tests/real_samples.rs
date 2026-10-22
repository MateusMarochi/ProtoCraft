//! Regressão opt-in: os arquivos de terceiros permanecem fora do repositório.
use eda_core::{ComponentKind, Point};
use eda_import::{container::read_container, dsn::parse_dsn, import_proteus};
use std::{collections::BTreeSet, path::PathBuf};

#[test]
#[ignore = "requer PROTOCRAFT_PROTEUS_SAMPLES apontando para as seis amostras externas"]
fn six_real_projects_preserve_geometry_and_physical_pin_numbers() {
    let root = PathBuf::from(
        std::env::var_os("PROTOCRAFT_PROTEUS_SAMPLES").expect("pasta externa das amostras"),
    );
    let expected = [
        ("01_Circuito_RC", 4, 8, 10, 3, 1),
        ("02_Portas_Logicas", 14, 42, 61, 21, 1),
        ("03_Multiplexador", 12, 36, 51, 18, 1),
        ("04_FlipFlop_RS", 8, 24, 36, 12, 0),
        ("05_Inversor_CMOS", 6, 14, 22, 6, 1),
        ("06_Somador_Completo", 42, 126, 192, 79, 33),
    ];
    for (name, components, pins, wires, junctions, labels) in expected {
        let path = root.join(name).join("projeto_original.pdsprj");
        let container = read_container(&path).unwrap();
        let dsn = parse_dsn(container.dsn.as_deref().unwrap()).unwrap();
        assert_eq!(dsn.components.len(), components, "{name}");
        assert_eq!(
            dsn.components.iter().map(|c| c.pins.len()).sum::<usize>(),
            pins,
            "{name}"
        );
        assert_eq!(dsn.wires.len(), wires, "{name}");
        assert_eq!(dsn.junctions.len(), junctions, "{name}");
        assert_eq!(dsn.labels.len(), labels, "{name}");
        let imported = import_proteus(&path).unwrap();
        imported.project.validate().unwrap();
        assert!(!imported.report.electrical_complete);
        let grounds = dsn.labels.iter().filter(|label| label.is_ground).count();
        assert_eq!(imported.report.components, components + grounds, "{name}");
        assert_eq!(imported.report.junctions, junctions, "{name}");
        assert_eq!(imported.report.labels, labels, "{name}");
        let vertices: BTreeSet<Point> = imported
            .project
            .wires
            .values()
            .flat_map(|wire| [wire.start, wire.end])
            .collect();
        for component in imported
            .project
            .components
            .values()
            .filter(|component| component.kind != ComponentKind::Ground)
        {
            for pin in component.pins() {
                assert!(
                    vertices.contains(&pin.position),
                    "{name}: {} pino {} desconectado da geometria de origem",
                    component.reference,
                    pin.reference.pin
                );
            }
        }
        let encoded = serde_json::to_vec(&imported.project).unwrap();
        let decoded: eda_core::Project = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, imported.project);
        if name == "05_Inversor_CMOS" {
            let q = imported
                .project
                .components
                .values()
                .find(|c| c.reference == "Q2")
                .unwrap();
            let metadata = q.imported.as_ref().unwrap();
            assert_eq!(
                metadata.pins.iter().find(|p| p.name == "D").unwrap().number,
                "2"
            );
            assert_eq!(
                metadata.pins.iter().find(|p| p.name == "G").unwrap().number,
                "1"
            );
        }
    }
}
