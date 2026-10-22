use eda_core::{Component, ComponentId, ComponentKind as K, LabelId, NetLabel, Point, Project};
use eda_simulation::{
    Analysis, SimulationControl, SimulationEngine as E, SimulationRequest, SimulationResult,
    TransientSession, simulate_with_engine,
};
fn circuit(elements: &[(K, f64, &[&str])]) -> Project {
    let mut p = Project::empty("Bancada mista");
    for (index, (kind, value, nets)) in elements.iter().enumerate() {
        let id = ComponentId(index as u64 + 1);
        let c = Component {
            id,
            reference: format!("X{}", id.0),
            kind: *kind,
            value: *value,
            position: Point::new(index as i32 * 20, 0),
            rotation: 0,
            device: None,
            footprint: None,
            imported: None,
            parameters: Default::default(),
        };
        assert_eq!(c.pins().len(), nets.len());
        for (pin, name) in c.pins().iter().zip(*nets) {
            let id = LabelId(1000 + p.labels.len() as u64);
            p.labels.insert(
                id,
                NetLabel {
                    id,
                    name: (*name).into(),
                    position: pin.position,
                },
            );
        }
        p.components.insert(id, c);
    }
    p.components.insert(
        ComponentId(900),
        Component {
            id: ComponentId(900),
            reference: "GND1".into(),
            kind: K::Ground,
            value: 0.0,
            position: Point::new(-20, 0),
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
            position: Point::new(-20, 0),
        },
    );
    p.next_id = 10000;
    p.validate().unwrap();
    p
}
fn run(p: Project, a: Analysis, e: E) -> SimulationResult {
    simulate_with_engine(
        &SimulationRequest {
            project: p,
            revision: 0,
            analysis: a,
        },
        e,
        &SimulationControl::default(),
    )
    .unwrap()
}
fn trace<'a>(r: &'a SimulationResult, name: &str) -> &'a Vec<[f64; 2]> {
    &r.traces.iter().find(|t| t.name == name).unwrap().samples
}
fn last(r: &SimulationResult, name: &str) -> f64 {
    trace(r, name).last().unwrap()[1]
}
fn clock(p: &mut Project, id: u64, frequency: f64) {
    let p = &mut p.components.get_mut(&ComponentId(id)).unwrap().parameters;
    p.frequency_hz = frequency;
    p.offset_v = 2.5;
    p.phase_deg = 180.0;
}

