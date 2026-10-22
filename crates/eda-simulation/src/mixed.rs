//! Estados e atrasos determinísticos de modelos mistos próprios.
use super::*;
use eda_core::Component;

pub(super) const MIN_TIME: f64 = 1e-12;
#[derive(Debug, Clone, Default)]
struct Device {
    logical: u16,
    output: u16,
    last_clock: bool,
    pending: Option<(f64, u16)>,
}
#[derive(Debug, Clone, Default)]
pub(super) struct DigitalState {
    devices: BTreeMap<ComponentId, Device>,
    pub(super) settle: bool,
}
impl DigitalState {
    pub(super) fn parameters_changed(&mut self) {
        for device in self.devices.values_mut() {
            device.pending = None;
            device.logical = device.output;
        }
        self.settle = true;
    }
    pub(super) fn output(&self, c: &Component) -> u16 {
        self.devices
            .get(&c.id)
            .map_or(u16::from(c.parameters.initial_state), |s| s.output)
    }
    pub(super) fn next_event(&self) -> Option<f64> {
        self.devices
            .values()
            .filter_map(|d| d.pending.map(|p| p.0))
            .min_by(f64::total_cmp)
    }
    pub(super) fn commit(&mut self, time: f64) -> bool {
        let mut changed = false;
        for d in self.devices.values_mut() {
            if let Some((due, value)) = d.pending
                && due <= time + 1e-15
            {
                changed |= d.output != value;
                d.output = value;
                d.pending = None;
            }
        }
        self.settle |= changed;
        changed
    }
    pub(super) fn observe(
        &mut self,
        circuit: &Circuit<'_>,
        x: &[f64],
        time: f64,
        dc: bool,
    ) -> Result<(), SimulationError> {
        for c in circuit
            .project
            .components
            .values()
            .filter(|c| c.kind.requires_mixed() || c.kind.is_gate())
        {
            let v = pin_values(circuit, c, x)?;
            let state = self.devices.entry(c.id).or_insert_with(|| Device {
                logical: u16::from(c.parameters.initial_state),
                output: u16::from(c.parameters.initial_state),
                ..Default::default()
            });
            let desired = desired(c, &v, state, dc)?;
            state.logical = desired;
            if desired == state.output {
                state.pending = None;
            } else if state.pending.is_none_or(|(_, value)| value != desired) {
                state.pending = Some((time + c.parameters.propagation_delay_s, desired));
            }
            if matches!(c.kind, ComponentKind::DFlipFlop) {
                state.last_clock = high(c, &v, 1);
            }
            if matches!(c.kind, ComponentKind::Counter4 | ComponentKind::Counter10) {
                state.last_clock = high(c, &v, 0);
            }
        }
        Ok(())
    }
    /// Detecta cruzamentos analógicos; a busca temporal refina somente esses modelos.
    pub(super) fn analog_crossing(
        &self,
        circuit: &Circuit<'_>,
        before: &[f64],
        after: &[f64],
    ) -> Result<bool, SimulationError> {
        for c in circuit.project.components.values().filter(|c| {
            matches!(
                c.kind,
                ComponentKind::Timer555 | ComponentKind::Comparator | ComponentKind::Schmitt
            )
        }) {
            let default = Device {
                logical: u16::from(c.parameters.initial_state),
                ..Default::default()
            };
            let state = self.devices.get(&c.id).unwrap_or(&default);
            let a = pin_values(circuit, c, before)?;
            let b = pin_values(circuit, c, after)?;
            if desired(c, &a, state, false)? != desired(c, &b, state, false)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
fn pin_values(
    circuit: &Circuit<'_>,
    c: &Component,
    x: &[f64],
) -> Result<Vec<f64>, SimulationError> {
    (0..c.kind.pin_count())
        .map(|pin| circuit.pin(c.id, pin).map(|p| p.map_or(0.0, |i| x[i])))
        .collect()
}
fn high(c: &Component, v: &[f64], pin: usize) -> bool {
    if let Some((s, g)) = c.kind.supply_pins() {
        v[pin] - v[g as usize] >= 0.5 * (v[s as usize] - v[g as usize])
    } else {
        v[pin] >= c.parameters.threshold_v
    }
}
fn desired(c: &Component, v: &[f64], state: &Device, dc: bool) -> Result<u16, SimulationError> {
    use ComponentKind::*;
    let old = state.logical;
    if let Some((s, g)) = c.kind.supply_pins()
        && v[s as usize] - v[g as usize] <= 1.0
    {
        return Ok(0);
    }
    let value = match c.kind {
        Comparator => {
            let d = v[0] - v[1];
            if d > c.parameters.hysteresis_v * 0.5 {
                1
            } else if d < -c.parameters.hysteresis_v * 0.5 {
                0
            } else {
                old
            }
        }
        Schmitt => {
            let supply = v[2] - v[3];
            let input = v[0] - v[3];
            if input < supply / 3.0 {
                1
            } else if input > supply * 2.0 / 3.0 {
                0
            } else {
                old
            }
        }
        Timer555 => {
            let ctrl = v[4] - v[0];
            if v[3] - v[0] < 0.7 {
                0
            } else if v[1] - v[0] < ctrl * 0.5 {
                1
            } else if v[5] - v[0] > ctrl {
                0
            } else {
                old
            }
        }
        DFlipFlop => {
            let preset = !high(c, v, 2);
            let clear = !high(c, v, 3);
            if preset && clear {
                return Err(SimulationError::InvalidValue(format!(
                    "{}: PREn e CLRn ativos simultaneamente",
                    c.reference
                )));
            }
            if clear {
                0
            } else if preset {
                1
            } else if !dc && high(c, v, 1) && !state.last_clock {
                u16::from(high(c, v, 0))
            } else {
                old
            }
        }
        Counter4 | Counter10 => {
            if high(c, v, 1) {
                0
            } else if !dc && high(c, v, 2) && high(c, v, 0) && !state.last_clock {
                (old + 1) % if c.kind == Counter4 { 16 } else { 10 }
            } else {
                old
            }
        }
        _ => u16::from(models::output_target(c, v) > 0.0),
    };
    Ok(value)
}
/// Rede resistiva dos estágios alimentados; cada ramo contribui à corrente de alimentação.
pub(super) fn chip_branches(c: &Component, v: &[f64], state: u16) -> Vec<(usize, usize, f64)> {
    use ComponentKind::*;
    let Some((s, g)) = c.kind.supply_pins() else {
        return Vec::new();
    };
    let (s, g) = (s as usize, g as usize);
    let powered = v[s] - v[g] > 1.0;
    let mut branches = Vec::new();
    let mut drive = |pin: usize, high: bool| {
        branches.push((
            pin,
            if powered && high { s } else { g },
            1.0 / if powered {
                c.parameters.output_resistance
            } else {
                c.parameters.off_resistance
            },
        ));
    };
    match c.kind {
        Comparator => drive(2, state != 0),
        Schmitt => drive(1, state != 0),
        Timer555 => {
            drive(2, state != 0);
            branches.push((
                6,
                g,
                1.0 / if powered && state == 0 {
                    c.parameters.on_resistance
                } else {
                    c.parameters.off_resistance
                },
            ));
            branches.push((s, 4, 1.0 / 5000.0));
            branches.push((4, g, 1.0 / 10000.0));
        }
        DFlipFlop => {
            drive(4, state != 0);
            drive(5, state == 0);
        }
        Counter4 => {
            for i in 0..4 {
                drive(5 + i, state & (1 << i) != 0);
            }
        }
        Counter10 => {
            for i in 0..10 {
                drive(5 + i, state == i as u16);
            }
        }
        _ => {}
    }
    let inputs: &[usize] = match c.kind {
        Comparator => &[0, 1],
        Schmitt => &[0],
        Timer555 => &[1, 3, 5],
        DFlipFlop => &[0, 1, 2, 3],
        Counter4 | Counter10 => &[0, 1, 2],
        _ => &[],
    };
    for &pin in inputs {
        branches.push((pin, g, 1e-12));
    }
    branches
}
pub(super) fn chip_currents(c: &Component, v: &[f64], state: u16) -> Vec<f64> {
    let mut currents = vec![0.0; v.len()];
    for (p, n, g) in chip_branches(c, v, state) {
        let i = (v[p] - v[n]) * g;
        currents[p] += i;
        currents[n] -= i;
    }
    currents
}
pub(super) fn next_clock_edge(project: &Project, time: f64) -> Option<f64> {
    project
        .components
        .values()
        .filter(|c| c.kind == ComponentKind::SquareVoltage)
        .filter_map(|c| {
            let p = &c.parameters;
            let cycles = time * p.frequency_hz + p.phase_deg / 360.0;
            let base = cycles.floor();
            [base + p.duty_cycle, base + 1.0, base + 1.0 + p.duty_cycle]
                .into_iter()
                .map(|edge| (edge - p.phase_deg / 360.0) / p.frequency_hz)
                .filter(|edge| *edge > time + 1e-15)
                .min_by(f64::total_cmp)
        })
        .min_by(f64::total_cmp)
}
