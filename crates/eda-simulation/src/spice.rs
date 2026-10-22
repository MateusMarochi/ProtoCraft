use super::*;

pub(crate) fn export(project: &Project, analysis: Analysis) -> Result<String, SimulationError> {
    steps(analysis)?;
    let circuit = Circuit::new(project)?;
    let mut out = String::from("* ProtoCraft - netlist original em SI\n");
    for c in project.components.values() {
        use ComponentKind::*;
        if matches!(c.kind, Ground | Connector) {
            continue;
        }
        let nodes: Vec<_> = (0..c.kind.pin_count())
            .map(|pin| {
                circuit
                    .netlist
                    .net_for_pin(PinRef {
                        component: c.id,
                        pin: u16::from(pin),
                    })
                    .map(|n| n.0)
                    .ok_or_else(|| SimulationError::InvalidProject("pino sem rede".into()))
            })
            .collect::<Result<_, _>>()?;
        let (p, n, id, value) = (nodes[0], nodes[1], c.id.0, c.value);
        let params = &c.parameters;
        match c.kind {
            Vcvs | Vccs => {
                let prefix = if c.kind == Vcvs { "E" } else { "G" };
                out.push_str(&format!(
                    "{prefix}{id} {} {} {p} {n} {value:.15e}\n",
                    nodes[2], nodes[3]
                ));
            }
            Nmos | Pmos => {
                let (gate, drain, source) = (nodes[0], nodes[1], nodes[2]);
                let kind = if c.kind == Nmos { "NMOS" } else { "PMOS" };
                let vto = if c.kind == Nmos {
                    params.mos_threshold_v
                } else {
                    -params.mos_threshold_v
                };
                out.push_str(&format!("M{id} {drain} {gate} {source} {source} MOSGEN{id} W=1u L=1u\n.model MOSGEN{id} {kind}(LEVEL=1 VTO={vto:.15e} KP={value:.15e} LAMBDA={:.15e} IS={:.15e})\nR{id}GS {gate} {source} 1e12\nR{id}DS {drain} {source} 1e12\n",params.channel_modulation,params.saturation_current));
            }
            Resistor | Capacitor | Inductor | CurrentSource => {
                let prefix = match c.kind {
                    Resistor => "R",
                    Capacitor => "C",
                    Inductor => "L",
                    _ => "I",
                };
                out.push_str(&format!("{prefix}{id} {p} {n} {value:.15e}\n"));
            }
            VoltageSource | LogicInput => out.push_str(&format!(
                "V{id} {p} {n} {:.15e}\n",
                models::source_value(c, 0.0, true)
            )),
            SineVoltage => out.push_str(&format!(
                "V{id} {p} {n} DC {:.15e} SIN({:.15e} {value:.15e} {:.15e} 0 0 {:.15e})\n",
                params.offset_v, params.offset_v, params.frequency_hz, params.phase_deg
            )),
            SquareVoltage if params.phase_deg == 0.0 => {
                let period = 1.0 / params.frequency_hz;
                let rise = (period * 1e-9).max(1e-15);
                out.push_str(&format!("V{id} {p} {n} DC {:.15e} PULSE({:.15e} {:.15e} 0 {rise:.15e} {rise:.15e} {:.15e} {period:.15e})\n",params.offset_v,params.offset_v-value,params.offset_v+value,period*params.duty_cycle));
            }
            Switch => out.push_str(&format!(
                "R{id} {p} {n} {:.15e}\n",
                if params.closed {
                    params.on_resistance
                } else {
                    params.off_resistance
                }
            )),
            Potentiometer => {
                let (r1, r2) = models::pot_resistances(c);
                let w = nodes[2];
                out.push_str(&format!(
                    "R{id}A {p} {w} {r1:.15e}\nR{id}B {w} {n} {r2:.15e}\n"
                ));
            }
            Diode | Led | Zener => {
                let saturation = if c.kind == Zener {
                    params.saturation_current
                } else {
                    value
                };
                out.push_str(&format!(
                    "D{id} {p} {n} DGEN{id}\n.model DGEN{id} D(Is={saturation:.15e} N={:.15e}",
                    params.ideality
                ));
                if c.kind == Zener {
                    out.push_str(&format!(" BV={value:.15e} IBV={saturation:.15e}"));
                }
                out.push_str(")\n");
            }
            Npn | Pnp => {
                let (b, col, e) = (nodes[0], nodes[1], nodes[2]);
                let kind = if c.kind == Npn { "NPN" } else { "PNP" };
                out.push_str(&format!("Q{id} {col} {b} {e} QGEN{id}\n.model QGEN{id} {kind}(IS={:.15e} BF={value:.15e} BR=1 NF={:.15e} NR={:.15e})\n",params.saturation_current,params.ideality,params.ideality));
            }
            _ => {
                return Err(SimulationError::Unsupported(format!(
                    "exportação SPICE de {} ({})",
                    c.reference,
                    c.kind.label()
                )));
            }
        }
    }
    match analysis {
        Analysis::Dc => out.push_str(".op\n"),
        Analysis::Transient { step, stop } => {
            out.push_str(&format!(".tran {step:.15e} {stop:.15e} UIC\n"))
        }
    }
    out.push_str(".end\n");
    Ok(out)
}
