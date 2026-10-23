use eda_core::{
    Component, ComponentId, ComponentKind as K, LabelId, ModelParameters, NetLabel, Point, Project,
};
use eda_simulation::{
    Analysis, SimulationControl, SimulationEngine, SimulationRequest, TransientSession,
    simulate_with_engine,
};

fn circuit(elements: &[(K, f64, &[&str])]) -> Project {
    let mut p = Project::empty("Modelos analíticos");
    for (index, (kind, value, nets)) in elements.iter().enumerate() {
        let id = ComponentId(index as u64 + 1);
        let c = Component {
            id,
            reference: format!("X{}", id.0),
            kind: *kind,
            value: *value,
            position: Point::new(index as i32 * 12, 0),
            rotation: 0,
            device: None,
            footprint: None,
            imported: None,
            parameters: ModelParameters::default(),
        };
        for (pin, name) in c.pins().iter().zip(*nets) {
            let label_id = LabelId(1000 + p.labels.len() as u64);
            p.labels.insert(
                label_id,
                NetLabel {
                    id: label_id,
                    name: (*name).into(),
                    position: pin.position,
                },
            );
        }
        p.components.insert(id, c);
    }
    let id = ComponentId(900);
    p.components.insert(
        id,
        Component {
            id,
            reference: "GND1".into(),
            kind: K::Ground,
            value: 0.0,
            position: Point::new(-100, 0),
            rotation: 0,
            device: None,
            footprint: None,
            imported: None,
            parameters: Default::default(),
        },
    );
    p.labels.insert(
        LabelId(999),
        NetLabel {
            id: LabelId(999),
            name: "0".into(),
            position: Point::new(-100, 0),
        },
    );
    p.next_id = 10000;
    p.validate().unwrap();
    p
}
fn run(p: Project, a: Analysis, e: SimulationEngine) -> eda_simulation::SimulationResult {
    simulate_with_engine(
        &SimulationRequest {
            project: p,
            revision: 7,
            analysis: a,
        },
        e,
        &SimulationControl::default(),
    )
    .unwrap()
}
fn out(r: &eda_simulation::SimulationResult, name: &str) -> f64 {
    r.traces
        .iter()
        .find(|t| t.name == name)
        .unwrap()
        .samples
        .last()
        .unwrap()[1]
}

