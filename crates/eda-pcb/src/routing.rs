use super::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RouteMode {
    #[default]
    Diagonal,
    Orthogonal,
    Free,
}
impl RouteMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Diagonal => "45°",
            Self::Orthogonal => "90°",
            Self::Free => "Livre",
        }
    }
}
pub fn manual_path(start: NmPoint, end: NmPoint, mode: RouteMode, flip: bool) -> Vec<NmPoint> {
    if !start.valid() || !end.valid() {
        return vec![start, end];
    }
    let (dx, dy) = (end.x - start.x, end.y - start.y);
    let diagonal = dx.abs().min(dy.abs());
    let corner = match mode {
        RouteMode::Free => end,
        RouteMode::Orthogonal => {
            if flip {
                NmPoint::new(start.x, end.y)
            } else {
                NmPoint::new(end.x, start.y)
            }
        }
        RouteMode::Diagonal => {
            if flip {
                NmPoint::new(
                    end.x - diagonal * dx.signum(),
                    end.y - diagonal * dy.signum(),
                )
            } else {
                NmPoint::new(
                    start.x + diagonal * dx.signum(),
                    start.y + diagonal * dy.signum(),
                )
            }
        }
    };
    let mut points = vec![start];
    for p in [corner, end] {
        if points.last() != Some(&p) {
            points.push(p);
        }
    }
    points
}
#[derive(Debug, Clone, Copy)]
pub struct RoutingTarget {
    pub position: NmPoint,
    pub anchor: PinRef,
    pub net: NetId,
}
pub fn routing_target(
    project: &Project,
    pads: &[PlacedPad],
    netlist: &Netlist,
    layer: BoardSide,
    point: NmPoint,
    tolerance: i64,
) -> Option<RoutingTarget> {
    if !point.valid() || tolerance < 0 {
        return None;
    }
    let board = project.board.as_ref()?;
    let mask = if layer == BoardSide::Top { 1 } else { 2 };
    let mut best: Option<(f64, u8, RoutingTarget)> = None;
    let mut candidate =
        |position: NmPoint, anchor: PinRef, net: NetId, radius: f64, priority: u8| {
            let dist = ((position.x - point.x) as f64).hypot((position.y - point.y) as f64);
            if dist <= radius.max(tolerance as f64)
                && best
                    .as_ref()
                    .is_none_or(|(d, p, _)| dist < *d || (dist == *d && priority < *p))
            {
                best = Some((
                    dist,
                    priority,
                    RoutingTarget {
                        position,
                        anchor,
                        net,
                    },
                ));
            }
        };
    for pad in pads {
        if pad.layers & mask != 0
            && let (Some(pin), Some(net)) = (pad.pin, pad.net)
        {
            let radius = if pad.polygons.iter().any(|polygon| inside(point, polygon)) {
                ((pad.position.x - point.x) as f64).hypot((pad.position.y - point.y) as f64) + 1.0
            } else {
                pad.pad.size.x.min(pad.pad.size.y) as f64 / 2.0
            };
            candidate(pad.position, pin, net, radius, 0);
        }
    }
    for v in board.vias.values() {
        if let Some(net) = netlist.net_for_pin(v.net) {
            candidate(v.position, v.net, net, v.diameter_nm as f64 / 2.0, 1);
        }
    }
    for t in board.tracks.values().filter(|t| t.layer == layer) {
        if let Some(net) = netlist.net_for_pin(t.net) {
            let (dx, dy) = ((t.end.x - t.start.x) as f64, (t.end.y - t.start.y) as f64);
            let k = (((point.x - t.start.x) as f64 * dx + (point.y - t.start.y) as f64 * dy)
                / (dx * dx + dy * dy))
                .clamp(0.0, 1.0);
            candidate(
                NmPoint::new(
                    t.start.x + (k * dx).round() as i64,
                    t.start.y + (k * dy).round() as i64,
                ),
                t.net,
                net,
                t.width_nm as f64 / 2.0,
                2,
            );
        }
    }
    best.map(|(_, _, target)| target)
}

/// Erros físicos existentes não impedem uma correção independente; conexões
/// pendentes permanecem no relatório, permitindo terminar o roteamento manual.
pub fn check_edit(project: &Project, command: Command) -> Result<PcbReport, String> {
    let physical = |d: &PcbDiagnostic| {
        d.severity == "error"
            && matches!(
                d.code.as_str(),
                "COPPER_SHORT"
                    | "CLEARANCE"
                    | "BOARD_EDGE"
                    | "ANNULAR_RING"
                    | "KEEPOUT"
                    | "HOLE_EDGE"
                    | "HOLE_COPPER"
                    | "HOLE_CLEARANCE"
                    | "HOLE_KEEPOUT"
            )
    };
    let key = |d: &PcbDiagnostic| format!("{}|{}|{:?}", d.code, d.message, d.position);
    let original = check(project)?;
    if original.diagnostics.len() >= 2000 {
        return Err("Reduza os erros DRC antes de adicionar geometria.".into());
    }
    let baseline: BTreeSet<_> = original
        .diagnostics
        .iter()
        .filter(|d| physical(d))
        .map(&key)
        .collect();
    let mut engine = Engine::new(project.clone()).map_err(|e| e.to_string())?;
    engine.execute(command).map_err(|e| e.to_string())?;
    let report = check(engine.project())?;
    if report.diagnostics.len() >= 2000 {
        return Err("A edição excedeu o limite de diagnósticos DRC.".into());
    }
    if let Some(error) = report
        .diagnostics
        .iter()
        .find(|d| physical(d) && !baseline.contains(&key(d)))
    {
        return Err(format!("{}: {}", error.code, error.message));
    }
    Ok(report)
}
