use eda_core::{Component, ComponentId, ComponentKind, Point, Project};
use eda_import::kicad_schematic::{export_kicad_schematic, parse_kicad_schematic};
use eda_netlist::build_netlist;
use std::{
    collections::{BTreeMap, BTreeSet},
    process::Command,
};

fn connectivity(project: &Project) -> BTreeSet<BTreeSet<(String, String)>> {
    let netlist = build_netlist(project);
    let mut nets: BTreeMap<_, BTreeSet<_>> = BTreeMap::new();
    for component in project.components.values() {
        for pin in component.pins() {
            if let Some(net) = netlist.net_for_pin(pin.reference) {
                nets.entry(net).or_default().insert((
                    component.reference.clone(),
                    component.pin_number(pin.reference.pin),
                ));
            }
        }
    }
    nets.into_values().collect()
}

fn two_ground_project() -> Project {
    let mut project = Project::empty("Terras separados no desenho, mesma rede elétrica");
    for (id, reference, x) in [(1, "GND1", 10), (2, "GND2", 20)] {
        project.components.insert(
            ComponentId(id),
            Component {
                id: ComponentId(id),
                reference: reference.into(),
                kind: ComponentKind::Ground,
                value: 0.0,
                position: Point::new(x, 0),
                rotation: 0,
                imported: None,
                device: None,
                footprint: None,
                parameters: Default::default(),
            },
        );
    }
    project.next_id = 3;
    project
}

#[test]
fn schematic_export_round_trip_preserves_component_pin_connectivity() {
    let source = two_ground_project();
    let (encoded, report) = export_kicad_schematic(&source).expect("esquema exportado");
    assert!(!report.lossless());
    assert_eq!(
        connectivity(&source),
        connectivity(
            &parse_kicad_schematic("roundtrip", &encoded)
                .unwrap()
                .project
        )
    );
}

#[test]
fn power_symbols_join_the_named_net_and_report_unmappable_metadata() {
    let source = r##"
(kicad_sch
 (version 20250114) (generator eeschema) (uuid "00000000-0000-4000-8000-000000000001")
 (paper "A4")
 (lib_symbols
  (symbol "power:GND" (power)
   (property "Reference" "#PWR" (at 0 0 0) (effects (font (size 1 1))))
   (symbol "power:GND_0_1")
   (symbol "power:GND_1_1"
    (pin power_in line (at 0 0 90) (length 0) (name "GND") (number "1")))))
 (symbol (lib_id "power:GND") (at 10 10 0) (unit 1)
  (property "Reference" "#PWR01" (at 10 10 0) (effects (font (size 1 1))))
  (property "Value" "GND" (at 10 10 0) (effects (font (size 1 1)))))
 (symbol (lib_id "power:GND") (at 20 10 0) (unit 1)
  (property "Reference" "#PWR02" (at 20 10 0) (effects (font (size 1 1))))
  (property "Value" "GND" (at 20 10 0) (effects (font (size 1 1)))))
)
"##;
    let imported = parse_kicad_schematic("power", source).expect("esquema KiCad válido");
    assert!(!imported.report.electrical_complete);
    assert_eq!(connectivity(&imported.project).len(), 1);
    let pins: Vec<_> = imported
        .project
        .components
        .values()
        .flat_map(|component| component.pins())
        .collect();
    assert_eq!(pins.len(), 2);
    let netlist = build_netlist(&imported.project);
    assert_eq!(
        netlist.net_for_pin(pins[0].reference),
        netlist.net_for_pin(pins[1].reference)
    );
}

#[test]
#[ignore = "requer PROTOCRAFT_KICAD_CLI apontando para o executável oficial do KiCad"]
fn kicad_cli_accepts_generated_schematic_and_exports_its_netlist() {
    let executable = std::env::var_os("PROTOCRAFT_KICAD_CLI")
        .expect("caminho do kicad-cli em PROTOCRAFT_KICAD_CLI");
    let directory = tempfile::tempdir().expect("pasta temporária");
    let schematic_path = directory.path().join("roundtrip.kicad_sch");
    let netlist_path = directory.path().join("roundtrip.net");
    let (schematic, _) = export_kicad_schematic(&two_ground_project()).expect("esquema exportado");
    std::fs::write(&schematic_path, schematic).expect("arquivo temporário");
    let result = Command::new(executable)
        .args([
            "sch",
            "export",
            "netlist",
            schematic_path.to_str().expect("caminho UTF-8"),
            "-o",
            netlist_path.to_str().expect("caminho UTF-8"),
        ])
        .output()
        .expect("executar kicad-cli");
    assert!(
        result.status.success(),
        "KiCad rejeitou o esquema: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let netlist = std::fs::read_to_string(netlist_path).expect("netlist KiCad exportada");
    let parsed = eda_import::sexpr::parse(&netlist).expect("netlist S-expression válida");
    let nets = parsed.child("nets").expect("seção de redes");
    let exported: Vec<_> = nets.children("net").collect();
    assert_eq!(
        exported.len(),
        1,
        "os dois símbolos Ground devem virar uma rede"
    );
    let nodes: BTreeSet<_> = exported[0]
        .children("node")
        .filter_map(|node| node.child("ref").and_then(|value| value.value(1)))
        .collect();
    assert_eq!(nodes, BTreeSet::from(["GND1", "GND2"]));
}