#[test]
fn mosfet_cutoff_triode_saturation_and_both_polarities_match_equations() {
    for (kind, polarity) in [(K::Nmos, 1.0), (K::Pmos, -1.0)] {
        for (gate, drain, expected) in [(1.0, 5.0, 0.0), (4.0, 1.0, 0.0303), (4.0, 5.0, 0.042)] {
            let p = circuit(&[
                (K::VoltageSource, polarity * gate, &["GATE", "0"]),
                (K::VoltageSource, polarity * drain, &["DRAIN", "0"]),
                (kind, 0.02, &["GATE", "DRAIN", "0"]),
            ]);
            for engine in E::all() {
                let r = run(p.clone(), Analysis::Dc, *engine);
                let point = r
                    .operating_points
                    .iter()
                    .find(|p| p.component == ComponentId(3))
                    .unwrap();
                assert!(
                    (point.current - polarity * expected).abs() < 1e-8,
                    "{kind:?}: {}",
                    point.current
                );
                assert!(point.pin_currents.iter().sum::<f64>().abs() < 1e-12);
                assert!((last(&r, "P(X1)") + last(&r, "P(X2)") + last(&r, "P(X3)")).abs() < 1e-8);
            }
        }
    }
}
#[test]
fn controlled_sources_obey_gain_transconductance_and_output_signs() {
    for (kind, value, expected) in [(K::Vcvs, 5.0, 5.0), (K::Vccs, 0.001, -10.0)] {
        let p = circuit(&[
            (K::VoltageSource, 1.0, &["IN", "0"]),
            (kind, value, &["IN", "0", "OUT", "0"]),
            (K::Resistor, 10000.0, &["OUT", "0"]),
        ]);
        let r = run(p, Analysis::Dc, E::Mixed);
        assert!((last(&r, "V(OUT)") - expected).abs() < 1e-9);
        assert!((last(&r, "I(X2)") + last(&r, "I(X3)")).abs() < 1e-12);
    }
}
#[test]
fn comparator_hysteresis_and_supply_current_preserve_kcl() {
    let mut p = circuit(&[
        (K::VoltageSource, 5.0, &["VCC", "0"]),
        (K::VoltageSource, 1.1, &["IN", "0"]),
        (K::VoltageSource, 1.0, &["REF", "0"]),
        (K::Comparator, 0.0, &["IN", "REF", "OUT", "VCC", "0"]),
        (K::Resistor, 1000.0, &["OUT", "0"]),
    ]);
    let mut session = TransientSession::new(p.clone(), 0, E::Mixed, 1e-5).unwrap();
    let high = session.advance(1, &SimulationControl::default()).unwrap();
    assert!(last(&high, "V(OUT)") > 4.5);
    let chip = high
        .operating_points
        .iter()
        .find(|p| p.component == ComponentId(4))
        .unwrap();
    assert!(chip.pin_currents.iter().sum::<f64>().abs() < 1e-12);
    assert!((chip.current - last(&high, "I(X5)")).abs() < 1e-10);
    p.components.get_mut(&ComponentId(2)).unwrap().value = 1.01;
    session.update_project(p.clone(), 1).unwrap();
    assert!(
        last(
            &session.advance(1, &SimulationControl::default()).unwrap(),
            "V(OUT)"
        ) > 4.5
    );
    p.components.get_mut(&ComponentId(2)).unwrap().value = 0.9;
    session.update_project(p, 2).unwrap();
    assert!(
        last(
            &session.advance(1, &SimulationControl::default()).unwrap(),
            "V(OUT)"
        ) < 0.001
    );
}
#[test]
fn schmitt_refines_analog_crossings_and_keeps_propagation_delay() {
    let mut p = circuit(&[
        (K::VoltageSource, 5.0, &["VCC", "0"]),
        (K::TriangleVoltage, 2.5, &["IN", "0"]),
        (K::Schmitt, 0.0, &["IN", "OUT", "VCC", "0"]),
        (K::Resistor, 10000.0, &["OUT", "0"]),
    ]);
    p.components
        .get_mut(&ComponentId(2))
        .unwrap()
        .parameters
        .offset_v = 2.5;
    let r = run(
        p,
        Analysis::Transient {
            step: 0.00008,
            stop: 0.001,
        },
        E::Mixed,
    );
    let samples = trace(&r, "V(OUT)");
    let falls: Vec<_> = samples
        .windows(2)
        .filter(|w| w[0][1] > 2.5 && w[1][1] < 2.5)
        .map(|w| w[1][0])
        .collect();
    assert_eq!(falls.len(), 1);
    assert!(
        (falls[0] - (1.0 / 3000.0 + 1e-7)).abs() < 2e-9,
        "{:?}",
        falls
    );
}
fn divider() -> Project {
    let mut p = circuit(&[
        (K::VoltageSource, 5.0, &["VCC", "0"]),
        (K::SquareVoltage, 2.5, &["CLK", "0"]),
        (
            K::DFlipFlop,
            0.0,
            &["QN", "CLK", "VCC", "VCC", "Q", "QN", "VCC", "0"],
        ),
        (K::Resistor, 10000.0, &["Q", "0"]),
        (K::Resistor, 10000.0, &["QN", "0"]),
    ]);
    clock(&mut p, 2, 100.0);
    p
}
#[test]
fn flip_flop_divides_clock_and_resolves_events_smaller_than_requested_step() {
    let r = run(
        divider(),
        Analysis::Transient {
            step: 0.007,
            stop: 0.05,
        },
        E::Mixed,
    );
    let samples = trace(&r, "V(Q)");
    let rises: Vec<_> = samples
        .windows(2)
        .filter(|w| w[0][1] < 2.5 && w[1][1] > 2.5)
        .map(|w| w[1][0])
        .collect();
    assert_eq!(rises.len(), 3, "{rises:?}");
    assert!((rises[0] - (0.005 + 1e-7)).abs() < 2e-11, "{rises:?}");
    assert!((rises[1] - rises[0] - 0.02).abs() < 2e-11);
    assert!(
        r.operating_points
            .iter()
            .find(|p| p.component == ComponentId(3))
            .unwrap()
            .digital_state
            == Some(1)
    );
}
#[test]
fn counters_wrap_and_decade_outputs_are_one_hot_despite_coarse_step() {
    for (kind, count) in [(K::Counter4, 1), (K::Counter10, 7)] {
        let mut pins = vec!["CLK", "0", "VCC", "VCC", "0"];
        let names = ["Q0", "Q1", "Q2", "Q3", "Q4", "Q5", "Q6", "Q7", "Q8", "Q9"];
        pins.extend_from_slice(&names[..if kind == K::Counter4 { 4 } else { 10 }]);
        let mut p = circuit(&[
            (K::VoltageSource, 5.0, &["VCC", "0"]),
            (K::SquareVoltage, 2.5, &["CLK", "0"]),
            (kind, 0.0, &pins),
        ]);
        clock(&mut p, 2, 1000.0);
        let r = run(
            p,
            Analysis::Transient {
                step: 0.004,
                stop: 0.017,
            },
            E::Mixed,
        );
        let chip = r
            .operating_points
            .iter()
            .find(|p| p.component == ComponentId(3))
            .unwrap();
        assert_eq!(chip.digital_state, Some(count));
        if kind == K::Counter10 {
            assert_eq!(
                chip.pin_voltages[5..].iter().filter(|v| **v > 2.5).count(),
                1
            );
            assert!(chip.pin_voltages[12] > 2.5);
        }
    }
}
#[test]
fn timer_astable_period_matches_rc() {
    let p = circuit(&[
        (K::VoltageSource, 5.0, &["VCC", "0"]),
        (
            K::Timer555,
            0.0,
            &["0", "CAP", "OUT", "VCC", "CTRL", "CAP", "DIS", "VCC"],
        ),
        (K::Resistor, 10000.0, &["VCC", "DIS"]),
        (K::Resistor, 10000.0, &["DIS", "CAP"]),
        (K::Capacitor, 1e-6, &["CAP", "0"]),
        (K::Resistor, 100000.0, &["OUT", "0"]),
    ]);
    let r = run(
        p,
        Analysis::Transient {
            step: 0.0002,
            stop: 0.09,
        },
        E::Mixed,
    );
    let falls: Vec<_> = trace(&r, "V(OUT)")
        .windows(2)
        .filter(|w| w[0][1] > 2.5 && w[1][1] < 2.5)
        .map(|w| w[1][0])
        .collect();
    assert!(falls.len() >= 3, "{falls:?}");
    let expected = std::f64::consts::LN_2 * 30000.0 * 1e-6;
    assert!(
        (falls[2] - falls[1] - expected).abs() / expected < 0.015,
        "{falls:?}"
    );
    assert!((last(&r, "V(CTRL)") - 10.0 / 3.0).abs() < 1e-6);
}

