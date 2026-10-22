use eda_simulation::{Analysis, SimulationRequest};

#[test]
fn circuit_save_load_simulate_and_export_csv() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("divisor.protocraft");
    eda_storage::save(&path, &eda_core::examples::voltage_divider()).unwrap();
    let project = eda_storage::load(&path).unwrap();
    let result = eda_simulation::simulate(&SimulationRequest {
        project,
        revision: 7,
        analysis: Analysis::Dc,
    })
    .unwrap();
    assert_eq!(result.revision, 7);
    let output = result
        .traces
        .iter()
        .find(|trace| trace.name == "V(VOUT)")
        .unwrap();
    assert!((output.samples[0][1] - 5.0).abs() < 1e-8);
    assert!(
        result
            .traces
            .iter()
            .all(|trace| trace.samples.iter().all(|sample| sample[1].is_finite()))
    );
    let csv = dir.path().join("resultado.csv");
    eda_storage::export_csv(&csv, &result).unwrap();
    let text = std::fs::read_to_string(csv).unwrap();
    assert!(text.starts_with("serie,unidade,tempo_s,valor\n"));
    assert!(text.lines().count() > 1);
}
