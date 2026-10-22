use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComponentKind {
    Resistor,
    Capacitor,
    Inductor,
    Diode,
    VoltageSource,
    CurrentSource,
    SineVoltage,
    SquareVoltage,
    TriangleVoltage,
    SawVoltage,
    Switch,
    Potentiometer,
    Led,
    Zener,
    Npn,
    Pnp,
    OpAmp,
    LogicInput,
    LogicNot,
    LogicAnd,
    LogicOr,
    LogicXor,
    LogicNand,
    LogicNor,
    Ground,
    Connector,
    Nmos,
    Pmos,
    Comparator,
    Schmitt,
    Timer555,
    DFlipFlop,
    Counter4,
    Counter10,
    Vcvs,
    Vccs,
    Device,
    Imported,
}
impl ComponentKind {
    pub fn all() -> &'static [Self] {
        use ComponentKind::*;
        &[
            Resistor,
            Capacitor,
            Inductor,
            Diode,
            Led,
            Zener,
            Switch,
            Potentiometer,
            VoltageSource,
            CurrentSource,
            SineVoltage,
            SquareVoltage,
            TriangleVoltage,
            SawVoltage,
            Npn,
            Pnp,
            OpAmp,
            LogicInput,
            LogicNot,
            LogicAnd,
            LogicOr,
            LogicXor,
            LogicNand,
            LogicNor,
            Nmos,
            Pmos,
            Comparator,
            Schmitt,
            Timer555,
            DFlipFlop,
            Counter4,
            Counter10,
            Vcvs,
            Vccs,
            Ground,
            Connector,
        ]
    }
    pub fn label(self) -> &'static str {
        use ComponentKind::*;
        match self {
            Resistor => "Resistor",
            Capacitor => "Capacitor",
            Inductor => "Indutor",
            Diode => "Diodo genérico",
            Led => "LED genérico",
            Zener => "Zener genérico",
            Switch => "Chave SPST",
            Potentiometer => "Potenciômetro",
            VoltageSource => "Fonte de tensão DC",
            CurrentSource => "Fonte de corrente DC",
            SineVoltage => "Fonte senoidal",
            SquareVoltage => "Fonte quadrada / pulso",
            TriangleVoltage => "Fonte triangular",
            SawVoltage => "Fonte dente de serra",
            Npn => "Transistor NPN genérico",
            Pnp => "Transistor PNP genérico",
            OpAmp => "Amplificador operacional genérico",
            LogicInput => "Entrada lógica",
            LogicNot => "Porta NOT",
            LogicAnd => "Porta AND",
            LogicOr => "Porta OR",
            LogicXor => "Porta XOR",
            LogicNand => "Porta NAND",
            LogicNor => "Porta NOR",
            Ground => "Terra (GND)",
            Connector => "Conector",
            Nmos => "MOSFET canal N genérico",
            Pmos => "MOSFET canal P genérico",
            Comparator => "Comparador com histerese",
            Schmitt => "Schmitt inversor alimentado",
            Timer555 => "Temporizador 555 genérico",
            DFlipFlop => "Flip-flop D com preset/clear",
            Counter4 => "Contador binário de 4 bits",
            Counter10 => "Contador de década decodificado",
            Vcvs => "Fonte de tensão controlada",
            Vccs => "Fonte de corrente controlada",
            Imported => "Componente importado",
            Device => "Dispositivo de biblioteca",
        }
    }
    pub fn prefix(self) -> &'static str {
        use ComponentKind::*;
        match self {
            Resistor => "R",
            Capacitor => "C",
            Inductor => "L",
            Diode | Led | Zener => "D",
            VoltageSource | SineVoltage | SquareVoltage | TriangleVoltage | SawVoltage
            | LogicInput => "V",
            CurrentSource => "I",
            Switch => "SW",
            Potentiometer => "RV",
            Npn | Pnp => "Q",
            Nmos | Pmos => "M",
            Vcvs => "E",
            Vccs => "G",
            Ground => "GND",
            Connector => "J",
            _ => "U",
        }
    }
    pub fn unit(self) -> &'static str {
        use ComponentKind::*;
        match self {
            Resistor | Potentiometer => "Ω",
            Capacitor => "F",
            Inductor => "H",
            Diode | Led | CurrentSource => "A",
            VoltageSource | SineVoltage | SquareVoltage | TriangleVoltage | SawVoltage | Zener
            | LogicInput | LogicNot | LogicAnd | LogicOr | LogicXor | LogicNand | LogicNor => "V",
            Npn | Pnp => "β",
            Nmos | Pmos => "A/V²",
            Vccs => "A/V",
            Vcvs => "V/V",
            _ => "",
        }
    }
    pub fn default_value(self) -> f64 {
        use ComponentKind::*;
        match self {
            Resistor => 1000.0,
            Potentiometer => 10000.0,
            Capacitor => 1e-6,
            Inductor => 1e-3,
            Diode => 1e-12,
            Led => 1e-30,
            Zener => 5.1,
            Npn | Pnp => 100.0,
            CurrentSource | Vccs => 0.001,
            Nmos | Pmos => 0.02,
            Vcvs => 10.0,
            VoltageSource => 10.0,
            SineVoltage | SquareVoltage | TriangleVoltage | SawVoltage | LogicInput | LogicNot
            | LogicAnd | LogicOr | LogicXor | LogicNand | LogicNor => 5.0,
            _ => 0.0,
        }
    }
    pub fn is_waveform(self) -> bool {
        matches!(
            self,
            Self::SineVoltage | Self::SquareVoltage | Self::TriangleVoltage | Self::SawVoltage
        )
    }
    pub fn is_voltage_source(self) -> bool {
        self.is_waveform() || matches!(self, Self::VoltageSource | Self::LogicInput)
    }
    pub fn is_gate(self) -> bool {
        matches!(
            self,
            Self::LogicNot
                | Self::LogicAnd
                | Self::LogicOr
                | Self::LogicXor
                | Self::LogicNand
                | Self::LogicNor
        )
    }
    pub fn is_nonlinear(self) -> bool {
        self.is_gate()
            || self.requires_mixed()
            || matches!(
                self,
                Self::Diode
                    | Self::Led
                    | Self::Zener
                    | Self::Npn
                    | Self::Pnp
                    | Self::Nmos
                    | Self::Pmos
                    | Self::OpAmp
            )
    }
    pub fn positive_value(self) -> bool {
        matches!(
            self,
            Self::Resistor
                | Self::Potentiometer
                | Self::Capacitor
                | Self::Inductor
                | Self::Diode
                | Self::Led
                | Self::Zener
                | Self::Npn
                | Self::Pnp
                | Self::Nmos
                | Self::Pmos
        ) || self.is_gate()
    }
    pub fn requires_mixed(self) -> bool {
        matches!(
            self,
            Self::Comparator
                | Self::Schmitt
                | Self::Timer555
                | Self::DFlipFlop
                | Self::Counter4
                | Self::Counter10
        )
    }
    pub fn is_mosfet(self) -> bool {
        matches!(self, Self::Nmos | Self::Pmos)
    }
    pub fn supply_pins(self) -> Option<(u8, u8)> {
        match self {
            Self::Comparator => Some((3, 4)),
            Self::Schmitt => Some((2, 3)),
            Self::Timer555 => Some((7, 0)),
            Self::DFlipFlop => Some((6, 7)),
            Self::Counter4 | Self::Counter10 => Some((3, 4)),
            _ => None,
        }
    }
    pub fn pin_count(self) -> u8 {
        use ComponentKind::*;
        match self {
            Comparator => 5,
            Schmitt | Vcvs | Vccs => 4,
            Timer555 | DFlipFlop => 8,
            Counter4 => 9,
            Counter10 => 15,
            Nmos | Pmos => 3,
            Imported | Device => 0,
            Ground | Connector => 1,
            Potentiometer | Npn | Pnp | OpAmp | LogicAnd | LogicOr | LogicXor | LogicNand
            | LogicNor => 3,
            _ => 2,
        }
    }
    pub fn pin_offset(self, pin: u8) -> (i32, i32) {
        use ComponentKind::*;
        match self {
            Comparator => [(-4, -2), (-4, 2), (4, 0), (0, -5), (0, 5)][pin as usize],
            Schmitt => [(-4, 0), (4, 0), (0, -4), (0, 4)][pin as usize],
            Vcvs | Vccs => [(-4, -2), (-4, 2), (4, -2), (4, 2)][pin as usize],
            Timer555 | DFlipFlop => {
                if pin < 4 {
                    (-6, -3 + 2 * i32::from(pin))
                } else {
                    (6, 11 - 2 * i32::from(pin))
                }
            }
            Counter4 | Counter10 => {
                if pin < 5 {
                    (-6, -4 + 2 * i32::from(pin))
                } else {
                    (
                        6,
                        if self == Counter4 {
                            -3 + 2 * (i32::from(pin) - 5)
                        } else {
                            -9 + 2 * (i32::from(pin) - 5)
                        },
                    )
                }
            }
            Nmos | Pmos => [(-3, 0), (3, -2), (3, 2)][pin as usize],
            Ground | Connector => (0, 0),
            Potentiometer => [(-3, 0), (3, 0), (0, 3)][pin as usize],
            Npn | Pnp => [(-3, 0), (3, -2), (3, 2)][pin as usize],
            OpAmp | LogicAnd | LogicOr | LogicXor | LogicNand | LogicNor => {
                [(-3, -2), (-3, 2), (3, 0)][pin as usize]
            }
            _ => {
                if pin == 0 {
                    (-3, 0)
                } else {
                    (3, 0)
                }
            }
        }
    }
    pub fn pin_name(self, pin: u8) -> &'static str {
        use ComponentKind::*;
        match self {
            Nmos | Pmos => ["Gate", "Dreno", "Fonte"][pin as usize],
            Comparator => ["IN+", "IN-", "OUT", "VCC", "GND"][pin as usize],
            Schmitt => ["IN", "OUT", "VCC", "GND"][pin as usize],
            Vcvs | Vccs => ["CTRL+", "CTRL-", "OUT+", "OUT-"][pin as usize],
            Timer555 => ["GND", "TRIG", "OUT", "RSTn", "CTRL", "THR", "DIS", "VCC"][pin as usize],
            DFlipFlop => ["D", "CLK", "PREn", "CLRn", "Q", "Qn", "VCC", "GND"][pin as usize],
            Counter4 | Counter10 => [
                "CLK", "RST", "EN", "VCC", "GND", "Q0", "Q1", "Q2", "Q3", "Q4", "Q5", "Q6", "Q7",
                "Q8", "Q9",
            ][pin as usize],
            Ground => "GND",
            Connector => "Terminal",
            Diode | Led | Zener => {
                if pin == 0 {
                    "Ânodo"
                } else {
                    "Cátodo"
                }
            }
            Potentiometer => ["A", "B", "Cursor"][pin as usize],
            Npn | Pnp => ["Base", "Coletor", "Emissor"][pin as usize],
            OpAmp => ["Entrada +", "Entrada −", "Saída"][pin as usize],
            LogicAnd | LogicOr | LogicXor | LogicNand | LogicNor => {
                ["Entrada A", "Entrada B", "Saída"][pin as usize]
            }
            LogicNot => ["Entrada", "Saída"][pin as usize],
            _ => {
                if pin == 0 {
                    "1"
                } else {
                    "2"
                }
            }
        }
    }
}