#[test]
fn both_integrators_match_rc_solution_and_dynamic_is_second_order() {
    let p = circuit(&[
        (K::VoltageSource, 10.0, &["IN", "0"]),
        (K::Resistor, 1000.0, &["IN", "OUT"]),
        (K::Capacitor, 1e-6, &["OUT", "0"]),
    ]);
    let a = Analysis::Transient {
        step: 1e-5,
        stop: 0.001,
    };
    let exact = 10.0 * (1.0 - (-1.0_f64).exp());
    let stable = run(p.clone(), a, SimulationEngine::Stable);
    let dynamic = run(p, a, SimulationEngine::Dynamic);
    let error_stable = (out(&stable, "V(OUT)") - exact).abs();
    let error_dynamic = (out(&dynamic, "V(OUT)") - exact).abs();
    assert!(error_stable < 0.02);
    assert!(error_dynamic < 0.001 && error_dynamic < error_stable / 10.0);
    assert_ne!(stable.backend, dynamic.backend);
    assert!((out(&dynamic, "I(X2)") - out(&dynamic, "I(X3)")).abs() < 1e-9);
}
#[test]
fn waveform_sources_follow_frequency_offset_phase_and_duty() {
    for (kind, expected) in [
        (K::SineVoltage, 3.0),
        (K::SquareVoltage, 3.0),
        (K::TriangleVoltage, 1.0),
        (K::SawVoltage, 0.0),
    ] {
        let mut p = circuit(&[
            (kind, 2.0, &["OUT", "0"]),
            (K::Resistor, 1000.0, &["OUT", "0"]),
        ]);
        p.components
            .get_mut(&ComponentId(1))
            .unwrap()
            .parameters
            .offset_v = 1.0;
        let r = run(
            p.clone(),
            Analysis::Transient {
                step: 0.000025,
                stop: 0.00025,
            },
            SimulationEngine::Dynamic,
        );
        assert!((out(&r, "V(OUT)") - expected).abs() < 1e-10, "{kind:?}");
        let dc = run(p, Analysis::Dc, SimulationEngine::Stable);
        assert!((out(&dc, "V(OUT)") - 1.0).abs() < 1e-12);
    }
    let mut p = circuit(&[
        (K::SquareVoltage, 5.0, &["OUT", "0"]),
        (K::Resistor, 1000.0, &["OUT", "0"]),
    ]);
    p.components
        .get_mut(&ComponentId(1))
        .unwrap()
        .parameters
        .duty_cycle = 0.1;
    assert_eq!(
        out(
            &run(
                p,
                Analysis::Transient {
                    step: 0.00001,
                    stop: 0.0002
                },
                SimulationEngine::Stable
            ),
            "V(OUT)"
        ),
        -5.0
    );
}
#[test]
fn potentiometer_and_switch_have_finite_documented_resistances() {
    let mut p = circuit(&[
        (K::VoltageSource, 10.0, &["IN", "0"]),
        (K::Potentiometer, 10000.0, &["IN", "0", "OUT"]),
    ]);
    p.components
        .get_mut(&ComponentId(2))
        .unwrap()
        .parameters
        .wiper = 0.25;
    assert!((out(&run(p, Analysis::Dc, SimulationEngine::Stable), "V(OUT)") - 7.5).abs() < 1e-9);
    let mut p = circuit(&[
        (K::VoltageSource, 10.0, &["IN", "0"]),
        (K::Switch, 0.0, &["IN", "OUT"]),
        (K::Resistor, 1000.0, &["OUT", "0"]),
    ]);
    assert!(
        out(
            &run(p.clone(), Analysis::Dc, SimulationEngine::Stable),
            "V(OUT)"
        ) < 0.00002
    );
    p.components
        .get_mut(&ComponentId(2))
        .unwrap()
        .parameters
        .closed = true;
    assert!((out(&run(p, Analysis::Dc, SimulationEngine::Stable), "V(OUT)") - 10.0).abs() < 0.001);
}
#[test]
fn led_and_zener_satisfy_kcl_and_show_forward_and_reverse_conduction() {
    for (kind, value, supply, low, high) in [
        (K::Led, 1e-30, 5.0, 1.5, 1.9),
        (K::Zener, 5.1, -10.0, -6.0, -5.1),
    ] {
        let p = circuit(&[
            (K::VoltageSource, supply, &["IN", "0"]),
            (K::Resistor, 1000.0, &["IN", "OUT"]),
            (kind, value, &["OUT", "0"]),
        ]);
        let r = run(p, Analysis::Dc, SimulationEngine::Stable);
        let u = out(&r, "V(OUT)");
        assert!(u > low && u < high, "{kind:?}: {u}");
        assert!((out(&r, "I(X2)") - out(&r, "I(X3)")).abs() < 1e-8);
        assert!((out(&r, "P(X2)") + out(&r, "P(X3)") + out(&r, "P(X1)")).abs() < 1e-7);
    }
}
#[test]
fn transistor_models_respect_beta_and_kcl_in_both_polarities() {
    for (kind, sign) in [(K::Npn, 1.0), (K::Pnp, -1.0)] {
        let p = circuit(&[
            (K::VoltageSource, sign * 10.0, &["VCC", "0"]),
            (K::VoltageSource, sign, &["BIAS", "0"]),
            (K::Resistor, 100000.0, &["BIAS", "BASE"]),
            (K::Resistor, 1000.0, &["VCC", "COL"]),
            (kind, 100.0, &["BASE", "COL", "0"]),
        ]);
        let r = run(p, Analysis::Dc, SimulationEngine::Dynamic);
        let ib = out(&r, "I(X3)");
        let ic = out(&r, "I(X5)");
        assert!(ic * sign > 0.0001 && ic * sign < 0.001);
        assert!((ic / ib - 100.0).abs() < 0.01, "{kind:?}: ic={ic} ib={ib}");
        assert!((ic - out(&r, "I(X4)")).abs() < 1e-9);
    }
}
#[test]
fn opamp_follower_and_rail_saturation_include_output_resistance() {
    let p = circuit(&[
        (K::VoltageSource, 1.0, &["IN", "0"]),
        (K::OpAmp, 0.0, &["IN", "OUT", "OUT"]),
        (K::Resistor, 1000.0, &["OUT", "0"]),
    ]);
    assert!((out(&run(p, Analysis::Dc, SimulationEngine::Stable), "V(OUT)") - 1.0).abs() < 0.0001);
    let p = circuit(&[
        (K::VoltageSource, 1.0, &["IN", "0"]),
        (K::OpAmp, 0.0, &["IN", "0", "OUT"]),
        (K::Resistor, 1000.0, &["OUT", "0"]),
    ]);
    assert!(
        (out(&run(p, Analysis::Dc, SimulationEngine::Stable), "V(OUT)") - 15.0 * 1000.0 / 1100.0)
            .abs()
            < 1e-7
    );
}
#[test]
fn logic_gates_pass_complete_truth_tables() {
    for kind in [
        K::LogicNot,
        K::LogicAnd,
        K::LogicOr,
        K::LogicXor,
        K::LogicNand,
        K::LogicNor,
    ] {
        for a in [false, true] {
            for b in [false, true] {
                let pins: &[&str] = if kind == K::LogicNot {
                    &["A", "OUT"]
                } else {
                    &["A", "B", "OUT"]
                };
                let mut p = circuit(&[
                    (K::LogicInput, 5.0, &["A", "0"]),
                    (K::LogicInput, 5.0, &["B", "0"]),
                    (kind, 5.0, pins),
                    (K::Resistor, 10000.0, &["OUT", "0"]),
                ]);
                p.components
                    .get_mut(&ComponentId(1))
                    .unwrap()
                    .parameters
                    .closed = a;
                p.components
                    .get_mut(&ComponentId(2))
                    .unwrap()
                    .parameters
                    .closed = b;
                let expected = match kind {
                    K::LogicNot => !a,
                    K::LogicAnd => a && b,
                    K::LogicOr => a || b,
                    K::LogicXor => a ^ b,
                    K::LogicNand => !(a && b),
                    K::LogicNor => !(a || b),
                    _ => false,
                };
                let value = out(&run(p, Analysis::Dc, SimulationEngine::Stable), "V(OUT)");
                assert_eq!(value > 2.5, expected, "{kind:?} {a} {b}: {value}");
            }
        }
    }
}
#[test]
fn live_session_preserves_capacitor_state_after_parameter_edit_and_bounds_history() {
    let mut p = circuit(&[
        (K::VoltageSource, 10.0, &["IN", "0"]),
        (K::Resistor, 1000.0, &["IN", "OUT"]),
        (K::Capacitor, 1e-6, &["OUT", "0"]),
    ]);
    let mut session = TransientSession::new(p.clone(), 0, SimulationEngine::Dynamic, 1e-5).unwrap();
    let result = session.advance(100, &SimulationControl::default()).unwrap();
    let before = out(&result, "V(OUT)");
    p.components.get_mut(&ComponentId(1)).unwrap().value = 0.0;
    assert!(!session.update_project(p.clone(), 1).unwrap());
    let result = session.advance(1, &SimulationControl::default()).unwrap();
    assert!(out(&result, "V(OUT)") < before && out(&result, "V(OUT)") > before * 0.98);
    let result = session
        .advance(1000, &SimulationControl::default())
        .unwrap();
    assert!(result.traces.iter().all(|t| t.samples.len() <= 512));
    p.components.get_mut(&ComponentId(2)).unwrap().position.x += 1;
    assert!(session.update_project(p, 2).unwrap());
    assert_eq!(session.time(), 0.0);
}

