use eda_core::*;
#[test]
fn physical_project_snapshots_round_trip_and_v4_migrates() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("fisico.protocraft");
    let p = eda_library::physical_example().unwrap();
    eda_storage::save(&file, &p).unwrap();
    assert_eq!(eda_storage::load(&file).unwrap(), p);
    let mut legacy = serde_json::to_value(eda_core::examples::voltage_divider()).unwrap();
    legacy["format_version"] = 4.into();
    for c in legacy["components"].as_object_mut().unwrap().values_mut() {
        c.as_object_mut().unwrap().remove("device");
        c.as_object_mut().unwrap().remove("footprint");
    }
    let old = dir.path().join("antigo.protocraft");
    std::fs::write(&old, serde_json::to_vec(&legacy).unwrap()).unwrap();
    assert_eq!(
        eda_storage::load(&old).unwrap().format_version,
        eda_core::FORMAT_VERSION
    );
    let mut invalid = p.clone();
    invalid
        .components
        .get_mut(&ComponentId(1))
        .unwrap()
        .device
        .as_mut()
        .unwrap()
        .pins[0]
        .pads = vec!["9999".into()];
    assert!(eda_storage::save(&file, &invalid).is_err());
    assert_eq!(eda_storage::load(&file).unwrap(), p);
}
#[test]
fn fabrication_preserves_existing_destinations_and_blocks_unrouted_boards() {
    let dir = tempfile::tempdir().unwrap();
    let existing = dir.path().join("existente");
    std::fs::create_dir(&existing).unwrap();
    std::fs::write(existing.join("preservar.txt"), b"original").unwrap();
    let p = eda_library::physical_example().unwrap();
    let tool = std::env::current_exe().unwrap();
    assert!(eda_storage::fabrication::generate(&p, &existing, &tool).is_err());
    assert_eq!(
        std::fs::read(existing.join("preservar.txt")).unwrap(),
        b"original"
    );
    let output = dir.path().join("nao-gerar");
    let error = eda_storage::fabrication::generate(&p, &output, &tool).unwrap_err();
    assert!(error.contains("DRC"));
    assert!(!output.exists());
}
