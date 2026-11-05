use eda_simulation::{Trace, measurements::*};

fn trace(samples: &[[f64; 2]]) -> Trace {
    Trace {
        name: "V(out)".into(),
        unit: "V".into(),
        samples: samples.to_vec(),
    }
}
fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-12, "{actual} vs {expected}");
}

#[test]
fn ramp_integrals_and_clipped_window_are_analytical() {
    let signal = trace(&[[0., 0.], [0.2, 0.2], [2., 2.]]);
    let m = measure_trace(&signal, &MeasurementOptions::default()).unwrap();
    near(m.mean, 1.);
    near(m.rms, (4f64 / 3.).sqrt());
    near(m.integral, 2.);
    near(m.peak_to_peak, 2.);
    let m = measure_trace(
        &signal,
        &MeasurementOptions {
            from_s: Some(0.5),
            to_s: Some(1.5),
            threshold: Some(1.),
        },
    )
    .unwrap();
    near(m.mean, 1.);
    near(m.rms, (13f64 / 12.).sqrt());
    near(m.duty_fraction.unwrap(), 0.5);
    assert_eq!(m.rising_crossings_s, vec![1.]);
    assert!(m.frequency_hz.is_none());
}

#[test]
fn irregular_samples_produce_time_weighted_mean_and_frequency() {
    let signal = trace(&[
        [0., -1.],
        [0.2, 1.],
        [1., -1.],
        [1.2, 1.],
        [2., -1.],
        [2.2, 1.],
        [3., -1.],
    ]);
    let m = measure_trace(
        &signal,
        &MeasurementOptions {
            threshold: Some(0.),
            ..Default::default()
        },
    )
    .unwrap();
    near(m.mean, 0.);
    near(m.rms, (1f64 / 3.).sqrt());
    near(m.duty_fraction.unwrap(), 0.5);
    near(m.frequency_hz.unwrap(), 1.);
    assert_eq!(m.rising_crossings_s.len(), 3);
}

#[test]
fn differential_measurements_require_matching_units_and_instants() {
    let signal = trace(&[[0., 3.], [1., 5.]]);
    let mut reference = trace(&[[0., 2.], [1., 2.]]);
    let m = measure_difference(&signal, &reference, &Default::default()).unwrap();
    near(m.mean, 2.);
    assert_eq!(m.reference.as_deref(), Some("V(out)"));
    reference.samples[1][0] = 2.;
    assert_eq!(
        measure_difference(&signal, &reference, &Default::default()).unwrap_err(),
        MeasurementError::IncompatibleReference
    );
    reference.samples[1][0] = 1.;
    reference.unit = "A".into();
    assert!(measure_difference(&signal, &reference, &Default::default()).is_err());
}

#[test]
fn rejects_nonfinite_duplicate_reversed_and_outside_windows() {
    for samples in [
        vec![],
        vec![[0., 1.]],
        vec![[0., 1.], [0., 2.]],
        vec![[1., 1.], [0., 2.]],
        vec![[0., f64::NAN], [1., 2.]],
    ] {
        assert!(measure_trace(&trace(&samples), &Default::default()).is_err());
    }
    for options in [
        MeasurementOptions {
            from_s: Some(-1.),
            ..Default::default()
        },
        MeasurementOptions {
            to_s: Some(2.),
            ..Default::default()
        },
        MeasurementOptions {
            threshold: Some(f64::INFINITY),
            ..Default::default()
        },
        MeasurementOptions {
            from_s: Some(1.),
            to_s: Some(1.),
            ..Default::default()
        },
    ] {
        assert!(measure_trace(&trace(&[[0., 1.], [1., 1.]]), &options).is_err());
    }
}

#[test]
fn tangent_touches_and_plateaus_require_a_confirmed_rising_transition() {
    let options = MeasurementOptions {
        threshold: Some(0.),
        ..Default::default()
    };
    for signal in [
        trace(&[[0., -1.], [1., 0.], [2., -1.], [3., 0.], [4., -1.]]),
        trace(&[[0., -1.], [1., 0.], [2., 0.], [3., -1.]]),
    ] {
        let m = measure_trace(&signal, &options).unwrap();
        assert!(m.rising_crossings_s.is_empty());
        assert!(m.frequency_hz.is_none());
    }
    let signal = trace(&[[0., -1.], [1., 0.], [2., 0.], [3., 1.]]);
    assert_eq!(
        measure_trace(&signal, &options).unwrap().rising_crossings_s,
        vec![1.]
    );
    let clipped = MeasurementOptions {
        from_s: Some(1.5),
        ..options
    };
    assert!(
        measure_trace(&signal, &clipped)
            .unwrap()
            .rising_crossings_s
            .is_empty()
    );
}

#[test]
fn constant_zero_and_overflow_have_explicit_results() {
    let signal = trace(&[[0., -2.], [0.1, -2.], [10., -2.]]);
    let m = measure_trace(
        &signal,
        &MeasurementOptions {
            threshold: Some(-3.),
            ..Default::default()
        },
    )
    .unwrap();
    near(m.rms, 2.);
    near(m.mean, -2.);
    near(m.duty_fraction.unwrap(), 1.);
    let m = measure_trace(
        &signal,
        &MeasurementOptions {
            threshold: Some(0.),
            ..Default::default()
        },
    )
    .unwrap();
    near(m.duty_fraction.unwrap(), 0.);
    near(
        measure_difference(&signal, &signal, &Default::default())
            .unwrap()
            .rms,
        0.,
    );
    assert!(measure_trace(&trace(&[[0., 1e308], [1., 1e308]]), &Default::default()).is_err());
}
