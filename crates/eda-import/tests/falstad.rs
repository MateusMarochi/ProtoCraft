use eda_core::{ComponentKind, Point};
use eda_import::falstad::import_text;

#[test]
fn text_and_xml_keep_passive_geometry_and_values() {
    let text = "$ 1 0.000005 10.2 50 5 50\nr 0 0 32 0 0 1000\nw 32 0 64 0 0\ng 64 0 64 16 0\n";
    let xml = "<cir f=\"1\"><r x=\"0 0 32 0\" f=\"0\" r=\"1000\" /><w x=\"32 0 64 0\" f=\"0\"/><g x=\"64 0 64 16\" f=\"0\"/></cir>";
    for source in [text, xml] {
        let import = import_text(source, "teste").unwrap();
        assert_eq!(import.report.components, 2);
        assert_eq!(import.report.wires, 1);
        assert!(import.report.electrical_complete);
        let resistor = import
            .project
            .components
            .values()
            .find(|c| c.kind == ComponentKind::Resistor)
            .unwrap();
        assert_eq!(resistor.value, 1000.0);
        assert_eq!(resistor.pins()[0].position, Point::new(0, 0));
        assert_eq!(resistor.pins()[1].position, Point::new(32, 0));
    }
}

#[test]
fn unknown_device_and_custom_model_block_simulation_without_dropping_pins() {
    let xml = "<cir><mm nm=\"depletion\" vt=\"-1.5\" be=\"20\"/><f x=\"0 0 48 0\" f=\"1\" mo=\"depletion\"/><q x=\"48 0 80 0\" f=\"0\"/></cir>";
    let import = import_text(xml, "modelos").unwrap();
    assert!(!import.report.electrical_complete);
    assert_eq!(import.project.components.len(), 2);
    assert_eq!(
        import
            .project
            .components
            .values()
            .find(|c| c.imported.as_ref().is_some_and(|i| i.device == "f"))
            .unwrap()
            .pins()
            .len(),
        3
    );
    assert_eq!(
        import
            .project
            .components
            .values()
            .find(|c| c.imported.as_ref().is_some_and(|i| i.device == "q"))
            .unwrap()
            .pins()
            .len(),
        2
    );
    assert!(
        !import
            .project
            .import_info
            .as_ref()
            .unwrap()
            .electrical_complete
    );
}

#[test]
fn rejects_xml_entities_and_bad_coordinates() {
    for source in [
        "<cir><r x=\"0 0 32 0\" f=\"0\" r=\"&evil;\"/></cir>",
        "$ 1 0.1\nr 0 0 999999 0 0 1000\n",
    ] {
        assert!(import_text(source, "erro").is_err());
    }
}

#[test]
fn accepts_predefined_xml_entities_but_rejects_dtd() {
    let safe = "<cir><x x=\"0 0 32 0\" f=\"0\" te=\"R&amp;D\"/><r x=\"0 0 32 0\" f=\"0\" r=\"1000\"/></cir>";
    assert_eq!(import_text(safe, "texto").unwrap().report.components, 1);
    let unsafe_xml = "<?xml version=\"1.0\"?><!DOCTYPE cir [<!ENTITY ext SYSTEM 'file:///secret'>]><cir><r x=\"0 0 32 0\" f=\"0\" r=\"1000\"/></cir>";
    assert!(import_text(unsafe_xml, "texto").is_err());
}

#[test]
fn annotations_and_visual_probes_do_not_make_circuits_partial() {
    let source = r#"<cir>
      <x x="0 0 32 0" f="0" te="divisor de teste"/>
      <v x="0 0 0 64" f="0" wf="0" maxv="10"/>
      <r x="0 0 64 0" f="0" r="1000"/>
      <r x="64 0 64 64" f="0" r="1000"/>
      <w x="0 64 64 64" f="0"/>
      <g x="64 64 64 80" f="0"/>
      <p x="64 0 64 64" f="0"/>
      <O x="64 0 64 0" f="0"/>
    </cir>"#;
    let import = import_text(source, "divisor").unwrap();
    assert!(import.report.electrical_complete);
    assert_eq!(import.report.components, 4);
    assert_eq!(import.report.wires, 1);
    assert!(
        import
            .project
            .components
            .values()
            .all(|component| { component.kind != ComponentKind::Imported })
    );
    let source = import
        .project
        .components
        .values()
        .find(|component| component.kind == ComponentKind::VoltageSource)
        .unwrap();
    assert_eq!(source.pins()[0].position, Point::new(0, 0));
    assert_eq!(source.pins()[1].position, Point::new(0, 64));
}

