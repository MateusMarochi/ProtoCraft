use super::*;
struct Drill {
    position: NmPoint,
    radius: f64,
    net: Option<NetId>,
    owner: String,
}
pub(super) fn check_drills(
    project: &Project,
    pads: &[PlacedPad],
    atoms: &[Copper],
) -> Result<Vec<PcbDiagnostic>, String> {
    let board = project.board.as_ref().ok_or("PCB ausente")?;
    let netlist = build_netlist(project);
    let mut drills = Vec::new();
    let mut seen = BTreeSet::new();
    for p in pads {
        if let Some(size) = p.pad.drill {
            let owner = format!(
                "{}:{}",
                project.components[&p.component].reference, p.number
            );
            if seen.insert((owner.clone(), p.position.x, p.position.y)) {
                drills.push(Drill {
                    position: p.position,
                    radius: size.x.max(size.y) as f64 / 2.0,
                    net: if p.pad.technology == PadTechnology::NonPlated {
                        None
                    } else {
                        p.net
                    },
                    owner,
                });
            }
        }
    }
    for v in board.vias.values() {
        drills.push(Drill {
            position: v.position,
            radius: v.drill_nm as f64 / 2.0,
            net: netlist.net_for_pin(v.net),
            owner: format!("via {}", v.id),
        });
    }
    for h in board.holes.values() {
        drills.push(Drill {
            position: h.position,
            radius: h.drill_nm as f64 / 2.0,
            net: None,
            owner: format!("furo {}", h.id),
        });
    }
    let mut diagnostics = Vec::new();
    let mut add = |code: &str, message: String, position: NmPoint| {
        if diagnostics.len() < 1000 {
            diagnostics.push(PcbDiagnostic {
                severity: "error".into(),
                code: code.into(),
                message,
                components: vec![],
                position: Some(position),
            });
        }
    };
    let mut work = 0usize;
    for (i, d) in drills.iter().enumerate() {
        let edge = d.radius + board.edge_clearance_nm as f64;
        if (d.position.x as f64) < edge
            || (d.position.y as f64) < edge
            || d.position.x as f64 + edge > board.width_nm as f64
            || d.position.y as f64 + edge > board.height_nm as f64
        {
            add(
                "HOLE_EDGE",
                format!(
                    "{owner} viola a distância de furação à borda.",
                    owner = d.owner
                ),
                d.position,
            );
        }
        let circle = Copper {
            points: vec![d.position],
            radius: d.radius,
            filled: false,
            layers: 3,
            net: None,
            component: None,
            label: String::new(),
        };
        for atom in atoms {
            if atom.label == d.owner || (d.net.is_some() && d.net == atom.net) {
                continue;
            }
            work = work.saturating_add(atom.points.len().max(1));
            if work > 2_000_000 {
                return Err("DRC de furação excedeu o orçamento de geometria.".into());
            }
            if distance(&circle, atom) < board.clearance_nm as f64 - 1.0 {
                add(
                    "HOLE_COPPER",
                    format!(
                        "{owner} invade a folga de cobre de {item}.",
                        owner = d.owner,
                        item = atom.label
                    ),
                    d.position,
                );
            }
        }
        for other in &drills[i + 1..] {
            work = work.saturating_add(1);
            if work > 2_000_000 {
                return Err("DRC de furação excedeu o orçamento de geometria.".into());
            }
            let gap = ((d.position.x - other.position.x) as f64)
                .hypot((d.position.y - other.position.y) as f64)
                - d.radius
                - other.radius;
            if gap < board.clearance_nm as f64 - 1.0 {
                add(
                    "HOLE_CLEARANCE",
                    format!(
                        "{owner} e {other} violam a folga entre furos.",
                        owner = d.owner,
                        other = other.owner
                    ),
                    d.position,
                );
            }
        }
    }
    for h in board.holes.values() {
        let circle = Copper {
            points: vec![h.position],
            radius: h.drill_nm as f64 / 2.0,
            filled: false,
            layers: 3,
            net: None,
            component: None,
            label: String::new(),
        };
        for (id, placement) in &board.placements {
            for k in &project.components[id]
                .footprint
                .as_ref()
                .ok_or("Footprint ausente")?
                .keepouts
            {
                work = work.saturating_add(k.polygon.len());
                if work > 2_000_000 {
                    return Err("DRC de furação excedeu o orçamento de geometria.".into());
                }
                let zone = Copper {
                    points: k.polygon.iter().map(|p| placement.transform(*p)).collect(),
                    radius: 0.0,
                    filled: true,
                    layers: 3,
                    net: None,
                    component: Some(*id),
                    label: String::new(),
                };
                if distance(&circle, &zone) <= 0.0 {
                    add(
                        "HOLE_KEEPOUT",
                        format!(
                            "Furo {id} invade área de exclusão de {reference}.",
                            id = h.id,
                            reference = project.components[id].reference
                        ),
                        h.position,
                    );
                }
            }
        }
    }
    Ok(diagnostics)
}
