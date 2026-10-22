//! Solver MNA original para circuitos pequenos, com grandezas em SI.
use eda_core::{ComponentId, ComponentKind, PinRef, Project};
use eda_netlist::{NetId, Netlist, build_netlist};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use thiserror::Error;

const MAX_UNKNOWNS: usize = 64;
const MAX_STEPS: usize = 20_000;
const MAX_WORK: usize = 150_000_000;

const VT: f64 = 0.02585;
mod mixed;
mod models;
mod spice;
mod stepper;

/// Motores próprios que compartilham a topologia e os modelos elétricos.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationEngine {
    #[default]
    Stable,
    Dynamic,
    Mixed,
}
impl SimulationEngine {
    pub fn all() -> &'static [Self] {
        &[Self::Stable, Self::Dynamic, Self::Mixed]
    }
    pub fn id(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Dynamic => "dynamic",
            Self::Mixed => "mixed",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Stable => "Estável",
            Self::Dynamic => "Dinâmico",
            Self::Mixed => "Misto",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Mixed => {
                "Analógico com estados digitais, eventos e atrasos de propagação; integração backward Euler."
            }
            Self::Stable => "Backward Euler: amortecimento numérico, útil para chaveamentos.",
            Self::Dynamic => {
                "Trapezoidal: preserva melhor oscilações; pode oscilar após descontinuidades."
            }
        }
    }
    pub fn parse(id: &str) -> Result<Self, SimulationError> {
        match id {
            "stable" => Ok(Self::Stable),
            "dynamic" => Ok(Self::Dynamic),
            "mixed" => Ok(Self::Mixed),
            _ => Err(SimulationError::InvalidValue(
                "motor deve ser stable, dynamic ou mixed".into(),
            )),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatingPoint {
    pub component: ComponentId,
    pub voltage: f64,
    pub current: f64,
    pub power: f64,
    pub pin_voltages: Vec<f64>,
    #[serde(default)]
    pub pin_currents: Vec<f64>,
    #[serde(default)]
    pub digital_state: Option<u16>,
}
#[derive(Debug, Clone)]
struct History {
    x: Vec<f64>,
    capacitor_currents: BTreeMap<ComponentId, f64>,
    digital: mixed::DigitalState,
    steps: usize,
}
impl History {
    fn zero(size: usize) -> Self {
        Self {
            x: vec![0.0; size],
            capacitor_currents: BTreeMap::new(),
            digital: Default::default(),
            steps: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Analysis {
    Dc,
    Transient { step: f64, stop: f64 },
}
#[derive(Debug, Clone)]
pub struct SimulationRequest {
    pub project: Project,
    pub revision: u64,
    pub analysis: Analysis,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trace {
    pub name: String,
    pub unit: String,
    pub samples: Vec<[f64; 2]>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    pub revision: u64,
    pub backend: String,
    pub analysis: Analysis,
    pub traces: Vec<Trace>,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub operating_points: Vec<OperatingPoint>,
}
/// Controle compartilhado: clones cancelam a mesma execução. O prazo começa em cada chamada.
#[derive(Debug, Clone)]
pub struct SimulationControl {
    cancelled: Arc<AtomicBool>,
    timeout: Duration,
}
impl Default for SimulationControl {
    fn default() -> Self {
        Self::new(Duration::from_secs(10))
    }
}
impl SimulationControl {
    pub fn new(timeout: Duration) -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            timeout,
        }
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}
struct Execution<'a> {
    control: &'a SimulationControl,
    started: Instant,
    work: std::cell::Cell<usize>,
}
impl Execution<'_> {
    fn charge(&self, cost: usize) -> Result<(), SimulationError> {
        let work = self.work.get().saturating_add(cost);
        self.work.set(work);
        if work > MAX_WORK {
            return Err(SimulationError::Limit(
                "orçamento de trabalho da execução excedido".into(),
            ));
        }
        Ok(())
    }

    fn check(&self) -> Result<(), SimulationError> {
        if self.control.cancelled.load(Ordering::Relaxed) {
            return Err(SimulationError::Cancelled);
        }
        if self.started.elapsed() >= self.control.timeout {
            return Err(SimulationError::Timeout);
        }
        Ok(())
    }
}
/// Interface independente da GUI para substituir o backend de simulação.
pub trait SimulationBackend: Send + Sync {
    fn run(
        &self,
        request: &SimulationRequest,
        control: &SimulationControl,
    ) -> Result<SimulationResult, SimulationError>;
}
#[derive(Debug, Default, Clone, Copy)]
pub struct MnaBackend;
impl SimulationBackend for MnaBackend {
    fn run(
        &self,
        request: &SimulationRequest,
        control: &SimulationControl,
    ) -> Result<SimulationResult, SimulationError> {
        simulate_mna(request, control, SimulationEngine::Stable)
    }
}
impl SimulationBackend for SimulationEngine {
    fn run(
        &self,
        request: &SimulationRequest,
        control: &SimulationControl,
    ) -> Result<SimulationResult, SimulationError> {
        simulate_mna(request, control, *self)
    }
}
pub fn simulate_with_engine(
    request: &SimulationRequest,
    engine: SimulationEngine,
    control: &SimulationControl,
) -> Result<SimulationResult, SimulationError> {
    engine.run(request, control)
}
#[derive(Debug, Error)]
pub enum SimulationError {
    #[error("Simulação cancelada pelo usuário.")]
    Cancelled,
    #[error("A simulação excedeu o tempo máximo permitido.")]
    Timeout,
    #[error("Projeto inválido: {0}")]
    InvalidProject(String),
    #[error("A simulação exige uma referência GND conectada.")]
    MissingGround,
    #[error(
        "Circuito singular: verifique redes flutuantes, fontes ideais em conflito e malhas de indutores."
    )]
    Singular,
    #[error("Limite da simulação excedido: {0}")]
    Limit(String),
    #[error("Parâmetro inválido: {0}")]
    InvalidValue(String),
    #[error("O circuito não convergiu em 100 iterações; reveja os modelos e a topologia.")]
    NonConvergence,
    #[error(
        "Modelo de simulação não suportado: {0}. O componente foi preservado para edição e inspeção."
    )]
    Unsupported(String),
}
struct Circuit<'a> {
    project: &'a Project,
    netlist: Netlist,
    nodes: BTreeMap<NetId, usize>,
    branches: BTreeMap<ComponentId, usize>,
    size: usize,
}
impl<'a> Circuit<'a> {
    fn new(project: &'a Project) -> Result<Self, SimulationError> {
        project
            .validate()
            .map_err(|e| SimulationError::InvalidProject(e.to_string()))?;
        if project
            .import_info
            .as_ref()
            .is_some_and(|info| !info.electrical_complete)
        {
            return Err(SimulationError::Unsupported(
                "conectividade incompleta do projeto importado".into(),
            ));
        }
        if let Some(component) = project
            .components
            .values()
            .find(|c| matches!(c.kind, ComponentKind::Imported | ComponentKind::Device))
        {
            return Err(SimulationError::Unsupported(component.reference.clone()));
        }
        let netlist = build_netlist(project);
        if project
            .components
            .values()
            .filter(|c| !matches!(c.kind, ComponentKind::Ground | ComponentKind::Connector))
            .count()
            > 256
        {
            return Err(SimulationError::Limit(
                "máximo de 256 componentes com modelo elétrico".into(),
            ));
        }
        if !netlist.is_grounded() {
            return Err(SimulationError::MissingGround);
        }
        let nodes: BTreeMap<_, _> = netlist
            .nets
            .iter()
            .filter(|n| n.id != NetId(0))
            .enumerate()
            .map(|(i, n)| (n.id, i))
            .collect();
        let mut branches = BTreeMap::new();
        for c in project.components.values() {
            if !c.value.is_finite()
                || c.value.abs() > 1e12
                || (c.kind.positive_value() && c.value <= 0.0)
            {
                return Err(SimulationError::InvalidValue(format!(
                    "{}: valor fora do intervalo permitido",
                    c.reference
                )));
            }
            if c.kind.is_voltage_source()
                || matches!(c.kind, ComponentKind::Inductor | ComponentKind::Vcvs)
            {
                branches.insert(c.id, nodes.len() + branches.len());
            }
        }
        let size = nodes.len() + branches.len();
        if size == 0 {
            return Err(SimulationError::InvalidValue(
                "circuito sem elementos elétricos".into(),
            ));
        }
        if size > MAX_UNKNOWNS {
            return Err(SimulationError::Limit(format!(
                "máximo de {MAX_UNKNOWNS} incógnitas MNA"
            )));
        }
        Ok(Self {
            project,
            netlist,
            nodes,
            branches,
            size,
        })
    }
    fn pin(&self, id: ComponentId, pin: u8) -> Result<Option<usize>, SimulationError> {
        let net = self
            .netlist
            .net_for_pin(PinRef {
                component: id,
                pin: u16::from(pin),
            })
            .ok_or_else(|| SimulationError::InvalidProject("pino sem rede".into()))?;
        if net == NetId(0) {
            Ok(None)
        } else {
            self.nodes
                .get(&net)
                .copied()
                .map(Some)
                .ok_or_else(|| SimulationError::InvalidProject("rede ausente".into()))
        }
    }
    fn solve(
        &self,
        history: &History,
        step: Option<f64>,
        time: f64,
        engine: SimulationEngine,
        execution: &Execution<'_>,
    ) -> Result<Vec<f64>, SimulationError> {
        let previous = &history.x;
        let mut guess = previous.clone();
        let nonlinear = self
            .project
            .components
            .values()
            .any(|c| c.kind.is_nonlinear());
        let trapezoidal = engine == SimulationEngine::Dynamic && history.steps > 0;
        for _ in 0..100 {
            execution.check()?;
            execution.charge(
                self.size
                    .pow(3)
                    .saturating_add(self.project.components.len()),
            )?;
            let mut a = vec![vec![0.0; self.size]; self.size];
            let mut b = vec![0.0; self.size];
            for c in self.project.components.values() {
                use ComponentKind::*;
                if matches!(c.kind, Ground | Connector) {
                    continue;
                }
                let pins: Vec<_> = (0..c.kind.pin_count())
                    .map(|i| self.pin(c.id, i))
                    .collect::<Result<_, _>>()?;
                let (p, n) = (pins[0], pins[1]);
                match c.kind {
                    Resistor => stamp_g(&mut a, p, n, 1.0 / c.value),
                    Switch => stamp_g(
                        &mut a,
                        p,
                        n,
                        1.0 / if c.parameters.closed {
                            c.parameters.on_resistance
                        } else {
                            c.parameters.off_resistance
                        },
                    ),
                    Potentiometer => {
                        let (r1, r2) = models::pot_resistances(c);
                        stamp_g(&mut a, p, pins[2], 1.0 / r1);
                        stamp_g(&mut a, pins[2], n, 1.0 / r2);
                    }
                    CurrentSource => stamp_i(&mut b, p, n, c.value),
                    Capacitor => {
                        if let Some(dt) = step {
                            let g = if trapezoidal { 2.0 } else { 1.0 } * c.value / dt;
                            stamp_g(&mut a, p, n, g);
                            let old_current = if trapezoidal {
                                *history.capacitor_currents.get(&c.id).unwrap_or(&0.0)
                            } else {
                                0.0
                            };
                            stamp_i(&mut b, p, n, -g * voltage(previous, p, n) - old_current);
                        }
                    }
                    Inductor | VoltageSource | SineVoltage | SquareVoltage | TriangleVoltage
                    | SawVoltage | LogicInput => {
                        let k = *self.branches.get(&c.id).ok_or_else(|| {
                            SimulationError::InvalidProject("ramo MNA ausente".into())
                        })?;
                        if let Some(i) = p {
                            a[i][k] += 1.0;
                            a[k][i] += 1.0;
                        }
                        if let Some(i) = n {
                            a[i][k] -= 1.0;
                            a[k][i] -= 1.0;
                        }
                        if c.kind.is_voltage_source() {
                            b[k] = models::source_value(
                                c,
                                if engine == SimulationEngine::Mixed && c.kind == SquareVoltage {
                                    time - step.unwrap_or(0.0) * 0.5
                                } else {
                                    time
                                },
                                step.is_none(),
                            );
                        } else if let Some(dt) = step {
                            let r = if trapezoidal { 2.0 } else { 1.0 } * c.value / dt;
                            a[k][k] -= r;
                            b[k] = -r * previous[k]
                                - if trapezoidal {
                                    voltage(previous, p, n)
                                } else {
                                    0.0
                                };
                        }
                    }
                    Diode | Led | Zener => {
                        let v = voltage(&guess, p, n);
                        let (current, g) = models::diode_iv(c, v);
                        stamp_g(&mut a, p, n, g);
                        stamp_i(&mut b, p, n, current - g * v);
                    }
                    Npn | Pnp => models::stamp_bjt(c, &pins, &guess, &mut a, &mut b),
                    Nmos | Pmos => models::stamp_mosfet(c, &pins, &guess, &mut a, &mut b),
                    Vcvs => {
                        let k = self.branches[&c.id];
                        for (pin, sign) in [(pins[2], 1.0), (pins[3], -1.0)] {
                            if let Some(i) = pin {
                                a[i][k] += sign;
                                a[k][i] += sign;
                            }
                        }
                        if let Some(i) = pins[0] {
                            a[k][i] -= c.value;
                        }
                        if let Some(i) = pins[1] {
                            a[k][i] += c.value;
                        }
                    }
                    Vccs => {
                        for (out, os) in [(pins[2], 1.0), (pins[3], -1.0)] {
                            for (input, is) in [(pins[0], 1.0), (pins[1], -1.0)] {
                                if let (Some(o), Some(i)) = (out, input) {
                                    a[o][i] += os * is * c.value;
                                }
                            }
                        }
                    }
                    Comparator | Schmitt | Timer555 | DFlipFlop | Counter4 | Counter10 => {
                        let v: Vec<_> = pins.iter().map(|p| p.map_or(0.0, |i| guess[i])).collect();
                        for (p, n, g) in mixed::chip_branches(c, &v, history.digital.output(c)) {
                            stamp_g(&mut a, pins[p], pins[n], g);
                        }
                    }
                    kind if kind.is_gate() && engine == SimulationEngine::Mixed => {
                        let out = pins[pins.len() - 1];
                        let g = 1.0 / c.parameters.output_resistance;
                        for input in &pins[..pins.len() - 1] {
                            stamp_g(&mut a, *input, None, 1e-12);
                        }
                        stamp_g(&mut a, out, None, g);
                        if let Some(i) = out {
                            b[i] += g * if history.digital.output(c) != 0 {
                                c.value
                            } else {
                                0.0
                            };
                        }
                    }
                    OpAmp | LogicNot | LogicAnd | LogicOr | LogicXor | LogicNand | LogicNor => {
                        models::stamp_output(c, &pins, &guess, &mut a, &mut b)
                    }
                    _ => {}
                }
            }
            let next = solve_linear(a, b)?;
            if !nonlinear {
                return Ok(next);
            }
            let mut factor: f64 = 1.0;
            for c in self.project.components.values() {
                let pairs: &[(u8, u8)] = match c.kind {
                    ComponentKind::Diode | ComponentKind::Led | ComponentKind::Zener => &[(0, 1)],
                    ComponentKind::Npn | ComponentKind::Pnp => &[(0, 1), (0, 2)],
                    ComponentKind::Nmos | ComponentKind::Pmos => &[(1, 2)],
                    _ => &[],
                };
                for &(p, n) in pairs {
                    let (p, n) = (self.pin(c.id, p)?, self.pin(c.id, n)?);
                    let delta = (voltage(&next, p, n) - voltage(&guess, p, n)).abs();
                    if delta > 0.15 {
                        factor = factor.min(0.15 / delta);
                    }
                }
            }
            let mut converged = true;
            for i in 0..self.size {
                let delta = factor * (next[i] - guess[i]);
                if delta.abs() > 1e-10 + 1e-8 * next[i].abs() {
                    converged = false;
                }
                guess[i] += delta;
            }
            if converged {
                return Ok(guess);
            }
        }
        Err(SimulationError::NonConvergence)
    }
    fn operating_points(
        &self,
        x: &[f64],
        old: &History,
        dt: Option<f64>,
        engine: SimulationEngine,
    ) -> Result<Vec<OperatingPoint>, SimulationError> {
        let mut points = Vec::new();
        let trap = engine == SimulationEngine::Dynamic && old.steps > 0;
        for c in self.project.components.values() {
            use ComponentKind::*;
            if matches!(c.kind, Ground | Connector) {
                continue;
            }
            let pins: Vec<_> = (0..c.kind.pin_count())
                .map(|i| self.pin(c.id, i))
                .collect::<Result<_, _>>()?;
            let v: Vec<_> = pins.iter().map(|p| p.map_or(0.0, |i| x[i])).collect();
            if c.kind.requires_mixed() || c.kind.is_mosfet() || matches!(c.kind, Vcvs | Vccs) {
                let state = old.digital.output(c);
                let currents = if c.kind.requires_mixed() {
                    mixed::chip_currents(c, &v, state)
                } else if c.kind.is_mosfet() {
                    models::mosfet(c, [v[0], v[1], v[2]]).0.to_vec()
                } else {
                    let current = if c.kind == Vcvs {
                        x[self.branches[&c.id]]
                    } else {
                        c.value * (v[0] - v[1])
                    };
                    vec![0.0, 0.0, current, -current]
                };
                let (u, i) = if let Some((s, g)) = c.kind.supply_pins() {
                    (v[s as usize] - v[g as usize], currents[s as usize])
                } else if c.kind.is_mosfet() {
                    (v[1] - v[2], currents[1])
                } else {
                    (v[2] - v[3], currents[2])
                };
                let power: f64 = v.iter().zip(&currents).map(|(v, i)| v * i).sum();
                if !power.is_finite() || currents.iter().any(|i| !i.is_finite()) {
                    return Err(SimulationError::InvalidValue(
                        "medida não finita em modelo complexo".into(),
                    ));
                }
                points.push(OperatingPoint {
                    component: c.id,
                    voltage: u,
                    current: i,
                    power,
                    pin_voltages: v,
                    pin_currents: currents,
                    digital_state: c.kind.requires_mixed().then_some(state),
                });
                continue;
            }
            let u = v[0] - v[1];
            let current = match c.kind {
                Resistor => u / c.value,
                Switch => {
                    u / if c.parameters.closed {
                        c.parameters.on_resistance
                    } else {
                        c.parameters.off_resistance
                    }
                }
                Potentiometer => (v[0] - v[2]) / models::pot_resistances(c).0,
                Capacitor => dt.map_or(0.0, |dt| {
                    let g = if trap { 2.0 } else { 1.0 } * c.value / dt;
                    g * (u - voltage(&old.x, pins[0], pins[1]))
                        - if trap {
                            *old.capacitor_currents.get(&c.id).unwrap_or(&0.0)
                        } else {
                            0.0
                        }
                }),
                CurrentSource => c.value,
                Diode | Led | Zener => models::diode_iv(c, u).0,
                Npn | Pnp => models::transistor(c, [v[0], v[1], v[2]]).0[1],
                OpAmp | LogicNot | LogicAnd | LogicOr | LogicXor | LogicNand | LogicNor => {
                    (v[v.len() - 1]
                        - if engine == SimulationEngine::Mixed && c.kind.is_gate() {
                            if old.digital.output(c) != 0 {
                                c.value
                            } else {
                                0.0
                            }
                        } else {
                            models::output_target(c, &v)
                        })
                        / c.parameters.output_resistance
                }
                _ => self.branches.get(&c.id).map_or(0.0, |k| x[*k]),
            };
            let (measured_voltage, power) = match c.kind {
                Potentiometer => {
                    let (r1, r2) = models::pot_resistances(c);
                    (u, (v[0] - v[2]).powi(2) / r1 + (v[1] - v[2]).powi(2) / r2)
                }
                Npn | Pnp => {
                    let currents = models::transistor(c, [v[0], v[1], v[2]]).0;
                    (
                        v[1] - v[2],
                        v.iter().zip(currents).map(|(v, i)| v * i).sum(),
                    )
                }
                OpAmp | LogicNot | LogicAnd | LogicOr | LogicXor | LogicNand | LogicNor => {
                    (v[v.len() - 1], v[v.len() - 1] * current)
                }
                _ => (u, u * current),
            };
            if !current.is_finite() || !power.is_finite() {
                return Err(SimulationError::InvalidValue(
                    "corrente ou potência não finita".into(),
                ));
            }
            points.push(OperatingPoint {
                component: c.id,
                voltage: measured_voltage,
                current,
                power,
                pin_voltages: v,
                pin_currents: Vec::new(),
                digital_state: (engine == SimulationEngine::Mixed && c.kind.is_gate())
                    .then_some(old.digital.output(c)),
            });
        }
        Ok(points)
    }
}

