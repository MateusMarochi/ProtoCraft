//! Remoção conservadora de trilhas exatamente sobrepostas.
use eda_core::{BoardSide, Command, Project};
use std::collections::BTreeMap;

/// Preserva a trilha de menor ID. Trilhas sem net derivada não são alteradas.
/// O lote precisa ser confirmado pelo Engine; a comparação de DRC ocorre antes da entrega.
fn duplicate_plan(project: &Project) -> Result<Vec<Command>, String> {
    let board = project
        .board
        .as_ref()
        .ok_or_else(|| eda_i18n::text("PCB ausente"))?;
    let netlist = eda_netlist::build_netlist(project);
    let mut seen = BTreeMap::new();
    let mut commands = Vec::new();
    for track in board.tracks.values() {
        let Some(net) = netlist.net_for_pin(track.net) else {
            continue;
        };
        let a = (track.start.x, track.start.y);
        let b = (track.end.x, track.end.y);
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        let layer = match track.layer {
            BoardSide::Top => 0,
            BoardSide::Bottom => 1,
        };
        let key = (net.0, layer, track.width_nm, a, b);
        if seen.insert(key, track.id).is_some() {
            commands.push(Command::DeleteTrack { id: track.id });
        }
    }
    if !commands.is_empty() {
        let before = super::check(project)?;
        let after = super::check_edit(project, Command::Batch(commands.clone()))?;
        if after.airwires.len() > before.airwires.len() {
            return Err(eda_i18n::text("Limpeza criaria conexões pendentes."));
        }
    }
    Ok(commands)
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CleanupOptions {
    pub merge_collinear: bool,
}

#[derive(Debug, Clone)]
pub struct CleanupPlan {
    pub commands: Vec<Command>,
    /// Quantidade total de segmentos excluídos, incluindo duplicatas e uniões.
    pub removed_tracks: usize,
    /// Quantidade de uniões de dois segmentos em um.
    pub merged_tracks: usize,
}

pub fn plan(project: &Project) -> Result<Vec<Command>, String> {
    duplicate_plan(project)
}

fn on_segment(p: eda_core::NmPoint, a: eda_core::NmPoint, b: eda_core::NmPoint) -> bool {
    let cross = (i128::from(p.x) - i128::from(a.x)) * (i128::from(b.y) - i128::from(a.y))
        - (i128::from(p.y) - i128::from(a.y)) * (i128::from(b.x) - i128::from(a.x));
    cross == 0
        && p.x >= a.x.min(b.x)
        && p.x <= a.x.max(b.x)
        && p.y >= a.y.min(b.y)
        && p.y <= a.y.max(b.y)
}

pub fn plan_with_options(
    project: &Project,
    options: &CleanupOptions,
) -> Result<CleanupPlan, String> {
    if options.merge_collinear && project.board.as_ref().is_some_and(|b| b.tracks.len() > 256) {
        return Err(eda_i18n::text(
            "Limpeza colinear limitada a 256 segmentos por operação.",
        ));
    }
    let mut pair_checks = 0usize;
    let mut commands = duplicate_plan(project)?;
    let mut engine = eda_core::Engine::new(project.clone()).map_err(|e| e.to_string())?;
    if !commands.is_empty() {
        engine
            .execute(Command::Batch(commands.clone()))
            .map_err(|e| e.to_string())?;
    }
    let mut merged_tracks = 0;
    if options.merge_collinear {
        loop {
            let current = engine.project();
            let board = current
                .board
                .as_ref()
                .ok_or_else(|| eda_i18n::text("PCB ausente"))?;
            let netlist = eda_netlist::build_netlist(current);
            let pads = super::placed_pads(current)?;
            let tracks: Vec<_> = board.tracks.values().collect();
            let baseline = super::check(current)?;
            let class_keys: std::collections::BTreeSet<_> = baseline
                .diagnostics
                .iter()
                .filter(|d| {
                    matches!(
                        d.code.as_str(),
                        "NETCLASS_TRACK_WIDTH" | "NETCLASS_VIA_SIZE"
                    )
                })
                .map(|d| format!("{}|{}|{:?}", d.code, d.message, d.position))
                .collect();
            let mut candidate = None;
            'pairs: for (i, a) in tracks.iter().enumerate() {
                for b in tracks.iter().skip(i + 1) {
                    pair_checks += 1;
                    if pair_checks > 100_000 {
                        return Err(eda_i18n::text(
                            "Limpeza colinear excedeu o orçamento de comparações.",
                        ));
                    }
                    if a.layer != b.layer || a.width_nm != b.width_nm {
                        continue;
                    }
                    let Some(net) = netlist.net_for_pin(a.net) else {
                        continue;
                    };
                    if netlist.net_for_pin(b.net) != Some(net) {
                        continue;
                    }
                    let mut points = [a.start, a.end, b.start, b.end];
                    if !on_segment(b.start, a.start, a.end)
                        && !on_segment(b.end, a.start, a.end)
                        && !on_segment(a.start, b.start, b.end)
                        && !on_segment(a.end, b.start, b.end)
                    {
                        continue;
                    }
                    let cross = (i128::from(a.end.x) - i128::from(a.start.x))
                        * (i128::from(b.end.y) - i128::from(b.start.y))
                        - (i128::from(a.end.y) - i128::from(a.start.y))
                            * (i128::from(b.end.x) - i128::from(b.start.x));
                    if cross != 0 {
                        continue;
                    }
                    points.sort_by_key(|p| (p.x, p.y));
                    let start = points[0];
                    let end = points[3];
                    let protected = |p| p != start && p != end && on_segment(p, start, end);
                    let layer_mask = if a.layer == BoardSide::Top { 1 } else { 2 };
                    let terminal_radius = a.width_nm as f64 / 2.0;
                    let pad_terminal = |pad: &super::PlacedPad| {
                        pad.layers & layer_mask != 0
                            && points.iter().any(|point| {
                                protected(*point)
                                    && pad.polygons.iter().any(|polygon| {
                                        super::inside(*point, polygon)
                                            || polygon
                                                .iter()
                                                .zip(polygon.iter().cycle().skip(1))
                                                .take(polygon.len())
                                                .any(|(a, b)| {
                                                    super::segment_point(*point, *a, *b)
                                                        <= terminal_radius
                                                })
                                    })
                            })
                    };
                    if pads
                        .iter()
                        .any(|p| protected(p.position) || pad_terminal(p))
                        || board.vias.values().any(|v| protected(v.position))
                        || tracks.iter().any(|t| {
                            t.id != a.id
                                && t.id != b.id
                                && t.layer == a.layer
                                && (protected(t.start)
                                    || protected(t.end)
                                    || points
                                        .iter()
                                        .any(|p| protected(*p) && on_segment(*p, t.start, t.end)))
                        })
                    {
                        continue;
                    }
                    let mut track = (*a).clone();
                    track.start = start;
                    track.end = end;
                    let batch = vec![
                        Command::UpdateTrack { track },
                        Command::DeleteTrack { id: b.id },
                    ];
                    // A união pode invalidar escapes curtos de pads sem mudar o cobre.
                    // O DRC de edição física não inclui as regras de classe.
                    let Ok(report) = super::check_edit(current, Command::Batch(batch.clone()))
                    else {
                        continue;
                    };
                    if report.diagnostics.iter().any(|d| {
                        matches!(
                            d.code.as_str(),
                            "NETCLASS_TRACK_WIDTH" | "NETCLASS_VIA_SIZE"
                        ) && !class_keys
                            .contains(&format!("{}|{}|{:?}", d.code, d.message, d.position))
                    }) {
                        continue;
                    }
                    candidate = Some(batch);
                    break 'pairs;
                }
            }
            let Some(batch) = candidate else {
                break;
            };
            super::check_edit(current, Command::Batch(batch.clone()))?;
            engine
                .execute(Command::Batch(batch.clone()))
                .map_err(|e| e.to_string())?;
            commands.extend(batch);
            merged_tracks += 1;
        }
    }
    if !commands.is_empty() {
        let before = super::check(project)?;
        let after = super::check_edit(project, Command::Batch(commands.clone()))?;
        if after.airwires.len() > before.airwires.len() {
            return Err(eda_i18n::text("Limpeza criaria conexões pendentes."));
        }
    }
    let removed_tracks = commands
        .iter()
        .filter(|c| matches!(c, Command::DeleteTrack { .. }))
        .count();
    Ok(CleanupPlan {
        commands,
        removed_tracks,
        merged_tracks,
    })
}
