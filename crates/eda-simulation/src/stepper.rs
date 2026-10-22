//! Avanço analógico com paradas em eventos digitais e bordas de fontes.
use super::*;

pub(super) fn advance<F>(
    circuit: &Circuit<'_>,
    state: &mut History,
    time: &mut f64,
    target: f64,
    execution: &Execution<'_>,
    mut sample: F,
) -> Result<(), SimulationError>
where
    F: FnMut(f64, &[f64], &[OperatingPoint]) -> Result<(), SimulationError>,
{
    let mut internal_steps = 0;
    while *time < target {
        execution.check()?;
        internal_steps += 1;
        if internal_steps > 100_000 {
            return Err(SimulationError::Limit(
                "excesso de eventos/passos internos; reduza a duração ou frequência".into(),
            ));
        }
        state.digital.commit(*time);
        let epsilon = mixed::MIN_TIME.max(time.abs() * f64::EPSILON * 4.0);
        let settling = state.digital.settle || state.steps == 0;
        state.digital.settle = false;
        let edge = mixed::next_clock_edge(circuit.project, *time);
        let mut end = target;
        if settling {
            end = end.min(*time + epsilon);
        }
        if let Some(event) = state.digital.next_event()
            && event > *time
        {
            end = end.min(event);
        }
        if let Some(edge) = edge {
            end = end.min(edge);
        }
        if end <= *time {
            return Err(SimulationError::Limit(
                "precisão temporal insuficiente para o próximo evento".into(),
            ));
        }
        let mut x = circuit.solve(
            state,
            Some(end - *time),
            end,
            SimulationEngine::Mixed,
            execution,
        )?;
        // Refina cruzamentos contínuos sem modificar estado durante as tentativas.
        if !settling
            && end - *time > epsilon * 2.0
            && state.digital.analog_crossing(circuit, &state.x, &x)?
        {
            let mut low = *time;
            let mut high = end;
            for _ in 0..32 {
                if high - low <= epsilon {
                    break;
                }
                let middle = low + (high - low) * 0.5;
                let trial = circuit.solve(
                    state,
                    Some(middle - *time),
                    middle,
                    SimulationEngine::Mixed,
                    execution,
                )?;
                if state.digital.analog_crossing(circuit, &state.x, &trial)? {
                    high = middle;
                    x = trial;
                } else {
                    low = middle;
                }
            }
            end = high;
        }
        let points =
            circuit.operating_points(&x, state, Some(end - *time), SimulationEngine::Mixed)?;
        state.capacitor_currents = points
            .iter()
            .filter(|p| circuit.project.components[&p.component].kind == ComponentKind::Capacitor)
            .map(|p| (p.component, p.current))
            .collect();
        state.x = x;
        state.steps += 1;
        *time = end;
        state.digital.observe(circuit, &state.x, end, false)?;
        state.digital.commit(end);
        if edge.is_some_and(|edge| (edge - end).abs() <= epsilon * 0.1) {
            state.digital.settle = true;
        }
        sample(end, &state.x, &points)?;
    }
    Ok(())
}

pub(super) fn dc(
    circuit: &Circuit<'_>,
    state: &mut History,
    execution: &Execution<'_>,
) -> Result<(Vec<f64>, Vec<OperatingPoint>), SimulationError> {
    for _ in 0..256 {
        execution.check()?;
        let x = circuit.solve(state, None, 0.0, SimulationEngine::Mixed, execution)?;
        state.digital.observe(circuit, &x, 0.0, true)?;
        if let Some(time) = state.digital.next_event() {
            state.digital.commit(time);
            state.x = x;
        } else {
            let points = circuit.operating_points(&x, state, None, SimulationEngine::Mixed)?;
            return Ok((x, points));
        }
    }
    Err(SimulationError::Limit(
        "o estado DC digital não estabilizou; use transiente para osciladores".into(),
    ))
}
