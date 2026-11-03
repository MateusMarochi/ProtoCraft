//! Planejamento conservador de movimento com trilhas conectadas.
use super::*;

fn pad_copper(pad: &PlacedPad, points: Vec<NmPoint>, radius: f64) -> bool {
    let probe = Copper {
        points,
        radius,
        filled: false,
        layers: pad.layers,
        net: None,
        component: None,
        label: String::new(),
    };
    pad.polygons.iter().any(|polygon| {
        let copper = Copper {
            points: polygon.clone(),
            radius: 0.0,
            filled: true,
            layers: pad.layers,
            net: pad.net,
            component: Some(pad.component),
            label: String::new(),
        };
        distance(&probe, &copper) <= 0.0
    })
}

/// Planeja uma translação sem alterar o projeto. Recusa contatos que exigem
/// modificar ramificações, vias ou outras geometrias não representadas no lote.
pub fn plan_placement(
    project: &Project,
    id: ComponentId,
    placement: Placement,
) -> Result<Vec<Command>, String> {
    project.validate().map_err(|error| error.to_string())?;
    let board = project.board.as_ref().ok_or("PCB ausente")?;
    let old = *board
        .placements
        .get(&id)
        .ok_or("Componente sem posicionamento PCB")?;
    if !placement.position.valid()
        || placement.side != old.side
        || placement.rotation != old.rotation
    {
        return Err("Somente translação na mesma face e rotação é suportada.".into());
    }
    if placement == old {
        return Ok(Vec::new());
    }
    let mut total_pads = 0usize;
    let mut moving_pads = 0usize;
    for component_id in board.placements.keys() {
        let count = project
            .components
            .get(component_id)
            .and_then(|c| c.footprint.as_ref())
            .map_or(0, |f| f.pads.len());
        total_pads = total_pads.saturating_add(count);
        if *component_id == id {
            moving_pads = count;
        }
    }
    let work = moving_pads
        .saturating_mul(
            total_pads
                .saturating_add(board.tracks.len())
                .saturating_add(board.vias.len()),
        )
        .saturating_add(board.tracks.len().saturating_mul(board.tracks.len()));
    if total_pads > 10_000 || work > 2_000_000 {
        return Err("Movimento excede o limite de geometria para análise segura.".into());
    }
    let pads = placed_pads(project)?;
    let netlist = build_netlist(project);
    let moving: Vec<_> = pads.iter().filter(|p| p.component == id).collect();
    for pad in &moving {
        for other in pads
            .iter()
            .filter(|p| p.component != id && p.layers & pad.layers != 0)
        {
            if pad.polygons.iter().any(|polygon| {
                let a = Copper {
                    points: polygon.clone(),
                    radius: 0.0,
                    filled: true,
                    layers: pad.layers,
                    net: pad.net,
                    component: Some(id),
                    label: String::new(),
                };
                other.polygons.iter().any(|polygon| {
                    let b = Copper {
                        points: polygon.clone(),
                        radius: 0.0,
                        filled: true,
                        layers: other.layers,
                        net: other.net,
                        component: Some(other.component),
                        label: String::new(),
                    };
                    distance(&a, &b) <= 0.0
                })
            }) {
                return Err(
                    "Pads de componentes distintos estão em contato; movimento recusado.".into(),
                );
            }
        }
    }
    let dx = placement.position.x - old.position.x;
    let dy = placement.position.y - old.position.y;
    let mut commands = vec![Command::PlaceOnBoard { id, placement }];
    for track in board.tracks.values() {
        let mask = if track.layer == BoardSide::Top { 1 } else { 2 };
        let touched: Vec<_> = moving
            .iter()
            .filter(|p| {
                p.layers & mask != 0
                    && pad_copper(p, vec![track.start, track.end], track.width_nm as f64 / 2.0)
            })
            .collect();
        if touched.is_empty() {
            continue;
        }
        if touched.len() != 1
            || touched[0].net.is_none()
            || touched[0].net != netlist.net_for_pin(track.net)
        {
            return Err("Contato de trilha com pads é ambíguo ou possui rede incompatível.".into());
        }
        let pad = touched[0];
        let start = pad_copper(pad, vec![track.start], track.width_nm as f64 / 2.0);
        let end = pad_copper(pad, vec![track.end], track.width_nm as f64 / 2.0);
        if start == end {
            return Err("Trilha atravessa o pad ou tem contato ambíguo.".into());
        }
        let anchor = if start { track.start } else { track.end };
        if board.vias.values().any(|v| {
            segment_point(v.position, anchor, anchor)
                <= (v.diameter_nm + track.width_nm) as f64 / 2.0
        }) || board.tracks.values().any(|other| {
            other.id != track.id
                && other.layer == track.layer
                && segment_point(anchor, other.start, other.end)
                    <= (other.width_nm + track.width_nm) as f64 / 2.0
        }) || pads.iter().any(|other| {
            other.component != id
                && other.layers & mask != 0
                && pad_copper(other, vec![anchor], track.width_nm as f64 / 2.0)
        }) {
            return Err(
                "Contato compartilhado por via, trilha ou outro pad; movimento recusado.".into(),
            );
        }
        let mut updated = track.clone();
        let target = NmPoint::new(anchor.x + dx, anchor.y + dy);
        if start {
            updated.start = target;
        } else {
            updated.end = target;
        }
        commands.push(Command::UpdateTrack { track: updated });
    }
    for via in board.vias.values() {
        if moving
            .iter()
            .any(|p| pad_copper(p, vec![via.position], via.diameter_nm as f64 / 2.0))
        {
            return Err("Via em contato com pad impede movimento automático.".into());
        }
    }
    let before = check(project)?;
    let after = check_edit(project, Command::Batch(commands.clone()))?;
    if after.airwires.len() > before.airwires.len() {
        return Err("Movimento criaria conexões pendentes.".into());
    }
    Ok(commands)
}
