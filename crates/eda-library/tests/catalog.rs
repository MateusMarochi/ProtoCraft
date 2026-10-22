use eda_core::{Command, Engine, Point, Project};
use eda_library::catalog;

#[test]
fn every_catalog_entry_has_valid_exact_pin_to_pad_mapping() {
    let c = catalog().unwrap();
    assert!(c.devices.len() > 2200);
    for record in &c.devices {
        let mut engine = Engine::new(Project::empty("Catálogo")).unwrap();
        engine
            .execute(c.place_command(&record.id, Point::new(0, 0)).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e}", record.id));
        let component = engine.project().components.values().next().unwrap();
        assert_eq!(component.pins().len(), c.symbols[&record.symbol].len());
    }
}
#[test]
fn f103_lqfp48_pins_dimensions_and_suffix_lookup_match_reference() {
    let c = catalog().unwrap();
    let (d, f) = c.definition("STM32F103C8T6").unwrap();
    assert_eq!(d.id, "STM32F103C8Tx");
    assert_eq!(d.pins.len(), 48);
    for (number, name) in [("1", "VBAT"), ("7", "NRST"), ("34", "PA13"), ("37", "PA14")] {
        assert_eq!(
            d.pins.iter().find(|p| p.number == number).unwrap().name,
            name
        );
    }
    assert_eq!(f.body_bounds.width(), 7_000_000);
    assert_eq!(f.body_bounds.height(), 7_000_000);
    assert!(f.id.contains("P0.5mm"));
    assert!(c.find("STM32INEXISTENTE").is_err());
    let mut engine = Engine::new(Project::empty("MCU")).unwrap();
    engine
        .execute(Command::PlaceDevice {
            device: Box::new(d),
            footprint: Box::new(f),
            position: Point::new(0, 0),
        })
        .unwrap();
    assert!(engine.undo());
    assert!(engine.project().components.is_empty());
    assert!(engine.redo());
}
#[test]
fn esp32_module_retains_exposed_pad_antenna_keepout_and_package_size() {
    let (d, f) = catalog().unwrap().definition("ESP32-S3-WROOM-1").unwrap();
    assert_eq!(d.form, "Module");
    assert_eq!(d.pins.len(), 41);
    assert_eq!(
        d.pins.iter().find(|p| p.number == "41").unwrap().name,
        "GND"
    );
    assert_eq!(f.body_bounds.width(), 18_000_000);
    assert_eq!(f.body_bounds.height(), 25_500_000);
    assert!(!f.keepouts.is_empty());
    assert!(f.pads.iter().filter(|p| p.number == "41").count() > 1);
}
#[test]
fn bga_more_than_255_pins_rotates_without_index_truncation() {
    let c = catalog().unwrap();
    let row = c
        .devices
        .iter()
        .max_by_key(|d| c.symbols[&d.symbol].len())
        .unwrap();
    let mut engine = Engine::new(Project::empty("BGA")).unwrap();
    engine
        .execute(c.place_command(&row.id, Point::new(10, 10)).unwrap())
        .unwrap();
    let id = *engine.project().components.keys().next().unwrap();
    let original = engine.project().components[&id].pins();
    assert!(original.len() > 255);
    assert_eq!(
        original.last().unwrap().reference.pin as usize,
        original.len() - 1
    );
    for _ in 0..4 {
        engine.execute(Command::RotateComponent { id }).unwrap();
    }
    assert_eq!(engine.project().components[&id].pins(), original);
}

#[test]
fn antenna_variants_have_distinct_explicit_footprints_and_exact_part_search() {
    let c = catalog().unwrap();
    let (a, fa) = c.definition("ESP32-C6-MINI-1").unwrap();
    let (b, fb) = c.definition("ESP32-C6-MINI-1U").unwrap();
    assert_ne!(fa.id, fb.id);
    assert_ne!(fa.body_bounds.height(), fb.body_bounds.height());
    assert_eq!(a.pins.len(), b.pins.len());
    assert!(c.find("ESP32-C6-MINI-1/U").is_err());
    assert_eq!(c.search("STM32F103C8T6").len(), 1);
    assert_eq!(
        c.definition("STM32F103C8T6").unwrap().0.requested_part,
        "STM32F103C8T6"
    );
}

#[test]
fn grouped_kicad_ground_pins_expand_to_original_physical_numbers() {
    let (d, f) = catalog().unwrap().definition("ESP32-WROOM-32E").unwrap();
    assert_eq!(d.pins.len(), 39);
    for number in ["1", "15", "38", "39"] {
        let p = d.pins.iter().find(|p| p.number == number).unwrap();
        assert_eq!(p.name, "GND");
        assert_eq!(p.pads, vec![number.to_string()]);
    }
    assert!(
        d.pins
            .iter()
            .any(|p| p.number == "17" && p.electrical_type == "no_connect")
    );
    assert_eq!(f.body_bounds.width(), 18_000_000);
    assert_eq!(f.body_bounds.height(), 25_500_000);
}
