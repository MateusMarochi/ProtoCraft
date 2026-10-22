//! PCB de duas camadas, conectividade derivada do esquema e DRC geométrico conservador.
use eda_core::*;
use eda_netlist::{NetId, Netlist, build_netlist};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
mod drilling;
mod export;
mod routing;
pub use export::{export_kicad, export_svg};
pub use routing::{RouteMode, RoutingTarget, check_edit, manual_path, routing_target};

#[derive(Debug, Clone, Serialize)]
pub struct PlacedPad {
    pub component: ComponentId,
    pub number: String,
    pub pin: Option<PinRef>,
    pub net: Option<NetId>,
    pub position: NmPoint,
    pub polygon: Vec<NmPoint>,
    pub polygons: Vec<Vec<NmPoint>>,
    pub layers: u8,
    pub pad: Pad,
}
#[derive(Debug, Clone, Serialize)]
pub struct Airwire {
    pub net: NetId,
    pub start: NmPoint,
    pub end: NmPoint,
}
#[derive(Debug, Clone, Serialize)]
pub struct PcbDiagnostic {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub components: Vec<ComponentId>,
    pub position: Option<NmPoint>,
}
#[derive(Debug, Clone, Serialize)]
pub struct PcbReport {
    pub diagnostics: Vec<PcbDiagnostic>,
    pub airwires: Vec<Airwire>,
    pub placed: usize,
    pub pads: usize,
    pub routed_length_mm: f64,
}
impl PcbReport {
    pub fn errors(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == "error")
            .count()
    }
}
fn mm(v: i64) -> f64 {
    v as f64 / NM_PER_MM as f64
}
fn layer_mask(pad: &Pad, side: BoardSide) -> u8 {
    if pad.layers.iter().any(|l| l == "*.Cu") {
        return 3;
    }
    let mut bits = 0;
    for l in &pad.layers {
        if l == "F.Cu" {
            bits |= 1;
        }
        if l == "B.Cu" {
            bits |= 2;
        }
    }
    if side == BoardSide::Bottom {
        (bits & 1) << 1 | (bits & 2) >> 1
    } else {
        bits
    }
}
pub fn pad_polygon(pad: &Pad, placement: &Placement) -> Vec<NmPoint> {
    let angle = (pad.rotation_mdeg as f64 / 1000.0).to_radians();
    let (sin, cos) = angle.sin_cos();
    let convert = |x: f64, y: f64| {
        placement.transform(NmPoint::new(
            pad.position.x + (cos * x - sin * y).round() as i64,
            pad.position.y + (sin * x + cos * y).round() as i64,
        ))
    };
    let (w, h) = (pad.size.x as f64 * 0.5, pad.size.y as f64 * 0.5);
    match pad.shape {
        PadShape::Circle => (0..64)
            .map(|i| {
                let a = i as f64 * std::f64::consts::TAU / 64.0;
                convert(w * a.cos(), h * a.sin())
            })
            .collect(),
        // Para DRC, o retângulo externo é conservador em pads arredondados/ovais.
        _ => [(-w, -h), (w, -h), (w, h), (-w, h)]
            .iter()
            .map(|(x, y)| convert(*x, *y))
            .collect(),
    }
}
fn custom_polygons(pad: &Pad, placement: &Placement) -> Vec<Vec<NmPoint>> {
    let angle = (pad.rotation_mdeg as f64 / 1000.0).to_radians();
    let (sin, cos) = angle.sin_cos();
    let mut result = vec![pad_polygon(pad, placement)];
    if pad.shape == PadShape::Custom {
        for poly in &pad.polygons {
            result.push(
                poly.iter()
                    .map(|p| {
                        placement.transform(NmPoint::new(
                            pad.position.x + (cos * p.x as f64 - sin * p.y as f64).round() as i64,
                            pad.position.y + (sin * p.x as f64 + cos * p.y as f64).round() as i64,
                        ))
                    })
                    .collect(),
            );
        }
    }
    result
}
pub fn placed_pads(project: &Project) -> Result<Vec<PlacedPad>, String> {
    project.validate().map_err(|e| e.to_string())?;
    let board = project.board.as_ref().ok_or("O projeto não tem PCB.")?;
    let netlist = build_netlist(project);
    let mut result = Vec::new();
    for (id, placement) in &board.placements {
        let c = &project.components[id];
        let fp = c.footprint.as_ref().ok_or("Footprint ausente")?;
        let mut mapping = BTreeMap::new();
        for p in c.pins() {
            for number in c.pads_for_pin(p.reference.pin) {
                mapping.insert(number, p.reference);
            }
        }
        for pad in &fp.pads {
            let pin = mapping.get(&pad.number).copied();
            result.push(PlacedPad {
                component: *id,
                number: pad.number.clone(),
                pin,
                net: pin.and_then(|p| netlist.net_for_pin(p)),
                position: placement.transform(pad.position),
                polygon: pad_polygon(pad, placement),
                polygons: custom_polygons(pad, placement),
                layers: layer_mask(pad, placement.side),
                pad: pad.clone(),
            });
        }
    }
    Ok(result)
}
fn bounds(points: &[NmPoint]) -> (i64, i64, i64, i64) {
    points.iter().fold(
        (i64::MAX, i64::MAX, i64::MIN, i64::MIN),
        |(a, b, c, d), p| (a.min(p.x), b.min(p.y), c.max(p.x), d.max(p.y)),
    )
}
fn segment_point(p: NmPoint, a: NmPoint, b: NmPoint) -> f64 {
    let (x, y) = ((b.x - a.x) as f64, (b.y - a.y) as f64);
    let len = x * x + y * y;
    let t = if len == 0.0 {
        0.0
    } else {
        (((p.x - a.x) as f64 * x + (p.y - a.y) as f64 * y) / len).clamp(0.0, 1.0)
    };
    ((p.x - a.x) as f64 - t * x).hypot((p.y - a.y) as f64 - t * y)
}
fn cross(a: NmPoint, b: NmPoint, c: NmPoint) -> i128 {
    (b.x - a.x) as i128 * (c.y - a.y) as i128 - (b.y - a.y) as i128 * (c.x - a.x) as i128
}
fn segment_distance(a: NmPoint, b: NmPoint, c: NmPoint, d: NmPoint) -> f64 {
    let (x, y, z, w) = (
        cross(a, b, c),
        cross(a, b, d),
        cross(c, d, a),
        cross(c, d, b),
    );
    if (x > 0 && y < 0 || x < 0 && y > 0) && (z > 0 && w < 0 || z < 0 && w > 0) {
        return 0.0;
    }
    segment_point(a, c, d)
        .min(segment_point(b, c, d))
        .min(segment_point(c, a, b))
        .min(segment_point(d, a, b))
}
fn inside(point: NmPoint, poly: &[NmPoint]) -> bool {
    let mut result = false;
    for (a, b) in poly
        .iter()
        .zip(poly.iter().cycle().skip(1))
        .take(poly.len())
    {
        if segment_point(point, *a, *b) < 0.5 {
            return true;
        }
        if (a.y > point.y) != (b.y > point.y)
            && (point.x as f64)
                < (b.x - a.x) as f64 * (point.y - a.y) as f64 / (b.y - a.y) as f64 + a.x as f64
        {
            result = !result;
        }
    }
    result
}
#[derive(Clone)]
struct Copper {
    points: Vec<NmPoint>,
    radius: f64,
    filled: bool,
    layers: u8,
    net: Option<NetId>,
    component: Option<ComponentId>,
    label: String,
}
fn distance(a: &Copper, b: &Copper) -> f64 {
    if a.filled && b.points.iter().any(|p| inside(*p, &a.points))
        || b.filled && a.points.iter().any(|p| inside(*p, &b.points))
    {
        return -a.radius - b.radius;
    }
    let edges = |p: &Copper| -> Vec<_> {
        if p.points.len() == 1 {
            vec![(p.points[0], p.points[0])]
        } else if p.filled {
            p.points
                .iter()
                .copied()
                .zip(p.points.iter().copied().cycle().skip(1))
                .take(p.points.len())
                .collect()
        } else {
            vec![(p.points[0], p.points[1])]
        }
    };
    edges(a)
        .iter()
        .flat_map(|(x, y)| {
            edges(b)
                .into_iter()
                .map(move |(z, w)| segment_distance(*x, *y, z, w))
        })
        .fold(f64::INFINITY, f64::min)
        - a.radius
        - b.radius
}
fn root(parents: &mut [usize], mut index: usize) -> usize {
    while parents[index] != index {
        parents[index] = parents[parents[index]];
        index = parents[index];
    }
    index
}
fn union(parents: &mut [usize], a: usize, b: usize) {
    let x = root(parents, a);
    let y = root(parents, b);
    parents[y] = x;
}
pub fn check(project: &Project) -> Result<PcbReport, String> {
    let pads = placed_pads(project)?;
    let board = project.board.as_ref().ok_or("PCB ausente")?;
    let netlist = build_netlist(project);
    let mut report = PcbReport {
        diagnostics: Vec::new(),
        airwires: Vec::new(),
        placed: board.placements.len(),
        pads: pads.len(),
        routed_length_mm: 0.0,
    };
    if project
        .import_info
        .as_ref()
        .is_some_and(|i| !i.electrical_complete)
    {
        report.diagnostics.push(PcbDiagnostic {
            severity: "error".into(),
            code: "INCOMPLETE_IMPORT".into(),
            message: "Importação elétrica parcial: confira a conectividade antes de fabricar."
                .into(),
            components: vec![],
            position: None,
        });
    }
    for d in eda_netlist::erc(project, &netlist)
        .into_iter()
        .filter(|d| d.code == "NC_CONNECTED")
    {
        report.diagnostics.push(PcbDiagnostic {
            severity: "error".into(),
            code: d.code,
            message: d.message,
            components: d
                .objects
                .iter()
                .filter_map(|o| {
                    if let ObjectId::Component(id) = o {
                        Some(*id)
                    } else {
                        None
                    }
                })
                .collect(),
            position: None,
        });
    }
    let mut atoms = Vec::new();
    for p in &pads {
        if p.layers != 0 && p.pad.technology != PadTechnology::NonPlated {
            let circle = p.pad.shape == PadShape::Circle && p.pad.size.x == p.pad.size.y;
            for polygon in &p.polygons {
                atoms.push(Copper {
                    points: if circle {
                        vec![p.position]
                    } else {
                        polygon.clone()
                    },
                    radius: if circle {
                        p.pad.size.x as f64 / 2.0
                    } else {
                        0.0
                    },
                    filled: !circle,
                    layers: p.layers,
                    net: p.net,
                    component: Some(p.component),
                    label: format!(
                        "{}:{}",
                        project.components[&p.component].reference, p.number
                    ),
                });
            }
        }
    }
    for t in board.tracks.values() {
        report.routed_length_mm += mm(t.end.x - t.start.x).hypot(mm(t.end.y - t.start.y));
        atoms.push(Copper {
            points: vec![t.start, t.end],
            radius: t.width_nm as f64 / 2.0,
            filled: false,
            layers: if t.layer == BoardSide::Top { 1 } else { 2 },
            net: netlist.net_for_pin(t.net),
            component: None,
            label: format!("trilha {}", t.id),
        });
    }
    for v in board.vias.values() {
        atoms.push(Copper {
            points: vec![v.position],
            radius: v.diameter_nm as f64 / 2.0,
            filled: false,
            layers: 3,
            net: netlist.net_for_pin(v.net),
            component: None,
            label: format!("via {}", v.id),
        });
    }
    if atoms.len() > 10000 {
        return Err("DRC limitado a 10.000 objetos de cobre por análise.".into());
    }
    report
        .diagnostics
        .extend(drilling::check_drills(project, &pads, &atoms)?);
    let mut add = |code: &str,
                   message: String,
                   components: Vec<ComponentId>,
                   position: Option<NmPoint>,
                   error: bool| {
        if report.diagnostics.len() < 2000 {
            report.diagnostics.push(PcbDiagnostic {
                severity: if error { "error" } else { "warning" }.into(),
                code: code.into(),
                message,
                components,
                position,
            });
        }
    };
    for c in project.components.values() {
        if matches!(c.kind, ComponentKind::Ground | ComponentKind::Connector) {
            continue;
        }
        if c.footprint.is_none() {
            add(
                "MISSING_FOOTPRINT",
                format!("{} não tem footprint.", c.reference),
                vec![c.id],
                None,
                true,
            );
        } else if !board.placements.contains_key(&c.id) {
            add(
                "UNPLACED",
                format!("{} ainda não foi posicionado.", c.reference),
                vec![c.id],
                None,
                true,
            );
        }
    }
    for atom in &atoms {
        let (a, b, c, d) = bounds(&atom.points);
        let r = atom.radius as i64 + board.edge_clearance_nm;
        if a - r < 0 || b - r < 0 || c + r > board.width_nm || d + r > board.height_nm {
            add(
                "BOARD_EDGE",
                format!("{} viola a distância à borda.", atom.label),
                atom.component.into_iter().collect(),
                atom.points.first().copied(),
                true,
            );
        }
    }
    for v in board.vias.values() {
        if (v.diameter_nm - v.drill_nm) / 2 < 100000 {
            add(
                "ANNULAR_RING",
                format!("Via {} tem anel menor que 0,1 mm.", v.id),
                vec![],
                Some(v.position),
                true,
            );
        }
    }
    let mut work = 0usize;
    let mut parents: Vec<_> = (0..atoms.len()).collect();
    let mut terminals = BTreeMap::new();
    for (i, atom) in atoms.iter().enumerate() {
        if atom.component.is_some()
            && let Some(previous) = terminals.insert(atom.label.clone(), i)
        {
            union(&mut parents, i, previous);
        }
    }
    let mut sorted: Vec<_> = atoms
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let b = bounds(&a.points);
            (i, b.0 - a.radius as i64, b.2 + a.radius as i64)
        })
        .collect();
    sorted.sort_by_key(|(_, min, _)| *min);
    for (pos, (i, _, max)) in sorted.iter().enumerate() {
        let a = &atoms[*i];
        for (j, min, _) in &sorted[pos + 1..] {
            if *min > *max + board.clearance_nm {
                break;
            }
            let b = &atoms[*j];
            if a.layers & b.layers == 0 {
                continue;
            }
            let ba = bounds(&a.points);
            let bb = bounds(&b.points);
            let reach = board.clearance_nm as f64 + a.radius + b.radius;
            if ba.1 as f64 - bb.3 as f64 > reach || bb.1 as f64 - ba.3 as f64 > reach {
                continue;
            }
            work = work.saturating_add(a.points.len().max(1).saturating_mul(b.points.len().max(1)));
            if work > 2_000_000 {
                return Err("DRC excedeu o orçamento de geometria; reduza sobreposições ou divida a verificação.".into());
            }
            let gap = distance(a, b);
            if a.net.is_some() && a.net == b.net {
                if gap <= 1.0 {
                    union(&mut parents, *i, *j);
                }
            } else if gap < (board.clearance_nm as f64) - 1.0 {
                add(
                    if gap <= 0.0 {
                        "COPPER_SHORT"
                    } else {
                        "CLEARANCE"
                    },
                    format!(
                        "{} e {}: separação {:.4} mm; mínimo {:.4} mm.",
                        a.label,
                        b.label,
                        gap / NM_PER_MM as f64,
                        mm(board.clearance_nm)
                    ),
                    a.component.into_iter().chain(b.component).collect(),
                    a.points.first().copied(),
                    true,
                );
            }
        }
    }
    let mut courtyards = Vec::new();
    for (id, placement) in &board.placements {
        let f = project.components[id]
            .footprint
            .as_ref()
            .ok_or("Footprint ausente")?;
        let points: Vec<_> = f
            .lines
            .iter()
            .filter(|l| l.layer == "F.CrtYd")
            .flat_map(|l| [l.start, l.end])
            .collect();
        let (min, max) = if points.is_empty() {
            (f.body_bounds.min, f.body_bounds.max)
        } else {
            let (a, b, c, d) = bounds(&points);
            (NmPoint::new(a, b), NmPoint::new(c, d))
        };
        let points = vec![
            min,
            NmPoint::new(max.x, min.y),
            max,
            NmPoint::new(min.x, max.y),
        ]
        .into_iter()
        .map(|p| placement.transform(p))
        .collect();
        courtyards.push((
            *id,
            placement.side,
            Copper {
                points,
                radius: 0.0,
                filled: true,
                layers: 3,
                net: None,
                component: Some(*id),
                label: String::new(),
            },
        ));
    }
    for (i, (id, side, a)) in courtyards.iter().enumerate() {
        for (other, other_side, b) in &courtyards[i + 1..] {
            if side == other_side && distance(a, b) <= 0.0 {
                add(
                    "COURTYARD_OVERLAP",
                    format!(
                        "Áreas de montagem de {} e {} se sobrepõem.",
                        project.components[id].reference, project.components[other].reference
                    ),
                    vec![*id, *other],
                    a.points.first().copied(),
                    true,
                );
            }
        }
    }
    for (id, placement) in &board.placements {
        let c = &project.components[id];
        let fp = c.footprint.as_ref().ok_or("Footprint ausente")?;
        for keepout in &fp.keepouts {
            let zone = Copper {
                points: keepout
                    .polygon
                    .iter()
                    .map(|p| placement.transform(*p))
                    .collect(),
                radius: 0.0,
                filled: true,
                layers: 3,
                net: None,
                component: Some(*id),
                label: String::new(),
            };
            for atom in &atoms {
                let forbidden = if atom.component.is_some() {
                    keepout.pads && atom.component != Some(*id)
                } else if atom.layers == 3 {
                    keepout.vias
                } else {
                    keepout.tracks
                };
                if forbidden && distance(&zone, atom) <= 0.0 {
                    add(
                        "KEEPOUT",
                        format!(
                            "{} invade a área de exclusão/antena de {}.",
                            atom.label, c.reference
                        ),
                        vec![*id],
                        atom.points.first().copied(),
                        true,
                    );
                }
            }
        }
    }
    let mut by_net: BTreeMap<NetId, Vec<usize>> = BTreeMap::new();
    for (i, a) in atoms.iter().enumerate() {
        if let Some(n) = a.net {
            by_net.entry(n).or_default().push(i);
        }
    }
    for (net, indices) in by_net {
        let mut islands: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for i in indices {
            islands.entry(root(&mut parents, i)).or_default().push(i);
        }
        let groups: Vec<_> = islands.into_values().collect();
        if groups.len() > 1 {
            let mut connected = BTreeSet::from([0]);
            while connected.len() < groups.len() {
                let mut best: Option<(f64, usize, NmPoint, NmPoint)> = None;
                for &a in &connected {
                    for (b, target) in groups.iter().enumerate() {
                        if connected.contains(&b) {
                            continue;
                        }
                        for i in &groups[a] {
                            for j in target {
                                work = work.saturating_add(1);
                                if work > 2_000_000 {
                                    return Err("DRC excedeu o orçamento de conexões; reduza as ilhas de cobre e execute novamente.".into());
                                }
                                let x = atoms[*i].points[0];
                                let y = atoms[*j].points[0];
                                let d = (x.x - y.x) as f64;
                                let e = (x.y - y.y) as f64;
                                let dist = d * d + e * e;
                                if best.as_ref().is_none_or(|v| dist < v.0) {
                                    best = Some((dist, b, x, y));
                                }
                            }
                        }
                    }
                }
                if let Some((_, b, start, end)) = best {
                    connected.insert(b);
                    report.airwires.push(Airwire { net, start, end });
                } else {
                    break;
                }
            }
            add(
                "UNROUTED",
                format!(
                    "Rede {} contém {} ilhas de cobre desconectadas.",
                    netlist
                        .nets
                        .iter()
                        .find(|n| n.id == net)
                        .map_or("?", |n| n.name.as_str()),
                    groups.len()
                ),
                vec![],
                None,
                true,
            );
        }
    }
    if report.diagnostics.len() == 2000 {
        report.diagnostics.push(PcbDiagnostic {
            severity: "error".into(),
            code: "DIAGNOSTIC_LIMIT".into(),
            message: "Limite de diagnósticos atingido; corrija as violações e execute novamente."
                .into(),
            components: vec![],
            position: None,
        });
    }
    Ok(report)
}

pub fn pin_map(project: &Project) -> Result<Vec<serde_json::Value>, String> {
    project.validate().map_err(|e| e.to_string())?;
    let netlist = build_netlist(project);
    let mut rows = Vec::new();
    for c in project.components.values() {
        if let Some(f) = &c.footprint {
            for pin in c.pins() {
                rows.push(serde_json::json!({"component":c.id,"reference":c.reference,"device":c.device.as_ref().map(|d|&d.id),"pin":c.pin_number(pin.reference.pin),"name":c.pin_name(pin.reference.pin),"pads":c.pads_for_pin(pin.reference.pin),"footprint":f.id,"net":netlist.net_for_pin(pin.reference)}));
            }
        }
    }
    Ok(rows)
}

fn net_number(netlist: &Netlist, anchor: PinRef) -> Result<u32, String> {
    netlist
        .net_for_pin(anchor)
        .map(|n| n.0 + 1)
        .ok_or("Referência de rede não encontrada".into())
}
