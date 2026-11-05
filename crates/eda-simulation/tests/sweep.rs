use eda_core::examples::voltage_divider;
use eda_simulation::{
    Analysis, SimulationControl, SimulationEngine, SimulationRequest,
    sweep::{SweepConfig, SweepScale, dc_sweep},
};
#[test]
fn sweep_preserves_original_and_obeys_linear_dc_scaling() {
    let project = voltage_divider();
    let source = project
        .components
        .values()
        .find(|c| c.kind == eda_core::ComponentKind::VoltageSource)
        .unwrap()
        .id;
    let original = project.clone();
    let request = SimulationRequest {
        project,
        revision: 17,
        analysis: Analysis::Dc,
    };
    let config = SweepConfig {
        component: source,
        start: 2.0,
        stop: 6.0,
        points: 3,
        scale: SweepScale::Linear,
    };
    let result = dc_sweep(
        &request,
        &config,
        SimulationEngine::Stable,
        &SimulationControl::default(),
    )
    .unwrap();
    assert_eq!(result.revision, 17);
    assert_eq!(
        result.points.iter().map(|p| p.value).collect::<Vec<_>>(),
        vec![2.0, 4.0, 6.0]
    );
    let first = &result.points[0].result.operating_points;
    assert!(!first.is_empty());
    let last = &result.points[2].result.operating_points;
    for (a, b) in first.iter().zip(last) {
        assert!((b.voltage - a.voltage * 3.0).abs() < 1e-8);
        assert!((b.current - a.current * 3.0).abs() < 1e-8);
    }
    assert_eq!(request.project, original);
    let cancelled = SimulationControl::default();
    cancelled.cancel();
    assert!(dc_sweep(&request, &config, SimulationEngine::Stable, &cancelled).is_err());
    let timeout = SimulationControl::new(std::time::Duration::ZERO);
    assert!(dc_sweep(&request, &config, SimulationEngine::Stable, &timeout).is_err());
    let invalid = SweepConfig {
        start: f64::NAN,
        ..config
    };
    assert!(
        dc_sweep(
            &request,
            &invalid,
            SimulationEngine::Stable,
            &SimulationControl::default()
        )
        .is_err()
    );
}

#[test]
fn logarithmic_endpoints_and_invalid_domains_are_explicit() {
    let project = voltage_divider();
    let resistor = project
        .components
        .values()
        .find(|c| c.reference == "R1")
        .unwrap()
        .id;
    let request = SimulationRequest {
        project,
        revision: 1,
        analysis: Analysis::Dc,
    };
    let config = SweepConfig {
        component: resistor,
        start: 100.0,
        stop: 10_000.0,
        points: 3,
        scale: SweepScale::Log,
    };
    let result = dc_sweep(
        &request,
        &config,
        SimulationEngine::Stable,
        &SimulationControl::default(),
    )
    .unwrap();
    assert!((result.points[1].value - 1000.0).abs() < 1e-9);
    assert_eq!(result.points[2].value, 10_000.0);
    let current = result.points[0]
        .result
        .operating_points
        .iter()
        .find(|p| p.component == resistor)
        .unwrap()
        .current
        .abs();
    assert!((current - 10.0 / 1100.0).abs() < 1e-10);
    for invalid in [
        SweepConfig {
            points: 129,
            ..config.clone()
        },
        SweepConfig {
            points: 1,
            ..config.clone()
        },
        SweepConfig {
            start: 0.0,
            ..config.clone()
        },
        SweepConfig {
            start: -1.0,
            scale: SweepScale::Linear,
            ..config.clone()
        },
    ] {
        assert!(
            dc_sweep(
                &request,
                &invalid,
                SimulationEngine::Stable,
                &SimulationControl::default()
            )
            .is_err()
        );
    }
    let transient = SimulationRequest {
        analysis: Analysis::Transient {
            step: 1e-5,
            stop: 1e-3,
        },
        ..request
    };
    assert!(
        dc_sweep(
            &transient,
            &config,
            SimulationEngine::Stable,
            &SimulationControl::default()
        )
        .is_err()
    );
}