#[test]
fn all_original_examples_validate_and_run_with_supported_engines() {
    for (name, _, factory) in eda_core::examples::CATALOG {
        let p = factory();
        p.validate().unwrap();
        let netlist = eda_netlist::build_netlist(&p);
        let diagnostics = eda_netlist::erc(&p, &netlist);
        assert!(
            !diagnostics
                .iter()
                .any(|d| matches!(d.severity, eda_netlist::Severity::Error)),
            "{name}: {diagnostics:?}"
        );
        for engine in SimulationEngine::all() {
            if p.components.values().any(|c| c.kind.requires_mixed())
                && *engine != SimulationEngine::Mixed
            {
                continue;
            }
            if !matches!(*name, "555-astavel" | "555-led-1hz") {
                run(p.clone(), Analysis::Dc, *engine);
            }
            let r = run(
                p.clone(),
                Analysis::Transient {
                    step: 1e-5,
                    stop: 0.0002,
                },
                *engine,
            );
            assert!(
                r.traces
                    .iter()
                    .flat_map(|t| &t.samples)
                    .all(|s| s[1].is_finite()),
                "{name}"
            );
        }
    }
}

#[test]
fn rlc_matches_damped_step_and_dynamic_has_less_numerical_damping() {
    let p = circuit(&[
        (K::VoltageSource, 1.0, &["IN", "0"]),
        (K::Resistor, 10.0, &["IN", "MID"]),
        (K::Inductor, 1e-3, &["MID", "OUT"]),
        (K::Capacitor, 1e-6, &["OUT", "0"]),
    ]);
    let stop = 0.0001;
    let alpha = 5000.0_f64;
    let wd = (1e9 - alpha * alpha).sqrt();
    let exact = 1.0 - (-alpha * stop).exp() * ((wd * stop).cos() + alpha / wd * (wd * stop).sin());
    let dynamic = run(
        p.clone(),
        Analysis::Transient { step: 1e-6, stop },
        SimulationEngine::Dynamic,
    );
    let stable = run(
        p,
        Analysis::Transient { step: 1e-6, stop },
        SimulationEngine::Stable,
    );
    let error_dynamic = (out(&dynamic, "V(OUT)") - exact).abs();
    assert!(error_dynamic < 0.002);
    assert!(error_dynamic < (out(&stable, "V(OUT)") - exact).abs());
}