fn voltage(x: &[f64], p: Option<usize>, n: Option<usize>) -> f64 {
    p.map_or(0.0, |i| x[i]) - n.map_or(0.0, |i| x[i])
}
fn stamp_g(a: &mut [Vec<f64>], p: Option<usize>, n: Option<usize>, g: f64) {
    if let Some(i) = p {
        a[i][i] += g;
    }
    if let Some(j) = n {
        a[j][j] += g;
    }
    if let (Some(i), Some(j)) = (p, n) {
        a[i][j] -= g;
        a[j][i] -= g;
    }
}
fn stamp_i(b: &mut [f64], p: Option<usize>, n: Option<usize>, current: f64) {
    if let Some(i) = p {
        b[i] -= current;
    }
    if let Some(i) = n {
        b[i] += current;
    }
}
fn solve_linear(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Result<Vec<f64>, SimulationError> {
    let size = b.len();
    for i in 0..size {
        let scale = a[i].iter().fold(0.0_f64, |m, v| m.max(v.abs()));
        if scale == 0.0 || !scale.is_finite() {
            return Err(SimulationError::Singular);
        }
        for v in &mut a[i] {
            *v /= scale;
        }
        b[i] /= scale;
    }
    for k in 0..size {
        let mut pivot = k;
        for i in k + 1..size {
            if a[i][k].abs() > a[pivot][k].abs() {
                pivot = i;
            }
        }
        if a[pivot][k].abs() < 1e-14 {
            return Err(SimulationError::Singular);
        }
        a.swap(k, pivot);
        b.swap(k, pivot);
        for i in k + 1..size {
            let (above, remaining) = a.split_at_mut(i);
            let row = &mut remaining[0];
            let pivot_row = &above[k];
            let ratio = row[k] / pivot_row[k];
            for (current, pivot_value) in row[k + 1..].iter_mut().zip(&pivot_row[k + 1..]) {
                *current -= ratio * pivot_value;
            }
            b[i] -= ratio * b[k];
        }
    }
    let mut x = vec![0.0; size];
    for i in (0..size).rev() {
        let sum: f64 = (i + 1..size).map(|j| a[i][j] * x[j]).sum();
        x[i] = (b[i] - sum) / a[i][i];
        if !x[i].is_finite() || x[i].abs() > 1e15 {
            return Err(SimulationError::InvalidValue(
                "resultado numérico fora dos limites".into(),
            ));
        }
    }
    Ok(x)
}
fn steps(analysis: Analysis) -> Result<usize, SimulationError> {
    match analysis {
        Analysis::Dc => Ok(1),
        Analysis::Transient { step, stop } => {
            if !step.is_finite()
                || !stop.is_finite()
                || step < 1e-12
                || stop <= 0.0
                || stop > 1e6
                || step > stop
            {
                return Err(SimulationError::InvalidValue(
                    "transiente exige 1 ps ≤ passo ≤ duração ≤ 10⁶ s".into(),
                ));
            }
            let ratio = stop / step;
            let count = (ratio - 1e-12 * ratio).ceil().max(1.0);
            if count > MAX_STEPS as f64 {
                return Err(SimulationError::Limit(format!(
                    "máximo de {MAX_STEPS} passos"
                )));
            }
            Ok(count as usize)
        }
    }
}
pub fn simulate(request: &SimulationRequest) -> Result<SimulationResult, SimulationError> {
    simulate_controlled(request, &SimulationControl::default())
}
pub fn simulate_controlled(
    request: &SimulationRequest,
    control: &SimulationControl,
) -> Result<SimulationResult, SimulationError> {
    MnaBackend.run(request, control)
}
fn validate_engine(project: &Project, engine: SimulationEngine) -> Result<(), SimulationError> {
    if engine != SimulationEngine::Mixed
        && let Some(c) = project
            .components
            .values()
            .find(|c| c.kind.requires_mixed())
    {
        return Err(SimulationError::Unsupported(format!(
            "{} exige o motor Misto (mixed)",
            c.reference
        )));
    }
    Ok(())
}
fn simulate_mna(
    request: &SimulationRequest,
    control: &SimulationControl,
    engine: SimulationEngine,
) -> Result<SimulationResult, SimulationError> {
    let execution = Execution {
        control,
        started: Instant::now(),
        work: std::cell::Cell::new(0),
    };
    execution.check()?;
    validate_engine(&request.project, engine)?;
    let count = steps(request.analysis)?;
    let circuit = Circuit::new(&request.project)?;
    if engine != SimulationEngine::Mixed {
        check_work(&circuit, count)?;
    }
    let mut result = empty_result(&circuit, request.revision, request.analysis, engine);
    if result.traces.len().saturating_mul(count + 1) > 2_000_000 {
        return Err(SimulationError::Limit(
            "máximo de dois milhões de amostras".into(),
        ));
    }
    let mut state = History::zero(circuit.size);
    let mut previous_time = 0.0;
    if matches!(request.analysis, Analysis::Transient { .. }) {
        record(&circuit, &mut result, 0.0, &state.x, &[]);
    }
    if engine == SimulationEngine::Mixed {
        match request.analysis {
            Analysis::Dc => {
                let (x, points) = stepper::dc(&circuit, &mut state, &execution)?;
                record(&circuit, &mut result, 0.0, &x, &points);
            }
            Analysis::Transient { step, stop } => {
                for i in 1..=count {
                    let target = (i as f64 * step).min(stop);
                    stepper::advance(
                        &circuit,
                        &mut state,
                        &mut previous_time,
                        target,
                        &execution,
                        |time, x, points| {
                            if result
                                .traces
                                .len()
                                .saturating_mul(result.traces[0].samples.len() + 1)
                                > 2_000_000
                            {
                                return Err(SimulationError::Limit(
                                    "máximo de dois milhões de amostras incluindo eventos".into(),
                                ));
                            }
                            record(&circuit, &mut result, time, x, points);
                            Ok(())
                        },
                    )?;
                }
            }
        }
        return Ok(result);
    }
    for i in 1..=count {
        execution.check()?;
        let (time, dt) = match request.analysis {
            Analysis::Dc => (0.0, None),
            Analysis::Transient { step, stop } => {
                let t = (i as f64 * step).min(stop);
                (t, Some(t - previous_time))
            }
        };
        let x = circuit.solve(&state, dt, time, engine, &execution)?;
        let points = circuit.operating_points(&x, &state, dt, engine)?;
        record(&circuit, &mut result, time, &x, &points);
        state.capacitor_currents = points
            .iter()
            .filter(|p| circuit.project.components[&p.component].kind == ComponentKind::Capacitor)
            .map(|p| (p.component, p.current))
            .collect();
        state.x = x;
        state.steps += 1;
        previous_time = time;
    }
    execution.check()?;
    Ok(result)
}
fn check_work(circuit: &Circuit<'_>, count: usize) -> Result<(), SimulationError> {
    let iterations = if circuit
        .project
        .components
        .values()
        .any(|c| c.kind.is_nonlinear())
    {
        100
    } else {
        1
    };
    if count
        .saturating_mul(
            circuit
                .size
                .pow(3)
                .saturating_add(circuit.project.components.len()),
        )
        .saturating_mul(iterations)
        > MAX_WORK
    {
        return Err(SimulationError::Limit(
            "custo numérico estimado acima do limite; reduza nós ou passos".into(),
        ));
    }
    Ok(())
}
fn empty_result(
    circuit: &Circuit<'_>,
    revision: u64,
    analysis: Analysis,
    engine: SimulationEngine,
) -> SimulationResult {
    let mut traces: Vec<_> = circuit
        .netlist
        .nets
        .iter()
        .map(|n| Trace {
            name: format!("V({})", n.name),
            unit: "V".into(),
            samples: Vec::new(),
        })
        .collect();
    for c in circuit
        .project
        .components
        .values()
        .filter(|c| !matches!(c.kind, ComponentKind::Ground | ComponentKind::Connector))
    {
        for (symbol, unit) in [("U", "V"), ("I", "A"), ("P", "W")] {
            traces.push(Trace {
                name: format!("{symbol}({})", c.reference),
                unit: unit.into(),
                samples: Vec::new(),
            });
        }
    }
    let mut warnings = Vec::new();
    if engine == SimulationEngine::Mixed {
        warnings.push("Eventos e bordas de fontes interrompem o passo analógico. Cruzamentos analógicos são refinados; atrasos e alimentação são modelos genéricos, sem setup/hold ou metastabilidade.".into());
    }
    if circuit
        .project
        .components
        .values()
        .any(|c| c.kind.is_nonlinear())
    {
        warnings.push("Modelos genéricos educacionais: sem parâmetros de fabricante, capacitâncias parasitas ou temperatura. O motor Misto usa atrasos genéricos configuráveis.".into());
    }
    if matches!(analysis, Analysis::Transient { .. }) {
        warnings.push(format!("{} C/L iniciam em zero; estados digitais usam os parâmetros do circuito. A primeira amostra precede a energização. Primeiro passo e alterações de parâmetros usam backward Euler.",engine.description()));
    }
    if let Analysis::Transient { step, .. } = analysis
        && circuit
            .project
            .components
            .values()
            .any(|c| c.kind.is_waveform() && step * c.parameters.frequency_hz > 0.05)
    {
        warnings.push("Passo acima de 1/20 do período de uma fonte: reduza o passo para evitar perda de forma de onda.".into());
    }
    SimulationResult {
        revision,
        backend: format!("ProtoCraft {} · Rust MNA", engine.label()),
        analysis,
        traces,
        warnings,
        operating_points: Vec::new(),
    }
}
fn record(
    circuit: &Circuit<'_>,
    result: &mut SimulationResult,
    time: f64,
    x: &[f64],
    points: &[OperatingPoint],
) {
    for (i, n) in circuit.netlist.nets.iter().enumerate() {
        result.traces[i]
            .samples
            .push([time, circuit.nodes.get(&n.id).map_or(0.0, |i| x[*i])]);
    }
    let mut index = circuit.netlist.nets.len();
    for c in circuit
        .project
        .components
        .values()
        .filter(|c| !matches!(c.kind, ComponentKind::Ground | ComponentKind::Connector))
    {
        let point = points.iter().find(|p| p.component == c.id);
        for value in [
            point.map_or(0.0, |p| p.voltage),
            point.map_or(0.0, |p| p.current),
            point.map_or(0.0, |p| p.power),
        ] {
            result.traces[index].samples.push([time, value]);
            index += 1;
        }
    }
    result.operating_points = points.to_vec();
}

/// Sessão contínua com memória de capacitores e indutores entre avanços.
pub struct TransientSession {
    project: Project,
    revision: u64,
    engine: SimulationEngine,
    step: f64,
    time: f64,
    state: History,
    result: SimulationResult,
}
impl TransientSession {
    pub fn new(
        project: Project,
        revision: u64,
        engine: SimulationEngine,
        step: f64,
    ) -> Result<Self, SimulationError> {
        validate_engine(&project, engine)?;
        steps(Analysis::Transient { step, stop: step })?;
        let circuit = Circuit::new(&project)?;
        let state = History::zero(circuit.size);
        let mut result = empty_result(
            &circuit,
            revision,
            Analysis::Transient { step, stop: step },
            engine,
        );
        record(&circuit, &mut result, 0.0, &state.x, &[]);
        Ok(Self {
            project,
            revision,
            engine,
            step,
            time: 0.0,
            state,
            result,
        })
    }
    /// Mudanças de valores preservam estado; mudanças de topologia reiniciam a sessão.
    pub fn update_project(
        &mut self,
        project: Project,
        revision: u64,
    ) -> Result<bool, SimulationError> {
        validate_engine(&project, self.engine)?;
        let circuit = Circuit::new(&project)?;
        let same = self.project.wires == project.wires
            && self.project.junctions == project.junctions
            && self.project.labels == project.labels
            && self.project.components.len() == project.components.len()
            && self.project.components.iter().all(|(id, c)| {
                project
                    .components
                    .get(id)
                    .is_some_and(|other| c.kind == other.kind && c.pins() == other.pins())
            });
        if !same {
            self.state = History::zero(circuit.size);
            self.time = 0.0;
        }
        self.state.steps = 0;
        self.state.digital.parameters_changed();
        self.result = empty_result(
            &circuit,
            revision,
            Analysis::Transient {
                step: self.step,
                stop: self.time.max(self.step),
            },
            self.engine,
        );
        self.project = project;
        self.revision = revision;
        Ok(!same)
    }
    pub fn advance(
        &mut self,
        count: usize,
        control: &SimulationControl,
    ) -> Result<SimulationResult, SimulationError> {
        if count == 0 || count > 1024 {
            return Err(SimulationError::Limit(
                "avanço contínuo deve ter 1 a 1024 passos".into(),
            ));
        }
        let execution = Execution {
            control,
            started: Instant::now(),
            work: std::cell::Cell::new(0),
        };
        execution.check()?;
        let circuit = Circuit::new(&self.project)?;
        if self.engine == SimulationEngine::Mixed {
            for _ in 0..count {
                let target = self.time + self.step;
                if !target.is_finite() || target > 1e6 {
                    return Err(SimulationError::Limit(
                        "tempo contínuo máximo de 10⁶ segundos".into(),
                    ));
                }
                stepper::advance(
                    &circuit,
                    &mut self.state,
                    &mut self.time,
                    target,
                    &execution,
                    |time, x, points| {
                        record(&circuit, &mut self.result, time, x, points);
                        for trace in &mut self.result.traces {
                            let excess = trace.samples.len().saturating_sub(512);
                            if excess > 0 {
                                trace.samples.drain(..excess);
                            }
                        }
                        Ok(())
                    },
                )?;
            }
            self.result.analysis = Analysis::Transient {
                step: self.step,
                stop: self.time,
            };
            return Ok(self.result.clone());
        }
        check_work(&circuit, count)?;
        for _ in 0..count {
            execution.check()?;
            let time = self.time + self.step;
            if !time.is_finite() || time > 1e6 {
                return Err(SimulationError::Limit(
                    "tempo contínuo máximo de 10⁶ segundos".into(),
                ));
            }
            let x = circuit.solve(&self.state, Some(self.step), time, self.engine, &execution)?;
            let points = circuit.operating_points(&x, &self.state, Some(self.step), self.engine)?;
            record(&circuit, &mut self.result, time, &x, &points);
            self.state.capacitor_currents = points
                .iter()
                .filter(|p| self.project.components[&p.component].kind == ComponentKind::Capacitor)
                .map(|p| (p.component, p.current))
                .collect();
            self.state.x = x;
            self.state.steps += 1;
            self.time = time;
        }
        for trace in &mut self.result.traces {
            let excess = trace.samples.len().saturating_sub(512);
            if excess > 0 {
                trace.samples.drain(..excess);
            }
        }
        self.result.analysis = Analysis::Transient {
            step: self.step,
            stop: self.time,
        };
        Ok(self.result.clone())
    }
    pub fn time(&self) -> f64 {
        self.time
    }
}
/// Exportação tipada: um modelo sem equivalente é rejeitado, nunca omitido.
pub fn export_spice(project: &Project, analysis: Analysis) -> Result<String, SimulationError> {
    spice::export(project, analysis)
}
