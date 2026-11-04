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
fn raspberry_pi_4_gpio_is_in_the_shared_physical_catalog() {
    let c = catalog().unwrap();
    let matches = c.search("Raspberry Pi 4");
    assert!(
        matches
            .iter()
            .any(|record| record.id == "RaspberryPi4B_GPIO")
    );
    assert!(
        c.manufacturers()
            .iter()
            .any(|name| name == "Raspberry Pi Ltd")
    );

    let (device, footprint) = c.definition("Raspberry Pi 4 Model B").unwrap();
    assert_eq!(device.pins.len(), 40);
    assert_eq!(device.pins[0].name, "3V3");
    assert_eq!(device.pins[1].name, "5V");
    assert_eq!(device.pins[10].name, "GPIO17");
    assert_eq!(device.pins[26].name, "ID_SD");
    assert_eq!(footprint.pads.len(), 40);
    assert_eq!(
        footprint
            .pads
            .iter()
            .find(|pad| pad.number == "1")
            .unwrap()
            .position,
        eda_core::NmPoint::new(0, 0)
    );
    assert!(
        c.place_command("RaspberryPi4B_GPIO", Point::new(0, 0))
            .is_ok()
    );
}

#[test]
fn raspberry_pi_catalog_covers_classic_headers_legacy_revisions_and_pico_families() {
    let c = catalog().unwrap();
    for id in [
        "RaspberryPi1BPlus_GPIO40",
        "RaspberryPi1APlus_GPIO40",
        "RaspberryPi2B_GPIO40",
        "RaspberryPi3B_GPIO40",
        "RaspberryPi3BPlus_GPIO40",
        "RaspberryPi3APlus_GPIO40",
        "RaspberryPiZero_GPIO40",
        "RaspberryPiZeroW_GPIO40",
        "RaspberryPiZeroWH_GPIO40",
        "RaspberryPiZero2W_GPIO40",
        "RaspberryPiZero2WWithHeaders_GPIO40",
        "RaspberryPi400_GPIO40",
        "RaspberryPi5_GPIO40",
        "RaspberryPi500_GPIO40",
        "RaspberryPi500Plus_GPIO40",
        "RaspberryPi1BRev1_GPIO26",
        "RaspberryPi1BRev2_GPIO26",
        "RaspberryPi1A_GPIO26",
        "RaspberryPiPico_GPIO40",
        "RaspberryPiPicoH_GPIO40",
        "RaspberryPiPicoW_GPIO40",
        "RaspberryPiPicoWH_GPIO40",
        "RaspberryPiPico2_GPIO40",
        "RaspberryPiPico2WithHeaders_GPIO40",
        "RaspberryPiPico2W_GPIO40",
        "RaspberryPiPico2WWithHeaders_GPIO40",
    ] {
        let (device, footprint) = c
            .definition(id)
            .unwrap_or_else(|error| panic!("{id}: {error}"));
        let expected = if id.ends_with("GPIO26") { 26 } else { 40 };
        assert_eq!(device.pins.len(), expected, "{id}");
        assert_eq!(footprint.pads.len(), expected, "{id}");
    }
    assert_eq!(
        c.definition("Raspberry Pi 5").unwrap().0.pins[2].name,
        "GPIO2"
    );

    let rev1 = c.definition("RaspberryPi1BRev1_GPIO26").unwrap().0;
    let rev2 = c.definition("RaspberryPi1BRev2_GPIO26").unwrap().0;
    assert_eq!(rev1.pins[2].name, "GPIO0/SDA0");
    assert_eq!(rev2.pins[2].name, "GPIO2/SDA1");
    assert_eq!(rev1.pins[4].name, "GPIO1/SCL0");
    assert_eq!(rev2.pins[4].name, "GPIO3/SCL1");
    assert_eq!(rev1.pins[12].name, "GPIO21");
    assert_eq!(rev2.pins[12].name, "GPIO27");
}

#[test]
fn raspberry_pi_keyboard_header_is_inverted_and_pico_has_its_own_two_row_layout() {
    let c = catalog().unwrap();
    let (_, standard) = c.definition("RaspberryPi4B_GPIO").unwrap();
    let (_, keyboard) = c.definition("RaspberryPi500Plus_GPIO40").unwrap();
    assert_eq!(standard.pads[0].position, eda_core::NmPoint::new(0, 0));
    assert_eq!(
        keyboard
            .pads
            .iter()
            .find(|pad| pad.number == "1")
            .unwrap()
            .position,
        eda_core::NmPoint::new(48_260_000, 2_540_000)
    );
    assert_eq!(
        keyboard
            .pads
            .iter()
            .find(|pad| pad.number == "40")
            .unwrap()
            .position,
        eda_core::NmPoint::new(0, 0)
    );

    let (pico, footprint) = c.definition("Raspberry Pi Pico 2 W").unwrap();
    assert_eq!(pico.pins[0].name, "GP0");
    assert_eq!(pico.pins[2].name, "GND");
    assert_eq!(pico.pins[29].name, "RUN");
    assert_eq!(pico.pins[30].name, "GP26/ADC0");
    assert_eq!(pico.pins[35].name, "3V3(OUT)");
    assert_eq!(
        footprint
            .pads
            .iter()
            .find(|pad| pad.number == "1")
            .unwrap()
            .position,
        eda_core::NmPoint::new(0, 0)
    );
    assert_eq!(
        footprint
            .pads
            .iter()
            .find(|pad| pad.number == "20")
            .unwrap()
            .position,
        eda_core::NmPoint::new(0, 48_260_000)
    );
    assert_eq!(
        footprint
            .pads
            .iter()
            .find(|pad| pad.number == "21")
            .unwrap()
            .position,
        eda_core::NmPoint::new(17_780_000, 48_260_000)
    );
    assert_eq!(
        footprint
            .pads
            .iter()
            .find(|pad| pad.number == "40")
            .unwrap()
            .position,
        eda_core::NmPoint::new(17_780_000, 0)
    );
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
