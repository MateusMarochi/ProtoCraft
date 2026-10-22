//! Equações próprias de modelos educacionais genéricos, em SI.
use super::{VT, stamp_g};
use eda_core::{Component, ComponentKind};
use std::f64::consts::TAU;

pub(crate) fn source_value(c: &Component, time: f64, dc: bool) -> f64 {
    use ComponentKind::*;
    let p = &c.parameters;
    if c.kind == LogicInput {
        return if p.closed { c.value } else { 0.0 };
    }
    if !c.kind.is_waveform() {
        return c.value;
    }
    if dc {
        return p.offset_v;
    }
    let phase = (time * p.frequency_hz + p.phase_deg / 360.0).rem_euclid(1.0);
    let wave = match c.kind {
        SineVoltage => (TAU * phase).sin(),
        SquareVoltage => {
            if phase < p.duty_cycle {
                1.0
            } else {
                -1.0
            }
        }
        TriangleVoltage => 1.0 - 4.0 * (phase - 0.5).abs(),
        SawVoltage => 2.0 * phase - 1.0,
        _ => 0.0,
    };
    p.offset_v + c.value * wave
}
pub(crate) fn diode_iv(c: &Component, v: f64) -> (f64, f64) {
    let vt = VT * c.parameters.ideality;
    let saturation = if c.kind == ComponentKind::Zener {
        c.parameters.saturation_current
    } else {
        c.value
    };
    let e = (v / vt).clamp(-80.0, 80.0).exp();
    let mut current = saturation * (e - 1.0);
    let mut g = saturation * e / vt;
    if c.kind == ComponentKind::Zener {
        let reverse = saturation * ((-v - c.value) / vt).clamp(-80.0, 80.0).exp();
        current -= reverse;
        g += reverse / vt;
    }
    (current, g)
}
pub(crate) fn transistor(c: &Component, v: [f64; 3]) -> ([f64; 3], [[f64; 3]; 3]) {
    let sign = if c.kind == ComponentKind::Pnp {
        -1.0
    } else {
        1.0
    };
    let vt = VT * c.parameters.ideality;
    let e1 = (sign * (v[0] - v[2]) / vt).clamp(-60.0, 60.0).exp();
    let e2 = (sign * (v[0] - v[1]) / vt).clamp(-60.0, 60.0).exp();
    let is = c.parameters.saturation_current;
    let (i1, i2) = (is * (e1 - 1.0), is * (e2 - 1.0));
    let (g1, g2) = (is * e1 / vt, is * e2 / vt);
    let af = c.value / (c.value + 1.0);
    let ar = 0.5;
    let ic = sign * (af * i1 - i2);
    let ie = sign * (-i1 + ar * i2);
    (
        [-ic - ie, ic, ie],
        [
            [
                (1.0 - af) * g1 + (1.0 - ar) * g2,
                -(1.0 - ar) * g2,
                -(1.0 - af) * g1,
            ],
            [af * g1 - g2, g2, -af * g1],
            [-g1 + ar * g2, -ar * g2, g1],
        ],
    )
}
pub(crate) fn output_target(c: &Component, v: &[f64]) -> f64 {
    use ComponentKind::*;
    if c.kind == OpAmp {
        return (c.parameters.gain * (v[0] - v[1]))
            .clamp(c.parameters.rail_low, c.parameters.rail_high);
    }
    let a = v[0] >= c.parameters.threshold_v;
    let b = v.get(1).copied().unwrap_or(0.0) >= c.parameters.threshold_v;
    let high = match c.kind {
        LogicNot => !a,
        LogicAnd => a && b,
        LogicOr => a || b,
        LogicXor => a ^ b,
        LogicNand => !(a && b),
        LogicNor => !(a || b),
        _ => false,
    };
    if high { c.value } else { 0.0 }
}
pub(crate) fn stamp_bjt(
    c: &Component,
    pins: &[Option<usize>],
    guess: &[f64],
    a: &mut [Vec<f64>],
    b: &mut [f64],
) {
    let v = [
        pins[0].map_or(0.0, |i| guess[i]),
        pins[1].map_or(0.0, |i| guess[i]),
        pins[2].map_or(0.0, |i| guess[i]),
    ];
    let (currents, jacobian) = transistor(c, v);
    for row in 0..3 {
        if let Some(r) = pins[row] {
            let mut equivalent = currents[row];
            for col in 0..3 {
                equivalent -= jacobian[row][col] * v[col];
                if let Some(k) = pins[col] {
                    a[r][k] += jacobian[row][col];
                }
            }
            b[r] -= equivalent;
        }
    }
}
pub(crate) fn stamp_output(
    c: &Component,
    pins: &[Option<usize>],
    guess: &[f64],
    a: &mut [Vec<f64>],
    b: &mut [f64],
) {
    let output = pins.len() - 1;
    for pin in &pins[..output] {
        stamp_g(a, *pin, None, 1e-12);
    }
    let g = 1.0 / c.parameters.output_resistance;
    stamp_g(a, pins[output], None, g);
    let v: Vec<_> = pins.iter().map(|p| p.map_or(0.0, |i| guess[i])).collect();
    let target = output_target(c, &v);
    if let Some(o) = pins[output] {
        let raw = c.parameters.gain * (v[0] - v[1]);
        if c.kind == ComponentKind::OpAmp
            && raw > c.parameters.rail_low
            && raw < c.parameters.rail_high
        {
            if let Some(p) = pins[0] {
                a[o][p] -= g * c.parameters.gain;
            }
            if let Some(n) = pins[1] {
                a[o][n] += g * c.parameters.gain;
            }
        } else {
            b[o] += g * target;
        }
    }
}
pub(crate) fn pot_resistances(c: &Component) -> (f64, f64) {
    (
        (c.value * c.parameters.wiper).max(1e-6),
        (c.value * (1.0 - c.parameters.wiper)).max(1e-6),
    )
}