#[test]
fn timer_monostable_pulse_and_reset_priority_match_rc_behavior() {
    let p = eda_core::examples::CATALOG
        .iter()
        .find(|e| e.0 == "555-monoestavel")
        .unwrap()
        .2();
    let r = run(
        p,
        Analysis::Transient {
            step: 1e-5,
            stop: 0.003,
        },
        E::Mixed,
    );
    let output = trace(&r, "V(VOUT)");
    let rise = output
        .windows(2)
        .find(|w| w[0][1] < 2.5 && w[1][1] > 2.5)
        .unwrap()[1][0];
    let fall = output
        .windows(2)
        .find(|w| w[0][1] > 2.5 && w[1][1] < 2.5)
        .unwrap()[1][0];
    let expected = 3.0_f64.ln() * 10000.0 * 100e-9;
    assert!(
        (fall - rise - expected).abs() / expected < 0.015,
        "{rise} {fall}"
    );

    let mut p = circuit(&[
        (K::VoltageSource, 5.0, &["VCC", "0"]),
        (K::VoltageSource, 5.0, &["RESET", "0"]),
        (
            K::Timer555,
            0.0,
            &["0", "0", "OUT", "RESET", "CTRL", "VCC", "DIS", "VCC"],
        ),
        (K::Resistor, 10000.0, &["VCC", "DIS"]),
    ]);
    let mut session = TransientSession::new(p.clone(), 0, E::Mixed, 1e-5).unwrap();
    assert!(
        last(
            &session.advance(1, &SimulationControl::default()).unwrap(),
            "V(OUT)"
        ) > 4.9
    );
    p.components.get_mut(&ComponentId(2)).unwrap().value = 0.0;
    session.update_project(p, 1).unwrap();
    let r = session.advance(1, &SimulationControl::default()).unwrap();
    assert!(last(&r, "V(OUT)") < 0.001);
    assert!(last(&r, "V(DIS)") < 0.001);
}

#[test]
fn async_preset_clear_and_unpowered_outputs_do_not_create_energy() {
    let mut p = circuit(&[
        (K::VoltageSource, 5.0, &["VCC", "0"]),
        (K::VoltageSource, 0.0, &["PRE", "0"]),
        (K::VoltageSource, 5.0, &["CLR", "0"]),
        (
            K::DFlipFlop,
            0.0,
            &["0", "0", "PRE", "CLR", "Q", "QN", "VCC", "0"],
        ),
        (K::Resistor, 10000.0, &["Q", "0"]),
    ]);
    let mut session = TransientSession::new(p.clone(), 0, E::Mixed, 1e-5).unwrap();
    assert!(
        last(
            &session.advance(1, &SimulationControl::default()).unwrap(),
            "V(Q)"
        ) > 4.9
    );
    p.components.get_mut(&ComponentId(2)).unwrap().value = 5.0;
    p.components.get_mut(&ComponentId(3)).unwrap().value = 0.0;
    session.update_project(p.clone(), 1).unwrap();
    let r = session.advance(1, &SimulationControl::default()).unwrap();
    assert!(last(&r, "V(Q)") < 0.001);
    assert!(last(&r, "V(QN)") > 4.9);
    p.components.get_mut(&ComponentId(1)).unwrap().value = 0.0;
    session.update_project(p, 2).unwrap();
    let r = session.advance(1, &SimulationControl::default()).unwrap();
    assert!(last(&r, "V(Q)").abs() < 1e-9);
    assert!(last(&r, "V(QN)").abs() < 1e-9);
}

