use eda_core::{
    ComponentKind, ImportedComponent, ImportedPin, ImportedProjectInfo, Point, examples,
};
use eda_simulation::{Analysis, SimulationError, SimulationRequest, export_spice, simulate};

#[test]
fn incomplete_import_cannot_produce_a_silent_numeric_result_or_spice_export() {
    let mut project = examples::voltage_divider();
    project.import_info = Some(ImportedProjectInfo {
        source_format: "teste".into(),
        electrical_complete: false,
        notes: vec!["Objetos omitidos".into()],
    });
    assert!(matches!(
        export_spice(&project, Analysis::Dc),
        Err(SimulationError::Unsupported(_))
    ));
    assert!(matches!(
        simulate(&SimulationRequest {
            project,
            revision: 0,
            analysis: Analysis::Dc
        }),
        Err(SimulationError::Unsupported(_))
    ));
}

#[test]
fn generic_imported_device_is_not_replaced_by_a_resistor() {
    let mut project = examples::voltage_divider();
    let component = project
        .components
        .values_mut()
        .find(|c| c.reference == "R1")
        .unwrap();
    component.kind = ComponentKind::Imported;
    component.value = 0.0;
    component.imported = Some(ImportedComponent {
        source_format: "teste".into(),
        device: "MODELO_DESCONHECIDO".into(),
        value_text: "".into(),
        properties: Default::default(),
        pins: vec![
            ImportedPin {
                number: "1".into(),
                name: "A".into(),
                offset: Point::new(-3, 0),
            },
            ImportedPin {
                number: "2".into(),
                name: "B".into(),
                offset: Point::new(3, 0),
            },
        ],
    });
    project.validate().unwrap();
    assert!(matches!(
        export_spice(&project, Analysis::Dc),
        Err(SimulationError::Unsupported(_))
    ));
}