/// Modelo de canal longo, corpo ligado à fonte; gate, dreno, fonte.
pub(crate) fn mosfet(c: &Component, v: [f64; 3]) -> ([f64; 3], [[f64; 3]; 3]) {
    let sign = if c.kind == ComponentKind::Pmos {
        -1.0
    } else {
        1.0
    };
    let (d, s) = if sign * (v[1] - v[2]) >= 0.0 {
        (1, 2)
    } else {
        (2, 1)
    };
    let ugs = sign * (v[0] - v[s]);
    let uds = sign * (v[d] - v[s]);
    let over = ugs - c.parameters.mos_threshold_v;
    let lambda = c.parameters.channel_modulation;
    let modulation = 1.0 + lambda * uds;
    let (i, gm, gd) = if over <= 0.0 {
        (0.0, 0.0, 0.0)
    } else if uds < over {
        let base = c.value * (over * uds - 0.5 * uds * uds);
        (
            base * modulation,
            c.value * uds * modulation,
            c.value * (over - uds) * modulation + lambda * base,
        )
    } else {
        let base = 0.5 * c.value * over * over;
        (
            base * modulation,
            c.value * over * modulation,
            lambda * base,
        )
    };
    let mut currents = [0.0; 3];
    let mut j = [[0.0; 3]; 3];
    currents[d] += sign * i;
    currents[s] -= sign * i;
    for (pin, derivative) in [(0, gm), (d, gd), (s, -gm - gd)] {
        j[d][pin] += derivative;
        j[s][pin] -= derivative;
    }
    // Diodo do corpo e fugas explícitas, sem capacitâncias parasitas.
    let body =
        c.parameters.saturation_current * ((sign * (v[2] - v[1]) / VT).clamp(-60.0, 60.0).exp());
    let body_current = sign * (body - c.parameters.saturation_current);
    let g = body / VT + 1e-12;
    let current = body_current + 1e-12 * (v[2] - v[1]);
    currents[2] += current;
    currents[1] -= current;
    j[2][2] += g;
    j[2][1] -= g;
    j[1][1] += g;
    j[1][2] -= g;
    let leak = 1e-12 * (v[0] - v[2]);
    currents[0] += leak;
    currents[2] -= leak;
    j[0][0] += 1e-12;
    j[0][2] -= 1e-12;
    j[2][2] += 1e-12;
    j[2][0] -= 1e-12;
    (currents, j)
}
pub(crate) fn stamp_mosfet(
    c: &Component,
    pins: &[Option<usize>],
    guess: &[f64],
    a: &mut [Vec<f64>],
    b: &mut [f64],
) {
    let v = [
        pins[0].map_or(0.0, |i| guess[i]),
        pins[1].map_or(0.0, |i| guess[i]),
        pins[2].map_or(0.0, |i| guess[i]),
    ];
    let (currents, j) = mosfet(c, v);
    for row in 0..3 {
        if let Some(r) = pins[row] {
            let mut equivalent = currents[row];
            for col in 0..3 {
                equivalent -= j[row][col] * v[col];
                if let Some(k) = pins[col] {
                    a[r][k] += j[row][col];
                }
            }
            b[r] -= equivalent;
        }
    }
}