#[test]
fn spice_never_silently_omits_a_new_component() {
    for kind in [
        K::Switch,
        K::Potentiometer,
        K::Led,
        K::Zener,
        K::Npn,
        K::Pnp,
        K::SineVoltage,
        K::SquareVoltage,
    ] {
        let pins = if kind.pin_count() == 3 {
            vec!["IN", "OUT", "0"]
        } else {
            vec!["IN", "OUT"]
        };
        let p = circuit(&[
            (K::VoltageSource, 5.0, &["IN", "0"]),
            (kind, kind.default_value(), &pins),
            (K::Resistor, 1000.0, &["OUT", "0"]),
        ]);
        let spice = eda_simulation::export_spice(&p, Analysis::Dc).unwrap();
        let element = match kind {
            K::Switch => "R2 ",
            K::Potentiometer => "R2A ",
            K::Led | K::Zener => "D2 ",
            K::Npn | K::Pnp => "Q2 ",
            _ => "V2 ",
        };
        assert!(spice.contains(element), "{kind:?}: {spice}");
    }
    for kind in [K::TriangleVoltage, K::SawVoltage, K::LogicAnd, K::OpAmp] {
        let pins = if kind.pin_count() == 3 {
            vec!["IN", "0", "OUT"]
        } else {
            vec!["IN", "OUT"]
        };
        let p = circuit(&[
            (K::VoltageSource, 5.0, &["IN", "0"]),
            (kind, kind.default_value(), &pins),
            (K::Resistor, 1000.0, &["OUT", "0"]),
        ]);
        assert!(matches!(
            eda_simulation::export_spice(&p, Analysis::Dc),
            Err(eda_simulation::SimulationError::Unsupported(_))
        ));
    }
}
