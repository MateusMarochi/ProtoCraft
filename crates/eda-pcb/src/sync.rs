//! Planeja a inclusão de footprints ainda não posicionados sem modificar cobre existente.
use eda_core::{
    Board, BoardSide, Command, ComponentId, CoreError, NM_PER_MM, NmPoint, Placement, Project,
};

pub struct SyncPlan {
    pub commands: Vec<Command>,
    pub added: Vec<ComponentId>,
    pub without_footprint: Vec<ComponentId>,
}

/// Organiza novos componentes em linhas abaixo dos corpos já posicionados.
/// O chamador aplica todos os comandos como lote atômico e deve executar o DRC.
pub fn plan(project: &Project) -> Result<SyncPlan, CoreError> {
    project.validate()?;
    let board = project.board.as_deref().cloned().unwrap_or_default();
    let mut commands = Vec::new();
    if project.board.is_none() {
        commands.push(Command::SetBoard {
            board: Board::default(),
        });
    }
    let gap = 5 * NM_PER_MM;
    let mut bottom = gap;
    for (id, placement) in &board.placements {
        if let Some(footprint) = project
            .components
            .get(id)
            .and_then(|c| c.footprint.as_ref())
        {
            let bounds = &footprint.body_bounds;
            for x in [bounds.min.x, bounds.max.x] {
                for y in [bounds.min.y, bounds.max.y] {
                    bottom = bottom.max(placement.transform(NmPoint::new(x, y)).y + gap);
                }
            }
        }
    }
    let (mut x, mut y, mut row_height) = (gap, bottom, 0);
    let mut added = Vec::new();
    let mut without_footprint = Vec::new();
    for component in project.components.values() {
        let Some(footprint) = &component.footprint else {
            without_footprint.push(component.id);
            continue;
        };
        if board.placements.contains_key(&component.id) {
            continue;
        }
        let bounds = &footprint.body_bounds;
        let (width, height) = (bounds.width(), bounds.height());
        if x + width + gap > board.width_nm {
            x = gap;
            y += row_height + gap;
            row_height = 0;
        }
        if x + width + gap > board.width_nm || y + height + gap > board.height_nm {
            return Err(CoreError::Invalid(
                "Espaço insuficiente na PCB para posicionar os footprints.".into(),
            ));
        }
        commands.push(Command::PlaceOnBoard {
            id: component.id,
            placement: Placement {
                position: NmPoint::new(x - bounds.min.x, y - bounds.min.y),
                rotation: 0,
                side: BoardSide::Top,
            },
        });
        added.push(component.id);
        x += width + gap;
        row_height = row_height.max(height);
    }
    Ok(SyncPlan {
        commands,
        added,
        without_footprint,
    })
}
