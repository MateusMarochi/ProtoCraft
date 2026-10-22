//! Exemplos originais com geometria e conexões verificáveis.
use super::*;

fn component(
    p: &mut Project,
    kind: ComponentKind,
    reference: &str,
    value: f64,
    x: i32,
    y: i32,
    rotation: u8,
) {
    let id = ComponentId(p.next_id);
    p.next_id += 1;
    p.components.insert(
        id,
        Component {
            id,
            reference: reference.into(),
            kind,
            value,
            position: Point::new(x, y),
            rotation,
            parameters: Default::default(),
            device: None,
            footprint: None,
            imported: None,
        },
    );
}
fn wire(p: &mut Project, a: (i32, i32), b: (i32, i32)) {
    let id = WireId(p.next_id);
    p.next_id += 1;
    p.wires.insert(
        id,
        Wire {
            id,
            start: Point::new(a.0, a.1),
            end: Point::new(b.0, b.1),
        },
    );
}
fn label(p: &mut Project, name: &str, x: i32, y: i32) {
    let id = LabelId(p.next_id);
    p.next_id += 1;
    p.labels.insert(
        id,
        NetLabel {
            id,
            position: Point::new(x, y),
            name: name.into(),
        },
    );
}
pub fn voltage_divider() -> Project {
    let mut p = Project::empty("Divisor de tensão — 10 V para 5 V");
    component(&mut p, ComponentKind::VoltageSource, "V1", 10.0, -12, 0, 1);
    component(&mut p, ComponentKind::Resistor, "R1", 1000.0, 0, -8, 0);
    component(&mut p, ComponentKind::Resistor, "R2", 1000.0, 10, 0, 1);
    component(&mut p, ComponentKind::Ground, "GND1", 0.0, -12, 8, 0);
    for (a, b) in [
        ((-12, -3), (-12, -8)),
        ((-12, -8), (-3, -8)),
        ((3, -8), (10, -8)),
        ((10, -8), (10, -3)),
        ((10, 3), (10, 8)),
        ((10, 8), (-12, 8)),
        ((-12, 3), (-12, 8)),
    ] {
        wire(&mut p, a, b);
    }
    label(&mut p, "VIN", -12, -8);
    label(&mut p, "VOUT", 10, -8);
    p
}
pub fn rc_filter() -> Project {
    let mut p = voltage_divider();
    p.name = "RC — R=1 kΩ, C=1 µF".into();
    if let Some(c) = p.components.values_mut().find(|c| c.reference == "R2") {
        c.kind = ComponentKind::Capacitor;
        c.reference = "C1".into();
        c.value = 1e-6;
    }
    p
}
pub fn diode_limiter() -> Project {
    let mut p = voltage_divider();
    p.name = "Diodo genérico — limitador".into();
    if let Some(c) = p.components.values_mut().find(|c| c.reference == "R2") {
        c.kind = ComponentKind::Diode;
        c.reference = "D1".into();
        c.value = 1e-12;
    }
    p
}

pub fn ac_filter() -> Project {
    let mut p = rc_filter();
    p.name = "RC com fonte senoidal de 1 kHz".into();
    if let Some(c) = p
        .components
        .values_mut()
        .find(|c| c.kind == ComponentKind::VoltageSource)
    {
        c.kind = ComponentKind::SineVoltage;
        c.value = 5.0;
    }
    p
}
fn insert_series(p: &mut Project, kind: ComponentKind, value: f64) {
    p.wires
        .retain(|_, w| !(w.start == Point::new(3, -8) && w.end == Point::new(10, -8)));
    component(p, kind, &format!("{}1", kind.prefix()), value, 7, -8, 0);
    wire(p, (3, -8), (4, -8));
}
pub fn switched_led() -> Project {
    let mut p = voltage_divider();
    p.name = "LED com chave interativa".into();
    for c in p.components.values_mut() {
        match c.reference.as_str() {
            "V1" => c.value = 5.0,
            "R1" => c.value = 330.0,
            "R2" => {
                c.kind = ComponentKind::Led;
                c.reference = "D1".into();
                c.value = ComponentKind::Led.default_value();
            }
            _ => {}
        }
    }
    insert_series(&mut p, ComponentKind::Switch, 0.0);
    p
}
pub fn rlc_step() -> Project {
    let mut p = rc_filter();
    p.name = "RLC amortecido — resposta ao degrau".into();
    if let Some(c) = p.components.values_mut().find(|c| c.reference == "R1") {
        c.value = 10.0;
    }
    insert_series(&mut p, ComponentKind::Inductor, 1e-3);
    p
}
pub fn zener_limiter() -> Project {
    let mut p = diode_limiter();
    p.name = "Zener genérico em polarização reversa".into();
    for c in p.components.values_mut() {
        if c.kind == ComponentKind::Diode {
            c.kind = ComponentKind::Zener;
            c.value = 5.1;
        }
        if c.kind == ComponentKind::VoltageSource {
            c.value = -10.0;
        }
    }
    p
}