#[test]
fn chunked_execution_retains_pending_edges_and_inertial_delay_filters_short_pulses() {
    let p = divider();
    let mut session = TransientSession::new(p, 0, E::Mixed, 0.001).unwrap();
    let control = SimulationControl::default();
    session.advance(5, &control).unwrap(); // Borda de subida pendente, ainda sem saída.
    assert!((session.time() - 0.005).abs() < 1e-12);
    let r = session.advance(1, &control).unwrap();
    assert!(last(&r, "V(Q)") > 4.9);
    let r = session.advance(10, &control).unwrap();
    assert!(last(&r, "V(Q)") < 0.001);

    let mut p = circuit(&[
        (K::SquareVoltage, 2.5, &["IN", "0"]),
        (K::LogicNot, 5.0, &["IN", "OUT"]),
        (K::Resistor, 10000.0, &["OUT", "0"]),
    ]);
    clock(&mut p, 1, 1000.0);
    p.components
        .get_mut(&ComponentId(1))
        .unwrap()
        .parameters
        .duty_cycle = 0.01;
    let gate = &mut p.components.get_mut(&ComponentId(2)).unwrap().parameters;
    gate.propagation_delay_s = 100e-6;
    gate.initial_state = 1;
    let r = run(
        p,
        Analysis::Transient {
            step: 0.0004,
            stop: 0.003,
        },
        E::Mixed,
    );
    // t=0 é a amostra anterior à aplicação das fontes; depois não deve haver pulso na saída.
    assert!(
        trace(&r, "V(OUT)")
            .iter()
            .filter(|s| s[0] > 0.0)
            .all(|s| s[1] > 4.9)
    );
}

#[test]
fn spice_exports_new_analog_models_and_refuses_behavioral_chips() {
    for (kind, token, pins) in [
        (K::Nmos, "NMOS", vec!["IN", "OUT", "0"]),
        (K::Pmos, "PMOS", vec!["IN", "OUT", "0"]),
        (K::Vcvs, "E2 ", vec!["IN", "0", "OUT", "0"]),
        (K::Vccs, "G2 ", vec!["IN", "0", "OUT", "0"]),
    ] {
        let p = circuit(&[
            (K::VoltageSource, 1.0, &["IN", "0"]),
            (kind, kind.default_value(), &pins),
        ]);
        let text = eda_simulation::export_spice(&p, Analysis::Dc).unwrap();
        assert!(text.contains(token), "{text}");
    }
    for (_, _, factory) in eda_core::examples::CATALOG {
        let p = factory();
        if p.components.values().any(|c| c.kind.requires_mixed()) {
            assert!(matches!(
                eda_simulation::export_spice(&p, Analysis::Dc),
                Err(eda_simulation::SimulationError::Unsupported(_))
            ));
        }
    }
}
#[test]
fn sequential_devices_require_mixed_and_invalid_async_inputs_are_reported() {
    let mut p = divider();
    assert!(
        simulate_with_engine(
            &SimulationRequest {
                project: p.clone(),
                revision: 0,
                analysis: Analysis::Dc
            },
            E::Stable,
            &SimulationControl::default()
        )
        .is_err()
    );
    // Os dois controles ativos simultaneamente não recebem estado lógico inventado.
    p.components.get_mut(&ComponentId(1)).unwrap().value = 0.0;
    // VCC independente para que PRE/CLR em zero não sejam confundidos com falta de alimentação.
    let mut p = circuit(&[
        (K::VoltageSource, 5.0, &["VCC", "0"]),
        (
            K::DFlipFlop,
            0.0,
            &["0", "0", "0", "0", "Q", "QN", "VCC", "0"],
        ),
    ]);
    let result = simulate_with_engine(
        &SimulationRequest {
            project: p.clone(),
            revision: 0,
            analysis: Analysis::Dc,
        },
        E::Mixed,
        &SimulationControl::default(),
    );
    assert!(result.unwrap_err().to_string().contains("PREn e CLRn"));
    p.components
        .get_mut(&ComponentId(2))
        .unwrap()
        .parameters
        .propagation_delay_s = 0.0;
    assert!(p.validate().is_err());
}