/// Parâmetros explícitos dos modelos genéricos; as unidades são SI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ModelParameters {
    pub frequency_hz: f64,
    pub offset_v: f64,
    pub phase_deg: f64,
    pub duty_cycle: f64,
    pub closed: bool,
    pub wiper: f64,
    pub gain: f64,
    pub rail_low: f64,
    pub rail_high: f64,
    pub threshold_v: f64,
    pub on_resistance: f64,
    pub off_resistance: f64,
    pub output_resistance: f64,
    pub saturation_current: f64,
    pub ideality: f64,
    pub mos_threshold_v: f64,
    pub channel_modulation: f64,
    pub hysteresis_v: f64,
    pub propagation_delay_s: f64,
    pub initial_state: u8,
}
impl Default for ModelParameters {
    fn default() -> Self {
        Self {
            frequency_hz: 1000.0,
            offset_v: 0.0,
            phase_deg: 0.0,
            duty_cycle: 0.5,
            closed: false,
            wiper: 0.5,
            gain: 100000.0,
            rail_low: -15.0,
            rail_high: 15.0,
            threshold_v: 2.5,
            on_resistance: 0.01,
            off_resistance: 1e9,
            output_resistance: 100.0,
            saturation_current: 1e-14,
            ideality: 1.0,
            mos_threshold_v: 2.0,
            channel_modulation: 0.01,
            hysteresis_v: 0.05,
            propagation_delay_s: 1e-7,
            initial_state: 0,
        }
    }
}
impl ModelParameters {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
    pub fn is_valid(&self) -> bool {
        (0.0..=1000.0).contains(&self.mos_threshold_v)
            && (0.0..=10.0).contains(&self.channel_modulation)
            && (0.0..=1000.0).contains(&self.hysteresis_v)
            && (1e-12..=1.0).contains(&self.propagation_delay_s)
            && self.initial_state < 16
            && (1e-6..=1e9).contains(&self.frequency_hz)
            && self.offset_v.is_finite()
            && self.offset_v.abs() <= 1e6
            && self.phase_deg.is_finite()
            && self.phase_deg.abs() <= 360000.0
            && (0.0001..=0.9999).contains(&self.duty_cycle)
            && (0.0..=1.0).contains(&self.wiper)
            && (1.0..=1e6).contains(&self.gain)
            && self.rail_low.is_finite()
            && self.rail_high.is_finite()
            && self.rail_low < self.rail_high
            && self.rail_low.abs() <= 1e6
            && self.rail_high.abs() <= 1e6
            && self.threshold_v.is_finite()
            && self.threshold_v.abs() <= 1e6
            && (1e-6..=1e6).contains(&self.on_resistance)
            && (1e6..=1e12).contains(&self.off_resistance)
            && (0.001..=1e6).contains(&self.output_resistance)
            && (1e-30..=1e-6).contains(&self.saturation_current)
            && (0.5..=4.0).contains(&self.ideality)
    }
}