type LabeledElement<'a> = (ComponentKind, f64, i32, i32, u8, &'a [&'a str]);
fn labeled(name: &str, elements: &[LabeledElement<'_>]) -> Project {
    let mut p = Project::empty(name);
    for (index, (kind, value, x, y, rotation, names)) in elements.iter().enumerate() {
        let id = ComponentId(p.next_id);
        component(
            &mut p,
            *kind,
            &format!("{}{}", kind.prefix(), index + 1),
            *value,
            *x,
            *y,
            *rotation,
        );
        let pins = p.components[&id].pins();
        for (pin, name) in pins.iter().zip(*names) {
            label(&mut p, name, pin.position.x, pin.position.y);
        }
    }
    component(&mut p, ComponentKind::Ground, "GND1", 0.0, -20, 18, 0);
    label(&mut p, "0", -20, 18);
    p
}
pub fn potentiometer() -> Project {
    labeled(
        "Potenciômetro carregado — ajuste o cursor",
        &[
            (ComponentKind::VoltageSource, 10.0, -16, 0, 1, &["VCC", "0"]),
            (
                ComponentKind::Potentiometer,
                10000.0,
                0,
                0,
                0,
                &["VCC", "0", "VOUT"],
            ),
            (ComponentKind::Resistor, 10000.0, 16, 0, 1, &["VOUT", "0"]),
        ],
    )
}
pub fn opamp_follower() -> Project {
    labeled(
        "Seguidor com operacional genérico",
        &[
            (ComponentKind::VoltageSource, 1.0, -16, 0, 1, &["VIN", "0"]),
            (ComponentKind::OpAmp, 0.0, 0, 0, 0, &["VIN", "VOUT", "VOUT"]),
            (ComponentKind::Resistor, 1000.0, 16, 0, 1, &["VOUT", "0"]),
        ],
    )
}
pub fn logic_xor() -> Project {
    labeled(
        "Porta XOR — alterne as entradas com dois cliques",
        &[
            (ComponentKind::LogicInput, 5.0, -16, -7, 1, &["A", "0"]),
            (ComponentKind::LogicInput, 5.0, -16, 7, 1, &["B", "0"]),
            (ComponentKind::LogicXor, 5.0, 0, 0, 0, &["A", "B", "VOUT"]),
            (ComponentKind::Resistor, 10000.0, 16, 0, 1, &["VOUT", "0"]),
        ],
    )
}
pub fn transistor_bias() -> Project {
    labeled(
        "NPN genérico — polarização em região ativa",
        &[
            (
                ComponentKind::VoltageSource,
                10.0,
                -24,
                -8,
                1,
                &["VCC", "0"],
            ),
            (ComponentKind::VoltageSource, 1.0, -24, 8, 1, &["BIAS", "0"]),
            (
                ComponentKind::Resistor,
                100000.0,
                -10,
                8,
                0,
                &["BIAS", "BASE"],
            ),
            (ComponentKind::Resistor, 1000.0, 14, -8, 1, &["VCC", "VOUT"]),
            (ComponentKind::Npn, 100.0, 4, 0, 0, &["BASE", "VOUT", "0"]),
        ],
    )
}
pub type ExampleFactory = (&'static str, &'static str, fn() -> Project);
pub fn mosfet_switch() -> Project {
    let mut p = labeled(
        "NMOS genérico — chaveamento de carga",
        &[
            (
                ComponentKind::VoltageSource,
                12.0,
                -24,
                -8,
                1,
                &["VCC", "0"],
            ),
            (ComponentKind::SquareVoltage, 2.5, -24, 7, 1, &["GATE", "0"]),
            (ComponentKind::Resistor, 1000.0, 15, -8, 1, &["VCC", "VOUT"]),
            (ComponentKind::Nmos, 0.02, 0, 0, 0, &["GATE", "VOUT", "0"]),
        ],
    );
    if let Some(c) = p
        .components
        .values_mut()
        .find(|c| c.kind == ComponentKind::SquareVoltage)
    {
        c.parameters.offset_v = 2.5;
        c.parameters.frequency_hz = 500.0;
        c.parameters.phase_deg = 180.0;
    }
    p
}
pub fn timer_astable() -> Project {
    labeled(
        "555 genérico — oscilador RC",
        &[
            (ComponentKind::VoltageSource, 5.0, -24, -8, 1, &["VCC", "0"]),
            (
                ComponentKind::Timer555,
                0.0,
                0,
                0,
                0,
                &["0", "CAP", "VOUT", "VCC", "CTRL", "CAP", "DIS", "VCC"],
            ),
            (ComponentKind::Resistor, 10000.0, 20, -8, 1, &["VCC", "DIS"]),
            (ComponentKind::Resistor, 10000.0, 20, 3, 1, &["DIS", "CAP"]),
            (ComponentKind::Capacitor, 1e-7, 20, 14, 1, &["CAP", "0"]),
            (
                ComponentKind::Resistor,
                100000.0,
                -16,
                12,
                1,
                &["VOUT", "0"],
            ),
        ],
    )
}
pub fn timer_monostable() -> Project {
    let mut p = labeled(
        "555 genérico — pulso monoestável",
        &[
            (ComponentKind::VoltageSource, 5.0, -24, -8, 1, &["VCC", "0"]),
            (ComponentKind::SquareVoltage, 2.5, -24, 6, 1, &["TRIG", "0"]),
            (
                ComponentKind::Timer555,
                0.0,
                0,
                0,
                0,
                &["0", "TRIG", "VOUT", "VCC", "CTRL", "CAP", "CAP", "VCC"],
            ),
            (ComponentKind::Resistor, 10000.0, 20, -5, 1, &["VCC", "CAP"]),
            (ComponentKind::Capacitor, 1e-7, 20, 8, 1, &["CAP", "0"]),
            (
                ComponentKind::Resistor,
                100000.0,
                -12,
                14,
                1,
                &["VOUT", "0"],
            ),
        ],
    );
    if let Some(c) = p
        .components
        .values_mut()
        .find(|c| c.kind == ComponentKind::SquareVoltage)
    {
        c.parameters.offset_v = 2.5;
        c.parameters.frequency_hz = 100.0;
        c.parameters.duty_cycle = 0.95;
        c.parameters.phase_deg = 340.0;
    }
    p
}
pub fn d_flip_flop() -> Project {
    let mut p = labeled(
        "Flip-flop D — divisão por dois e clear interativo",
        &[
            (ComponentKind::VoltageSource, 5.0, -24, -8, 1, &["VCC", "0"]),
            (ComponentKind::SquareVoltage, 2.5, -24, 5, 1, &["CLK", "0"]),
            (ComponentKind::LogicInput, 5.0, -24, 17, 1, &["CLRn", "0"]),
            (
                ComponentKind::DFlipFlop,
                0.0,
                0,
                0,
                0,
                &["QN", "CLK", "VCC", "CLRn", "VOUT", "QN", "VCC", "0"],
            ),
            (ComponentKind::Resistor, 10000.0, 20, -3, 1, &["VOUT", "0"]),
            (ComponentKind::Resistor, 10000.0, 20, 9, 1, &["QN", "0"]),
        ],
    );
    for c in p.components.values_mut() {
        if c.kind == ComponentKind::SquareVoltage {
            c.parameters.offset_v = 2.5;
            c.parameters.frequency_hz = 500.0;
            c.parameters.phase_deg = 180.0;
        }
        if c.kind == ComponentKind::LogicInput {
            c.parameters.closed = true;
        }
    }
    p
}
fn counter(kind: ComponentKind) -> Project {
    let outputs = ["Q0", "Q1", "Q2", "Q3", "Q4", "Q5", "Q6", "Q7", "Q8", "Q9"];
    let mut pins = vec!["CLK", "0", "VCC", "VCC", "0"];
    pins.extend_from_slice(
        &outputs[..if kind == ComponentKind::Counter4 {
            4
        } else {
            10
        }],
    );
    let mut p = labeled(
        if kind == ComponentKind::Counter4 {
            "Contador binário — 4 bits"
        } else {
            "Contador de década — dez saídas"
        },
        &[
            (ComponentKind::VoltageSource, 5.0, -24, -8, 1, &["VCC", "0"]),
            (ComponentKind::SquareVoltage, 2.5, -24, 5, 1, &["CLK", "0"]),
            (kind, 0.0, 0, 0, 0, &pins),
            (ComponentKind::Resistor, 330.0, 18, 13, 0, &["Q0", "LED"]),
            (
                ComponentKind::Led,
                ComponentKind::Led.default_value(),
                18,
                23,
                1,
                &["LED", "0"],
            ),
        ],
    );
    if let Some(c) = p
        .components
        .values_mut()
        .find(|c| c.kind == ComponentKind::SquareVoltage)
    {
        c.parameters.offset_v = 2.5;
        c.parameters.frequency_hz = 1000.0;
        c.parameters.phase_deg = 180.0;
    }
    p
}
pub fn binary_counter() -> Project {
    counter(ComponentKind::Counter4)
}
pub fn decade_counter() -> Project {
    counter(ComponentKind::Counter10)
}
pub fn schmitt_trigger() -> Project {
    let mut p = labeled(
        "Schmitt inversor — histerese alimentada",
        &[
            (ComponentKind::VoltageSource, 5.0, -24, -7, 1, &["VCC", "0"]),
            (
                ComponentKind::TriangleVoltage,
                2.5,
                -24,
                7,
                1,
                &["VIN", "0"],
            ),
            (
                ComponentKind::Schmitt,
                0.0,
                0,
                0,
                0,
                &["VIN", "VOUT", "VCC", "0"],
            ),
            (ComponentKind::Resistor, 10000.0, 18, 0, 1, &["VOUT", "0"]),
        ],
    );
    if let Some(c) = p
        .components
        .values_mut()
        .find(|c| c.kind == ComponentKind::TriangleVoltage)
    {
        c.parameters.offset_v = 2.5;
    }
    p
}
pub fn comparator() -> Project {
    let mut p = labeled(
        "Comparador alimentado — limiar e histerese",
        &[
            (ComponentKind::VoltageSource, 5.0, -24, -8, 1, &["VCC", "0"]),
            (ComponentKind::VoltageSource, 2.5, -24, 6, 1, &["REF", "0"]),
            (ComponentKind::SineVoltage, 2.5, -12, 14, 1, &["VIN", "0"]),
            (
                ComponentKind::Comparator,
                0.0,
                0,
                0,
                0,
                &["VIN", "REF", "VOUT", "VCC", "0"],
            ),
            (ComponentKind::Resistor, 10000.0, 18, 0, 1, &["VOUT", "0"]),
        ],
    );
    for c in p.components.values_mut() {
        if c.kind == ComponentKind::SineVoltage {
            c.parameters.offset_v = 2.5;
        }
        if c.kind == ComponentKind::Comparator {
            c.parameters.hysteresis_v = 0.5;
        }
    }
    p
}
pub fn controlled_amplifier() -> Project {
    let mut p = labeled(
        "Fonte controlada — amplificação ideal",
        &[
            (ComponentKind::SineVoltage, 1.0, -18, 0, 1, &["VIN", "0"]),
            (
                ComponentKind::Vcvs,
                5.0,
                0,
                0,
                0,
                &["VIN", "0", "VOUT", "0"],
            ),
            (ComponentKind::Resistor, 10000.0, 18, 0, 1, &["VOUT", "0"]),
        ],
    );
    if let Some(c) = p
        .components
        .values_mut()
        .find(|c| c.kind == ComponentKind::SineVoltage)
    {
        c.parameters.frequency_hz = 1000.0;
    }
    p
}
pub const CATALOG: &[ExampleFactory] = &[
    ("divisor", "Divisor de tensão", voltage_divider),
    ("rc", "Filtro RC", rc_filter),
    ("diodo", "Limitador com diodo", diode_limiter),
    ("rc-senoidal", "Filtro RC senoidal", ac_filter),
    ("led-chave", "LED com chave", switched_led),
    ("rlc", "Resposta RLC", rlc_step),
    ("zener", "Zener reverso", zener_limiter),
    ("potenciometro", "Potenciômetro", potentiometer),
    ("operacional", "Seguidor operacional", opamp_follower),
    ("logica-xor", "Lógica XOR", logic_xor),
    ("transistor", "Transistor NPN", transistor_bias),
    ("mosfet", "Chave NMOS", mosfet_switch),
    ("555-astavel", "555 astável", timer_astable),
    ("555-monoestavel", "555 monoestável", timer_monostable),
    ("flip-flop-d", "Flip-flop D", d_flip_flop),
    ("contador-binario", "Contador binário", binary_counter),
    ("contador-decada", "Contador de década", decade_counter),
    ("schmitt", "Schmitt com histerese", schmitt_trigger),
    ("comparador", "Comparador alimentado", comparator),
    (
        "fonte-controlada",
        "Amplificador controlado",
        controlled_amplifier,
    ),
];