#[test]
fn supported_semiconductors_switches_and_custom_diode_models_map_to_generic_models() {
    let source = r#"<cir>
      <dm nm="diode-test" is="2e-8" rs="0.25" n="1.6" bv="80"/>
      <tm nm="bjt-test" is="3e-13" nf="1.2"/>
      <mm nm="mos-test" vt="-1.5" be="0.04" la="0.02"/>
      <s x="0 0 32 0" f="0" p="0"/>
      <d x="32 0 64 0" f="0" mo="diode-test"/>
      <t x="64 32 96 32" f="0" pn="-1" be="80" mo="bjt-test"/>
      <f x="128 32 160 32" f="1" mo="mos-test"/>
    </cir>"#;
    let import = import_text(source, "modelos").unwrap();
    assert!(import.report.electrical_complete);
    let switch = import
        .project
        .components
        .values()
        .find(|component| component.kind == ComponentKind::Switch)
        .unwrap();
    assert!(switch.parameters.closed);
    let diode = import
        .project
        .components
        .values()
        .find(|component| component.kind == ComponentKind::Diode)
        .unwrap();
    assert!((diode.value - 2e-8).abs() < f64::EPSILON);
    assert!((diode.parameters.diode_series_resistance - 0.25).abs() < f64::EPSILON);
    assert!((diode.parameters.ideality - 1.6).abs() < f64::EPSILON);
    let transistor = import
        .project
        .components
        .values()
        .find(|component| component.kind == ComponentKind::Pnp)
        .unwrap();
    assert_eq!(transistor.value, 80.0);
    assert!((transistor.parameters.saturation_current - 3e-13).abs() < f64::EPSILON);
    assert!((transistor.parameters.ideality - 1.2).abs() < f64::EPSILON);
    let mosfet = import
        .project
        .components
        .values()
        .find(|component| component.kind == ComponentKind::Pmos)
        .unwrap();
    assert!((mosfet.parameters.mos_threshold_v - 1.5).abs() < f64::EPSILON);
    assert!((mosfet.value - 0.04).abs() < f64::EPSILON);
    assert!((mosfet.parameters.channel_modulation - 0.02).abs() < f64::EPSILON);
    assert_eq!(import.report.warnings.len(), 4);
}

#[test]
fn nonzero_capacitor_and_inductor_initial_conditions_remain_blocked() {
    for source in [
        "<cir><c x=\"0 0 32 0\" f=\"0\" c=\"1e-6\" iv=\"2\"/></cir>",
        "<cir><l x=\"0 0 32 0\" f=\"0\" l=\"1e-3\" ic=\"0.2\"/></cir>",
    ] {
        let import = import_text(source, "condicao-inicial").unwrap();
        assert!(!import.report.electrical_complete);
    }
}

#[test]
fn legacy_bjt_records_keep_beta_after_saved_junction_state_fields() {
    let source = "$ 1 0.000005 10.2 50 5 50\nt 0 0 32 0 0 1 0 0 120 default\n";
    let import = import_text(source, "bjt-legado").unwrap();
    let transistor = import
        .project
        .components
        .values()
        .find(|component| component.kind == ComponentKind::Npn)
        .unwrap();
    assert_eq!(transistor.value, 120.0);
    assert!(import.report.electrical_complete);
}

#[test]
fn zero_polarity_transistor_keeps_its_degenerate_connectivity_without_blocking_simulation() {
    let source = "<cir><t x=\"0 0 32 0\" f=\"0\" pn=\"0\" be=\"100\"/></cir>";
    let import = import_text(source, "transistor-degenerado").unwrap();
    assert!(import.report.electrical_complete);
    let leakage = import.project.components.values().next().unwrap();
    assert_eq!(leakage.kind, ComponentKind::Resistor);
    assert_eq!(leakage.value, 1e12);
    assert_eq!(leakage.pins()[0].position, Point::new(0, 0));
    assert_eq!(leakage.pins()[1].position, Point::new(32, 0));
}

#[test]
fn legacy_net_labels_and_opamps_map_to_native_connections_and_model() {
    let source = "$ 1 0.000005 10.2 50 5 50\nln 0 0 32 -16 0 NET_A\nln 64 0 96 -16 0 NET_A\na 0 64 64 64 8 15 -15 1000000 0 0 100000\n";
    let import = import_text(source, "rotulos-e-opamp").unwrap();
    assert!(import.report.electrical_complete);
    assert_eq!(import.report.labels, 2);
    let amplifier = import
        .project
        .components
        .values()
        .find(|component| component.kind == ComponentKind::OpAmp)
        .unwrap();
    assert_eq!(amplifier.pins()[0].position, Point::new(0, 80));
    assert_eq!(amplifier.pins()[1].position, Point::new(0, 48));
    assert_eq!(amplifier.pins()[2].position, Point::new(64, 64));
    assert_eq!(amplifier.parameters.gain, 100000.0);
    assert_eq!(amplifier.parameters.rail_high, 15.0);
    assert_eq!(amplifier.parameters.rail_low, -15.0);
}

#[test]
fn source_internal_resistance_is_preserved_as_a_series_component() {
    let source = r#"<cir>
      <R x="0 0 0 32" f="0" wf="0" maxv="10" ir="1"/>
      <r x="0 0 64 0" f="0" r="9"/>
      <g x="64 0 64 16" f="0"/>
    </cir>"#;
    let import = import_text(source, "fonte-com-resistencia").unwrap();
    assert!(import.report.electrical_complete);
    assert_eq!(
        import
            .project
            .components
            .values()
            .filter(|component| component.kind == ComponentKind::Resistor)
            .count(),
        2
    );
    assert!(import.project.components.values().any(|component| {
        component.kind == ComponentKind::Resistor && (component.value - 1.0).abs() < f64::EPSILON
    }));
}
