//! Métricas geométricas de cobre por rede derivadas da netlist atual.
//! Comprimentos somam segmentos por centro, não consideram comprimento vertical
//! das vias, propagação, impedância nem garantem continuidade física.
use eda_core::{BoardSide, NM_PER_MM, Project};
use eda_netlist::build_netlist;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub struct NetRoutingMetrics {
    pub net_id: u32,
    pub name: String,
    pub length_mm: f64,
    pub top_length_mm: f64,
    pub bottom_length_mm: f64,
    pub top_tracks: usize,
    pub bottom_tracks: usize,
    pub vias: usize,
    pub pins: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoutingMetrics {
    pub nets: Vec<NetRoutingMetrics>,
}

/// Falha com código estável se não houver placa ou se uma âncora não resolver.
pub fn routing_metrics(project: &Project) -> Result<RoutingMetrics, String> {
    let board = project.board.as_ref().ok_or("NO_BOARD")?;
    let netlist = build_netlist(project);
    let mut nets: BTreeMap<_, _> = netlist
        .nets
        .iter()
        .map(|n| {
            (
                n.id,
                NetRoutingMetrics {
                    net_id: n.id.0,
                    name: n.name.clone(),
                    length_mm: 0.0,
                    top_length_mm: 0.0,
                    bottom_length_mm: 0.0,
                    top_tracks: 0,
                    bottom_tracks: 0,
                    vias: 0,
                    pins: n.pins.len(),
                },
            )
        })
        .collect();
    for track in board.tracks.values() {
        let net = netlist.net_for_pin(track.net).ok_or("UNRESOLVED_NET")?;
        let metrics = nets.get_mut(&net).ok_or("UNRESOLVED_NET")?;
        let dx = (track.end.x as f64) - (track.start.x as f64);
        let dy = (track.end.y as f64) - (track.start.y as f64);
        let length = dx.hypot(dy) / NM_PER_MM as f64;
        metrics.length_mm += length;
        match track.layer {
            BoardSide::Top => {
                metrics.top_tracks += 1;
                metrics.top_length_mm += length;
            }
            BoardSide::Bottom => {
                metrics.bottom_tracks += 1;
                metrics.bottom_length_mm += length;
            }
        }
    }
    for via in board.vias.values() {
        let net = netlist.net_for_pin(via.net).ok_or("UNRESOLVED_NET")?;
        nets.get_mut(&net).ok_or("UNRESOLVED_NET")?.vias += 1;
    }
    Ok(RoutingMetrics {
        nets: nets.into_values().collect(),
    })
}
