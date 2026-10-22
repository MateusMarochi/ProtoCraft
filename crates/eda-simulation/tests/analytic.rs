use eda_core::{
    Component, ComponentId, ComponentKind, LabelId, NetLabel, Point, Project, Wire, WireId,
};
use eda_simulation::{Analysis, SimulationError, SimulationRequest, export_spice, simulate};
fn series(load: ComponentKind, value: f64) -> Project {
    let mut p = Project::empty("Teste analítico");
    for (id, reference, kind, value, x, y, rotation) in [
        (1, "V1", ComponentKind::VoltageSource, 10.0, 0, 0, 0),
        (2, "R1", ComponentKind::Resistor, 1000.0, -3, 6, 1),
        (3, "X1", load, value, -3, 18, 1),
        (4, "G1", ComponentKind::Ground, 0.0, 3, 0, 0),
    ] {
        let id = ComponentId(id);
        p.components.insert(
            id,
            Component {
                parameters: Default::default(),
                device: None,
                footprint: None,
                imported: None,
                id,
                reference: reference.into(),
                kind,
                value,
                position: Point { x, y },
                rotation,
            },
        );
    }
    for (id, x1, y1, x2, y2) in [
        (10, -3, 0, -3, 3),
        (11, -3, 9, -3, 15),
        (12, -3, 21, 3, 21),
        (13, 3, 21, 3, 0),
    ] {
        let id = WireId(id);
        p.wires.insert(
            id,
            Wire {
                id,
                start: Point { x: x1, y: y1 },
                end: Point { x: x2, y: y2 },
            },
        );
    }
    let id = LabelId(20);
    p.labels.insert(
        id,
        NetLabel {
            id,
            position: Point { x: -3, y: 12 },
            name: "OUT".into(),
        },
    );
    p.next_id = 100;
    p
}
fn run(p: Project, analysis: Analysis) -> eda_simulation::SimulationResult {
    simulate(&SimulationRequest {
        project: p,
        revision: 42,
        analysis,
    })
    .expect("simulação da fixture")
}
fn output(r: &eda_simulation::SimulationResult) -> &eda_simulation::Trace {
    r.traces
        .iter()
        .find(|t| t.name == "V(OUT)")
        .expect("rede OUT")
}
#[test]
fn divider_five_volts_and_ohm() {
    let r = run(series(ComponentKind::Resistor, 1000.0), Analysis::Dc);
    assert_eq!(r.revision, 42);
    assert!((output(&r).samples[0][1] - 5.0).abs() < 1e-9);
    let source = r
        .traces
        .iter()
        .find(|t| t.name == "I(V1)")
        .expect("corrente");
    assert!((source.samples[0][1] + 0.005).abs() < 1e-12);
}
#[test]
fn rc_tau_one_millisecond() {
    let r = run(
        series(ComponentKind::Capacitor, 1e-6),
        Analysis::Transient {
            step: 1e-5,
            stop: 0.005,
        },
    );
    let out = output(&r);
    assert_eq!(out.samples.len(), 501);
    for [time, value] in &out.samples {
        let expected = 10.0 * (1.0 - (-time / 0.001).exp());
        assert!(
            (value - expected).abs() < 0.02,
            "t={time}: {value} vs {expected}"
        );
    }
    assert!((out.samples[100][1] - 6.321205588).abs() < 0.02);
}
#[test]
fn inductor_dc_short_and_rl_transient() {
    let p = series(ComponentKind::Inductor, 1.0);
    let dc = run(p.clone(), Analysis::Dc);
    assert!(output(&dc).samples[0][1].abs() < 1e-10);
    let r = run(
        p,
        Analysis::Transient {
            step: 1e-5,
            stop: 0.005,
        },
    );
    for [time, value] in output(&r).samples.iter().skip(1) {
        assert!((value - 10.0 * (-time / 0.001).exp()).abs() < 0.02);
    }
}
#[test]
fn diode_matches_shockley_and_kcl() {
    let r = run(series(ComponentKind::Diode, 1e-12), Analysis::Dc);
    let v = output(&r).samples[0][1];
    assert!(v > 0.5 && v < 0.7);
    assert!(((10.0 - v) / 1000.0 - 1e-12 * ((v / 0.02585).exp() - 1.0)).abs() < 1e-9);
}
#[test]
fn missing_ground_and_singular_report_errors() {
    let mut p = series(ComponentKind::Resistor, 1000.0);
    p.components.remove(&ComponentId(4));
    assert!(matches!(
        simulate(&SimulationRequest {
            project: p,
            revision: 0,
            analysis: Analysis::Dc
        }),
        Err(SimulationError::MissingGround)
    ));
    let mut p = series(ComponentKind::Capacitor, 1e-6);
    p.components.remove(&ComponentId(2));
    assert!(matches!(
        simulate(&SimulationRequest {
            project: p,
            revision: 0,
            analysis: Analysis::Dc
        }),
        Err(SimulationError::Singular)
    ));
}
#[test]
fn invalid_values_and_excess_steps_fail() {
    let mut p = series(ComponentKind::Resistor, 1000.0);
    p.components.get_mut(&ComponentId(2)).expect("R1").value = f64::NAN;
    assert!(
        simulate(&SimulationRequest {
            project: p,
            revision: 0,
            analysis: Analysis::Dc
        })
        .is_err()
    );
    assert!(
        export_spice(
            &series(ComponentKind::Resistor, 1000.0),
            Analysis::Transient {
                step: 1e-9,
                stop: 1.0
            }
        )
        .is_err()
    );
}
#[test]
fn spice_does_not_emit_external_names() {
    let mut p = series(ComponentKind::Diode, 1e-12);
    p.name = ".include evil".into();
    let out = export_spice(&p, Analysis::Dc).expect("exportação");
    assert!(!out.contains("evil"));
    assert!(out.contains(".model DGEN3 D(Is="));
    assert!(out.ends_with(".end\n"));
}
#[test]
fn current_source_and_open_capacitor_dc() {
    let mut p = series(ComponentKind::Resistor, 1000.0);
    let source = p.components.get_mut(&ComponentId(1)).expect("fonte");
    source.kind = ComponentKind::CurrentSource;
    source.reference = "I1".into();
    source.value = -0.005;
    let r = run(p, Analysis::Dc);
    assert!((output(&r).samples[0][1] - 5.0).abs() < 1e-9);
    let dc = run(series(ComponentKind::Capacitor, 1e-6), Analysis::Dc);
    assert!((output(&dc).samples[0][1] - 10.0).abs() < 1e-9);
}
#[test]
fn rounded_step_count_is_finite_and_deterministic() {
    let p = series(ComponentKind::Capacitor, 1e-6);
    let analysis = Analysis::Transient {
        step: 0.01,
        stop: 0.07,
    };
    let a = run(p.clone(), analysis);
    let b = run(p, analysis);
    assert_eq!(output(&a).samples.len(), 8);
    assert_eq!(output(&a).samples, output(&b).samples);
    assert!((output(&a).samples.last().expect("amostra")[0] - 0.07).abs() < 1e-15);
}
#[test]
fn conflicting_ideal_sources_are_singular() {
    let mut p = series(ComponentKind::Resistor, 1000.0);
    let mut source = p.components[&ComponentId(1)].clone();
    source.id = ComponentId(5);
    source.reference = "V2".into();
    source.value = 11.0;
    p.components.insert(source.id, source);
    assert!(matches!(
        simulate(&SimulationRequest {
            project: p,
            revision: 0,
            analysis: Analysis::Dc
        }),
        Err(SimulationError::Singular)
    ));
}
#[test]
fn cancelled_control_shared_by_clone() {
    use eda_simulation::{SimulationControl, simulate_controlled};
    let control = SimulationControl::default();
    let other = control.clone();
    other.cancel();
    let request = SimulationRequest {
        project: series(ComponentKind::Resistor, 1000.0),
        revision: 42,
        analysis: Analysis::Dc,
    };
    assert!(matches!(
        simulate_controlled(&request, &control),
        Err(SimulationError::Cancelled)
    ));
}
#[test]
fn zero_timeout_and_backend_trait() {
    use eda_simulation::{MnaBackend, SimulationBackend, SimulationControl};
    use std::time::Duration;
    let backend: &dyn SimulationBackend = &MnaBackend;
    let request = SimulationRequest {
        project: series(ComponentKind::Resistor, 1000.0),
        revision: 42,
        analysis: Analysis::Dc,
    };
    assert!(matches!(
        backend.run(&request, &SimulationControl::new(Duration::ZERO)),
        Err(SimulationError::Timeout)
    ));
    assert!(backend.run(&request, &SimulationControl::default()).is_ok());
}
