//! Remoção conservadora de trilhas exatamente sobrepostas.
use eda_core::{BoardSide, Command, Project};
use std::collections::BTreeMap;

/// Preserva a trilha de menor ID. Trilhas sem net derivada não são alteradas.
/// O lote precisa ser confirmado pelo Engine; a comparação de DRC ocorre antes da entrega.
pub fn plan(project: &Project) -> Result<Vec<Command>, String> {
    let board = project.board.as_ref().ok_or("PCB ausente")?;
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
            return Err("Limpeza criaria conexões pendentes.".into());
        }
    }
    Ok(commands)
}
