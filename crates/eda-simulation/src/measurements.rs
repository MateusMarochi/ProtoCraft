//! Medições da interpolação linear das amostras, sem assumir passo uniforme.
use crate::Trace;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeasurementOptions {
    pub from_s: Option<f64>,
    pub to_s: Option<f64>,
    pub threshold: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceMeasurement {
    pub name: String,
    pub reference: Option<String>,
    pub unit: String,
    pub from_s: f64,
    pub to_s: f64,
    pub minimum: f64,
    pub maximum: f64,
    pub peak_to_peak: f64,
    pub mean: f64,
    pub rms: f64,
    pub integral: f64,
    pub threshold: Option<f64>,
    pub duty_fraction: Option<f64>,
    pub rising_crossings_s: Vec<f64>,
    pub mean_period_s: Option<f64>,
    pub frequency_hz: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementError {
    InvalidTrace,
    InvalidWindow,
    IncompatibleReference,
}
impl MeasurementError {
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidTrace => "measurement.invalid_trace",
            Self::InvalidWindow => "measurement.invalid_window",
            Self::IncompatibleReference => "measurement.incompatible_reference",
        }
    }
}
impl fmt::Display for MeasurementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&eda_i18n::text(self.code()))
    }
}
impl std::error::Error for MeasurementError {}

fn validate(trace: &Trace) -> Result<(), MeasurementError> {
    if !(2..=2_000_000).contains(&trace.samples.len())
        || trace
            .samples
            .iter()
            .any(|s| !s[0].is_finite() || !s[1].is_finite())
        || trace.samples.windows(2).any(|s| s[0][0] >= s[1][0])
    {
        return Err(MeasurementError::InvalidTrace);
    }
    Ok(())
}

/// Média e RMS integrados exatamente na interpolação linear, dentro da janela.
/// Frequência usa ao menos duas subidas pelo limiar; não mede precisão do solver.
pub fn measure_trace(
    trace: &Trace,
    options: &MeasurementOptions,
) -> Result<TraceMeasurement, MeasurementError> {
    validate(trace)?;
    let first = trace.samples[0][0];
    let last = trace.samples[trace.samples.len() - 1][0];
    let from = options.from_s.unwrap_or(first);
    let to = options.to_s.unwrap_or(last);
    if !from.is_finite()
        || !to.is_finite()
        || from < first
        || to > last
        || from >= to
        || options.threshold.is_some_and(|v| !v.is_finite())
    {
        return Err(MeasurementError::InvalidWindow);
    }
    let mut integral = 0.0;
    let mut square_integral = 0.0;
    let mut high_time = 0.0;
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    let mut rising_crossings_s = Vec::new();
    let mut threshold_side = None;
    let mut reached_threshold = None;
    for pair in trace.samples.windows(2) {
        let [t0, v0] = pair[0];
        let [t1, v1] = pair[1];
        let a = from.max(t0);
        let b = to.min(t1);
        if a >= b {
            continue;
        }
        let value_at = |t| v0 * (1.0 - (t - t0) / (t1 - t0)) + v1 * ((t - t0) / (t1 - t0));
        let va = value_at(a);
        let vb = value_at(b);
        let dt = b - a;
        minimum = minimum.min(va).min(vb);
        maximum = maximum.max(va).max(vb);
        integral += dt * (va / 2.0 + vb / 2.0);
        square_integral += dt * (va * va + va * vb + vb * vb) / 3.0;
        if let Some(threshold) = options.threshold {
            let side = threshold_side.get_or_insert({
                if va < threshold {
                    -1
                } else if va > threshold {
                    1
                } else {
                    0
                }
            });
            if va >= threshold && vb >= threshold {
                high_time += dt;
            } else if (va < threshold) != (vb < threshold) {
                let crossing = a + dt * ((threshold - va) / (vb - va));
                if va < threshold {
                    high_time += b - crossing;
                } else {
                    high_time += crossing - a;
                }
            }
            if va < threshold && vb >= threshold {
                reached_threshold = Some(a + dt * ((threshold - va) / (vb - va)));
            }
            // Um toque no limiar só é uma subida se depois alcançar valor estritamente maior.
            if vb > threshold {
                if *side < 0
                    && let Some(crossing) = reached_threshold
                    && crossing > from
                {
                    rising_crossings_s.push(crossing);
                }
                *side = 1;
                reached_threshold = None;
            } else if vb < threshold {
                *side = -1;
                reached_threshold = None;
            }
        }
    }
    let duration = to - from;
    let period = if rising_crossings_s.len() >= 2 {
        let n = rising_crossings_s.len();
        Some((rising_crossings_s[n - 1] - rising_crossings_s[0]) / (n - 1) as f64)
    } else {
        None
    };
    let result = TraceMeasurement {
        name: trace.name.clone(),
        reference: None,
        unit: trace.unit.clone(),
        from_s: from,
        to_s: to,
        minimum,
        maximum,
        peak_to_peak: maximum - minimum,
        mean: integral / duration,
        rms: (square_integral / duration).sqrt(),
        integral,
        threshold: options.threshold,
        duty_fraction: options.threshold.map(|_| high_time / duration),
        rising_crossings_s,
        mean_period_s: period,
        frequency_hz: period.map(|p| 1.0 / p),
    };
    if [
        result.minimum,
        result.maximum,
        result.peak_to_peak,
        result.mean,
        result.rms,
        result.integral,
    ]
    .iter()
    .any(|v| !v.is_finite())
        || result.frequency_hz.is_some_and(|v| !v.is_finite())
    {
        return Err(MeasurementError::InvalidTrace);
    }
    Ok(result)
}

/// Medição diferencial exige a mesma unidade e instantes, sem ressincronização implícita.
pub fn measure_difference(
    trace: &Trace,
    reference: &Trace,
    options: &MeasurementOptions,
) -> Result<TraceMeasurement, MeasurementError> {
    validate(trace)?;
    validate(reference)?;
    if trace.unit != reference.unit
        || trace.samples.len() != reference.samples.len()
        || trace
            .samples
            .iter()
            .zip(&reference.samples)
            .any(|(a, b)| a[0] != b[0])
    {
        return Err(MeasurementError::IncompatibleReference);
    }
    let difference = Trace {
        name: trace.name.clone(),
        unit: trace.unit.clone(),
        samples: trace
            .samples
            .iter()
            .zip(&reference.samples)
            .map(|(a, b)| [a[0], a[1] - b[1]])
            .collect(),
    };
    let mut measured = measure_trace(&difference, options)?;
    measured.reference = Some(reference.name.clone());
    Ok(measured)
}
