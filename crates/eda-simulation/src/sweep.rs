//! Varredura de valores DC usando o mesmo motor de comandos e solver.
use crate::{
    Analysis, SimulationControl, SimulationEngine, SimulationRequest, SimulationResult,
    simulate_with_engine,
};
use eda_core::{Command, ComponentId, ComponentKind, Engine};
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SweepScale {
    Linear,
    Log,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SweepConfig {
    pub component: ComponentId,
    pub start: f64,
    pub stop: f64,
    pub points: usize,
    pub scale: SweepScale,
}
#[derive(Debug, Clone, Serialize)]
pub struct SweepPoint {
    pub value: f64,
    pub result: SimulationResult,
}
#[derive(Debug, Clone, Serialize)]
pub struct SweepResult {
    pub revision: u64,
    pub component: ComponentId,
    pub points: Vec<SweepPoint>,
}

pub fn dc_sweep(
    request: &SimulationRequest,
    config: &SweepConfig,
    engine: SimulationEngine,
    control: &SimulationControl,
) -> Result<SweepResult, String> {
    let started = Instant::now();
    let check_control = || {
        if control.is_cancelled() {
            Err(eda_i18n::text(
                &crate::SimulationError::Cancelled.to_string(),
            ))
        } else if started.elapsed() >= control.timeout {
            Err(eda_i18n::text(&crate::SimulationError::Timeout.to_string()))
        } else {
            Ok(())
        }
    };
    check_control()?;
    let invalid = || eda_i18n::text("sweep.invalid");
    if request.analysis != Analysis::Dc
        || !(2..=128).contains(&config.points)
        || !config.start.is_finite()
        || !config.stop.is_finite()
        || (matches!(config.scale, SweepScale::Log) && (config.start <= 0.0 || config.stop <= 0.0))
    {
        return Err(invalid());
    }
    let component = request
        .project
        .components
        .get(&config.component)
        .ok_or_else(invalid)?;
    if !matches!(
        component.kind,
        ComponentKind::Resistor | ComponentKind::VoltageSource
    ) {
        return Err(invalid());
    }
    let point_error = |index: usize, error: String| {
        eda_i18n::format(
            "sweep.point_error",
            &[
                ("index", (index + 1).to_string()),
                ("component", component.reference.clone()),
                ("error", eda_i18n::text(&error)),
            ],
        )
    };
    let mut candidates = Vec::new();
    for index in 0..config.points {
        check_control()?;
        let fraction = index as f64 / (config.points - 1) as f64;
        let value = if index == 0 {
            config.start
        } else if index == config.points - 1 {
            config.stop
        } else {
            match config.scale {
                SweepScale::Linear => config.start * (1.0 - fraction) + config.stop * fraction,
                SweepScale::Log => {
                    (config.start.ln() * (1.0 - fraction) + config.stop.ln() * fraction).exp()
                }
            }
        };
        let mut edit =
            Engine::new(request.project.clone()).map_err(|e| point_error(index, e.to_string()))?;
        edit.execute(Command::SetComponent {
            id: config.component,
            reference: component.reference.clone(),
            value,
        })
        .map_err(|e| point_error(index, e.to_string()))?;
        candidates.push(value);
    }
    let mut points = Vec::new();
    for (index, value) in candidates.into_iter().enumerate() {
        check_control()?;
        let mut edit =
            Engine::new(request.project.clone()).map_err(|e| point_error(index, e.to_string()))?;
        edit.execute(Command::SetComponent {
            id: config.component,
            reference: component.reference.clone(),
            value,
        })
        .map_err(|e| point_error(index, e.to_string()))?;
        let project = edit.project().clone();
        let remaining = control.timeout.saturating_sub(started.elapsed());
        let point_control = SimulationControl {
            cancelled: control.cancelled.clone(),
            timeout: remaining,
        };
        let result = simulate_with_engine(
            &SimulationRequest {
                project,
                revision: request.revision,
                analysis: Analysis::Dc,
            },
            engine,
            &point_control,
        )
        .map_err(|e| point_error(index, e.to_string()))?;
        points.push(SweepPoint { value, result });
    }
    check_control()?;
    Ok(SweepResult {
        revision: request.revision,
        component: config.component,
        points,
    })
}
