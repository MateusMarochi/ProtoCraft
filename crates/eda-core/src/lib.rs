//! Domínio independente de interface e operações transacionais do ProtoCraft.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub mod examples;
pub const FORMAT_VERSION: u32 = 6;
pub const MAX_OBJECTS: usize = 20_000;
const COORD_LIMIT: i32 = 1_000_000;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub u64);
    };
}
id_type!(ComponentId);
id_type!(WireId);
id_type!(JunctionId);
id_type!(LabelId);

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}
impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
    pub fn is_valid(self) -> bool {
        self.x.abs_diff(0) <= COORD_LIMIT as u32 && self.y.abs_diff(0) <= COORD_LIMIT as u32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PinRef {
    pub component: ComponentId,
    pub pin: u16,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pin {
    pub reference: PinRef,
    pub position: Point,
}

pub mod physical;
pub use physical::*;
mod component;
pub use component::{ComponentKind, ModelParameters};

/// Metadados e geometria elétrica próprios; nenhuma primitiva gráfica externa é incorporada.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedPin {
    pub number: String,
    pub name: String,
    pub offset: Point,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedComponent {
    pub source_format: String,
    pub device: String,
    pub value_text: String,
    pub properties: BTreeMap<String, String>,
    pub pins: Vec<ImportedPin>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    pub id: ComponentId,
    pub reference: String,
    pub kind: ComponentKind,
    pub value: f64,
    pub position: Point,
    pub rotation: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported: Option<ImportedComponent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<Box<DeviceDefinition>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footprint: Option<Box<Footprint>>,
    #[serde(default, skip_serializing_if = "ModelParameters::is_default")]
    pub parameters: ModelParameters,
}
impl Component {
    pub fn pins(&self) -> Vec<Pin> {
        if let Some(device) = &self.device {
            return device
                .pins
                .iter()
                .enumerate()
                .map(|(index, pin)| {
                    let (mut x, mut y) = (pin.offset.x, pin.offset.y);
                    for _ in 0..self.rotation {
                        (x, y) = (-y, x);
                    }
                    Pin {
                        reference: PinRef {
                            component: self.id,
                            pin: index as u16,
                        },
                        position: Point::new(
                            self.position.x.saturating_add(x),
                            self.position.y.saturating_add(y),
                        ),
                    }
                })
                .collect();
        }
        if let Some(imported) = &self.imported {
            return imported
                .pins
                .iter()
                .enumerate()
                .map(|(index, pin)| {
                    let (mut x, mut y) = (pin.offset.x, pin.offset.y);
                    for _ in 0..self.rotation % 4 {
                        (x, y) = (y.saturating_neg(), x);
                    }
                    Pin {
                        reference: PinRef {
                            component: self.id,
                            pin: index as u16,
                        },
                        position: Point::new(
                            self.position.x.saturating_add(x),
                            self.position.y.saturating_add(y),
                        ),
                    }
                })
                .collect();
        }
        (0..self.kind.pin_count())
            .map(|pin| {
                let (mut x, mut y) = self.kind.pin_offset(pin);
                for _ in 0..self.rotation % 4 {
                    (x, y) = (-y, x);
                }
                Pin {
                    reference: PinRef {
                        component: self.id,
                        pin: u16::from(pin),
                    },
                    position: Point::new(
                        self.position.x.saturating_add(x),
                        self.position.y.saturating_add(y),
                    ),
                }
            })
            .collect()
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wire {
    pub id: WireId,
    pub start: Point,
    pub end: Point,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Junction {
    pub id: JunctionId,
    pub position: Point,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetLabel {
    pub id: LabelId,
    pub position: Point,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedProjectInfo {
    pub source_format: String,
    pub electrical_complete: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub format_version: u32,
    pub name: String,
    pub components: BTreeMap<ComponentId, Component>,
    pub wires: BTreeMap<WireId, Wire>,
    pub junctions: BTreeMap<JunctionId, Junction>,
    pub labels: BTreeMap<LabelId, NetLabel>,
    pub next_id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import_info: Option<ImportedProjectInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<Box<Board>>,
}

#[derive(Debug, Error, PartialEq)]
pub enum CoreError {
    #[error("Versão de projeto não suportada: {0}.")]
    UnsupportedVersion(u32),
    #[error("Projeto inválido: {0}")]
    Invalid(String),
    #[error("Objeto não encontrado.")]
    MissingObject,
    #[error("Limite de objetos ou identificadores atingido.")]
    Limit,
}
impl Project {
    pub fn empty(name: impl Into<String>) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            name: name.into(),
            components: BTreeMap::new(),
            wires: BTreeMap::new(),
            junctions: BTreeMap::new(),
            labels: BTreeMap::new(),
            next_id: 1,
            import_info: None,
            board: None,
        }
    }
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.format_version != FORMAT_VERSION {
            return Err(CoreError::UnsupportedVersion(self.format_version));
        }
        if self.name.trim().is_empty() || self.name.len() > 256 {
            return Err(CoreError::Invalid(
                "nome deve ter entre 1 e 256 bytes.".into(),
            ));
        }
        if self.import_info.as_ref().is_some_and(|info| {
            info.source_format.len() > 64
                || info.notes.len() > 256
                || info.notes.iter().any(|note| note.len() > 4096)
        }) {
            return Err(CoreError::Invalid(
                "relatório de importação excede os limites.".into(),
            ));
        }
        if self.components.len() + self.wires.len() + self.junctions.len() + self.labels.len()
            > MAX_OBJECTS
        {
            return Err(CoreError::Limit);
        }
        if let Some(board) = &self.board {
            board.validate(self)?;
        }
        if self
            .components
            .values()
            .map(|c| c.pins().len())
            .sum::<usize>()
            > 100_000
        {
            return Err(CoreError::Limit);
        }
        let mut ids = BTreeSet::new();
        let mut refs = BTreeSet::new();
        for (id, c) in &self.components {
            if *id != c.id || id.0 == 0 || !ids.insert(id.0) {
                return Err(CoreError::Invalid("ID de componente inconsistente.".into()));
            }
            if c.reference.is_empty()
                || c.reference.len() > 64
                || !c
                    .reference
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_')
                || !refs.insert(c.reference.to_ascii_uppercase())
            {
                return Err(CoreError::Invalid(
                    "referências devem ser únicas e conter letras, números ou sublinhado.".into(),
                ));
            }
            if !c.value.is_finite()
                || c.value.abs() > 1e15
                || (c.kind.positive_value() && c.value <= 0.0)
                || !c.parameters.is_valid()
                || (c.kind == ComponentKind::Counter10 && c.parameters.initial_state >= 10)
                || ((matches!(
                    c.kind,
                    ComponentKind::Timer555
                        | ComponentKind::DFlipFlop
                        | ComponentKind::Comparator
                        | ComponentKind::Schmitt
                ) || c.kind.is_gate())
                    && c.parameters.initial_state > 1)
            {
                return Err(CoreError::Invalid(format!(
                    "valor inválido em {}.",
                    c.reference
                )));
            }
            physical::validate_component(c)?;
            if c.kind == ComponentKind::Imported && c.imported.is_none() {
                return Err(CoreError::Invalid(
                    "componente importado sem descrição de pinos.".into(),
                ));
            }
            if let Some(imported) = &c.imported {
                let mut numbers = BTreeSet::new();
                if imported.source_format.len() > 64
                    || imported.device.len() > 256
                    || imported.value_text.len() > 256
                    || imported.properties.len() > 128
                    || imported
                        .properties
                        .iter()
                        .any(|(k, v)| k.len() > 128 || v.len() > 4096)
                    || imported.pins.len() > 64
                    || (c.kind != ComponentKind::Imported
                        && imported.pins.len() != c.kind.pin_count() as usize)
                    || imported.pins.iter().any(|pin| {
                        pin.number.is_empty()
                            || pin.number.len() > 32
                            || pin.name.len() > 64
                            || !pin.offset.is_valid()
                            || !numbers.insert(&pin.number)
                    })
                {
                    return Err(CoreError::Invalid(format!(
                        "metadados ou pinos importados inválidos em {}.",
                        c.reference
                    )));
                }
            }
            if c.rotation > 3
                || !c.position.is_valid()
                || c.pins().iter().any(|p| !p.position.is_valid())
            {
                return Err(CoreError::Invalid("posição ou rotação inválida.".into()));
            }
        }
        for (id, w) in &self.wires {
            if *id != w.id || id.0 == 0 || !ids.insert(id.0) {
                return Err(CoreError::Invalid("ID de fio inconsistente.".into()));
            }
            if !w.start.is_valid()
                || !w.end.is_valid()
                || w.start == w.end
                || (w.start.x != w.end.x && w.start.y != w.end.y)
            {
                return Err(CoreError::Invalid(
                    "fios precisam ser ortogonais, não nulos e dentro da grade.".into(),
                ));
            }
        }
        for (id, j) in &self.junctions {
            if *id != j.id || id.0 == 0 || !ids.insert(id.0) || !j.position.is_valid() {
                return Err(CoreError::Invalid("junção inválida.".into()));
            }
        }
        for (id, l) in &self.labels {
            if *id != l.id
                || id.0 == 0
                || !ids.insert(id.0)
                || !l.position.is_valid()
                || l.name.trim() != l.name
                || l.name.is_empty()
                || l.name.len() > 64
                || l.name.chars().any(char::is_control)
            {
                return Err(CoreError::Invalid(
                    "nome ou posição de rede inválida.".into(),
                ));
            }
        }
        if self.next_id == 0 || ids.last().is_some_and(|id| self.next_id <= *id) {
            return Err(CoreError::Invalid(
                "próximo identificador não é único.".into(),
            ));
        }
        Ok(())
    }
    fn allocate(&mut self) -> Result<u64, CoreError> {
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1).ok_or(CoreError::Limit)?;
        Ok(id)
    }
    fn apply(&mut self, command: Command, depth: usize) -> Result<(), CoreError> {
        if depth > 32 {
            return Err(CoreError::Limit);
        }
        match command {
            Command::PlaceDevice {
                device,
                footprint,
                position,
            } => {
                let id = ComponentId(self.allocate()?);
                let mut suffix = 1;
                while self
                    .components
                    .values()
                    .any(|c| c.reference.eq_ignore_ascii_case(&format!("U{suffix}")))
                {
                    suffix += 1;
                }
                self.components.insert(
                    id,
                    Component {
                        id,
                        reference: format!("U{suffix}"),
                        kind: ComponentKind::Device,
                        value: 0.0,
                        position,
                        rotation: 0,
                        imported: None,
                        parameters: Default::default(),
                        device: Some(device),
                        footprint: Some(footprint),
                    },
                );
            }
            Command::AssignFootprint { id, footprint } => {
                self.components
                    .get_mut(&id)
                    .ok_or(CoreError::MissingObject)?
                    .footprint = Some(footprint);
            }
            Command::SetBoard { board } => self.board = Some(Box::new(board)),
            Command::PlaceOnBoard { id, placement } => {
                self.board
                    .as_mut()
                    .ok_or_else(|| {
                        CoreError::Invalid("Crie a PCB antes de posicionar componentes.".into())
                    })?
                    .placements
                    .insert(id, placement);
            }
            Command::RemoveFromBoard { id } => {
                if let Some(b) = &mut self.board {
                    b.placements.remove(&id);
                    b.tracks.retain(|_, t| t.net.component != id);
                    b.vias.retain(|_, v| v.net.component != id);
                }
            }
            Command::AddTrack { track } => {
                let b = self.board.as_mut().ok_or(CoreError::MissingObject)?;
                if b.tracks.contains_key(&track.id) {
                    return Err(CoreError::Invalid("ID de trilha duplicado.".into()));
                }
                b.tracks.insert(track.id, track);
            }
            Command::UpdateTrack { track } => {
                let b = self.board.as_mut().ok_or(CoreError::MissingObject)?;
                if !b.tracks.contains_key(&track.id) {
                    return Err(CoreError::MissingObject);
                }
                b.tracks.insert(track.id, track);
            }
            Command::DeleteTrack { id } => {
                if self
                    .board
                    .as_mut()
                    .ok_or(CoreError::MissingObject)?
                    .tracks
                    .remove(&id)
                    .is_none()
                {
                    return Err(CoreError::MissingObject);
                }
            }
            Command::AddVia { via } => {
                let b = self.board.as_mut().ok_or(CoreError::MissingObject)?;
                if b.vias.contains_key(&via.id) {
                    return Err(CoreError::Invalid("ID de via duplicado.".into()));
                }
                b.vias.insert(via.id, via);
            }
            Command::UpdateVia { via } => {
                let b = self.board.as_mut().ok_or(CoreError::MissingObject)?;
                if !b.vias.contains_key(&via.id) {
                    return Err(CoreError::MissingObject);
                }
                b.vias.insert(via.id, via);
            }
            Command::AddHole { hole } => {
                let b = self.board.as_mut().ok_or(CoreError::MissingObject)?;
                if b.holes.contains_key(&hole.id) {
                    return Err(CoreError::Invalid("ID de furo duplicado.".into()));
                }
                b.holes.insert(hole.id, hole);
            }
            Command::UpdateHole { hole } => {
                let b = self.board.as_mut().ok_or(CoreError::MissingObject)?;
                if !b.holes.contains_key(&hole.id) {
                    return Err(CoreError::MissingObject);
                }
                b.holes.insert(hole.id, hole);
            }
            Command::DeleteHole { id } => {
                if self
                    .board
                    .as_mut()
                    .ok_or(CoreError::MissingObject)?
                    .holes
                    .remove(&id)
                    .is_none()
                {
                    return Err(CoreError::MissingObject);
                }
            }
            Command::DeleteVia { id } => {
                if self
                    .board
                    .as_mut()
                    .ok_or(CoreError::MissingObject)?
                    .vias
                    .remove(&id)
                    .is_none()
                {
                    return Err(CoreError::MissingObject);
                }
            }
            Command::PlaceComponent { kind, position } => {
                let id = ComponentId(self.allocate()?);
                let mut n = 1;
                while self.components.values().any(|c| {
                    c.reference
                        .eq_ignore_ascii_case(&format!("{}{n}", kind.prefix()))
                }) {
                    n += 1;
                }
                self.components.insert(
                    id,
                    Component {
                        id,
                        reference: format!("{}{n}", kind.prefix()),
                        kind,
                        value: kind.default_value(),
                        position,
                        rotation: 0,
                        parameters: Default::default(),
                        device: None,
                        footprint: None,
                        imported: None,
                    },
                );
            }
            Command::PasteComponent {
                mut component,
                position,
            } => {
                let id = ComponentId(self.allocate()?);
                let mut suffix = 1;
                while self.components.values().any(|existing| {
                    existing
                        .reference
                        .eq_ignore_ascii_case(&format!("{}{suffix}", component.kind.prefix()))
                }) {
                    suffix += 1;
                }
                component.id = id;
                component.reference = format!("{}{suffix}", component.kind.prefix());
                component.position = position;
                self.components.insert(id, component);
            }
            Command::RestoreComponent { component } => {
                if component.id.0 == 0
                    || component.id.0 >= self.next_id
                    || self.components.contains_key(&component.id)
                {
                    return Err(CoreError::Invalid(
                        "Não é possível restaurar o ID deste componente.".into(),
                    ));
                }
                self.components.insert(component.id, *component);
            }
            Command::MoveComponent { id, position } => {
                self.components
                    .get_mut(&id)
                    .ok_or(CoreError::MissingObject)?
                    .position = position
            }
            Command::RotateComponent { id } => {
                let c = self
                    .components
                    .get_mut(&id)
                    .ok_or(CoreError::MissingObject)?;
                c.rotation = (c.rotation + 1) % 4;
            }
            Command::SetComponent {
                id,
                reference,
                value,
            } => {
                let c = self
                    .components
                    .get_mut(&id)
                    .ok_or(CoreError::MissingObject)?;
                c.reference = reference;
                c.value = value;
            }
            Command::SetModel { id, parameters } => {
                self.components
                    .get_mut(&id)
                    .ok_or(CoreError::MissingObject)?
                    .parameters = parameters;
            }
            Command::AddWire { start, end } => {
                let id = WireId(self.allocate()?);
                self.wires.insert(id, Wire { id, start, end });
            }
            Command::AddJunction { position } => {
                let id = JunctionId(self.allocate()?);
                self.junctions.insert(id, Junction { id, position });
            }
            Command::AddLabel { position, name } => {
                let id = LabelId(self.allocate()?);
                self.labels.insert(id, NetLabel { id, position, name });
            }
            Command::Delete { object } => {
                if let ObjectId::Component(id) = object
                    && let Some(b) = &mut self.board
                {
                    b.placements.remove(&id);
                    b.tracks.retain(|_, t| t.net.component != id);
                    b.vias.retain(|_, v| v.net.component != id);
                }
                let existed = match object {
                    ObjectId::Component(id) => self.components.remove(&id).is_some(),
                    ObjectId::Wire(id) => self.wires.remove(&id).is_some(),
                    ObjectId::Junction(id) => self.junctions.remove(&id).is_some(),
                    ObjectId::Label(id) => self.labels.remove(&id).is_some(),
                };
                if !existed {
                    return Err(CoreError::MissingObject);
                }
            }
            Command::RenameProject { name } => self.name = name,
            Command::Batch(commands) => {
                if commands.len() > MAX_OBJECTS {
                    return Err(CoreError::Limit);
                }
                for c in commands {
                    self.apply(c, depth + 1)?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectId {
    Component(ComponentId),
    Wire(WireId),
    Junction(JunctionId),
    Label(LabelId),
}
#[derive(Debug, Clone)]
pub enum Command {
    PlaceDevice {
        device: Box<DeviceDefinition>,
        footprint: Box<Footprint>,
        position: Point,
    },
    AssignFootprint {
        id: ComponentId,
        footprint: Box<Footprint>,
    },
    SetBoard {
        board: Board,
    },
    PlaceOnBoard {
        id: ComponentId,
        placement: Placement,
    },
    RemoveFromBoard {
        id: ComponentId,
    },
    AddTrack {
        track: Track,
    },
    UpdateTrack {
        track: Track,
    },
    DeleteTrack {
        id: u64,
    },
    AddVia {
        via: Via,
    },
    UpdateVia {
        via: Via,
    },
    DeleteVia {
        id: u64,
    },
    AddHole {
        hole: MountingHole,
    },
    UpdateHole {
        hole: MountingHole,
    },
    DeleteHole {
        id: u64,
    },
    PlaceComponent {
        kind: ComponentKind,
        position: Point,
    },
    PasteComponent {
        component: Component,
        position: Point,
    },
    RestoreComponent {
        component: Box<Component>,
    },
    MoveComponent {
        id: ComponentId,
        position: Point,
    },
    RotateComponent {
        id: ComponentId,
    },
    SetComponent {
        id: ComponentId,
        reference: String,
        value: f64,
    },
    SetModel {
        id: ComponentId,
        parameters: ModelParameters,
    },
    AddWire {
        start: Point,
        end: Point,
    },
    AddJunction {
        position: Point,
    },
    AddLabel {
        position: Point,
        name: String,
    },
    Delete {
        object: ObjectId,
    },
    RenameProject {
        name: String,
    },
    Batch(Vec<Command>),
}

pub struct Engine {
    project: Project,
    revision: u64,
    undo: Vec<Project>,
    redo: Vec<Project>,
}
impl Engine {
    pub fn new(project: Project) -> Result<Self, CoreError> {
        project.validate()?;
        Ok(Self {
            project,
            revision: 0,
            undo: Vec::new(),
            redo: Vec::new(),
        })
    }
    pub fn project(&self) -> &Project {
        &self.project
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn execute(&mut self, command: Command) -> Result<(), CoreError> {
        // O snapshot só é criado ao confirmar uma edição, nunca durante desenho/hover.
        let mut candidate = self.project.clone();
        candidate.apply(command, 0)?;
        candidate.validate()?;
        if candidate != self.project {
            self.undo
                .push(std::mem::replace(&mut self.project, candidate));
            if self.undo.len() > 100 {
                self.undo.remove(0);
            }
            self.redo.clear();
            self.revision = self.revision.saturating_add(1);
        }
        Ok(())
    }
    pub fn undo(&mut self) -> bool {
        if let Some(project) = self.undo.pop() {
            self.redo
                .push(std::mem::replace(&mut self.project, project));
            self.revision = self.revision.saturating_add(1);
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self) -> bool {
        if let Some(project) = self.redo.pop() {
            self.undo
                .push(std::mem::replace(&mut self.project, project));
            self.revision = self.revision.saturating_add(1);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pasted_component_gets_fresh_id_reference_and_undoable_position() {
        let mut engine = Engine::new(Project::empty("Colar")).unwrap();
        engine
            .execute(Command::PlaceComponent {
                kind: ComponentKind::Resistor,
                position: Point::new(2, 3),
            })
            .unwrap();
        engine
            .execute(Command::SetComponent {
                id: ComponentId(1),
                reference: "R1".into(),
                value: 2_200.0,
            })
            .unwrap();
        engine
            .execute(Command::RotateComponent { id: ComponentId(1) })
            .unwrap();

        let mut clipboard = engine.project().components[&ComponentId(1)].clone();
        clipboard.id = ComponentId(99);
        clipboard.reference = "R1".into();
        engine
            .execute(Command::PasteComponent {
                component: clipboard,
                position: Point::new(20, 30),
            })
            .unwrap();

        let pasted = &engine.project().components[&ComponentId(2)];
        assert_eq!(pasted.id, ComponentId(2));
        assert_eq!(pasted.reference, "R2");
        assert_eq!(pasted.value, 2_200.0);
        assert_eq!(pasted.rotation, 1);
        assert_eq!(pasted.position, Point::new(20, 30));
        assert!(engine.undo());
        assert_eq!(engine.project().components.len(), 1);
        assert!(engine.redo());
        assert_eq!(engine.project().components.len(), 2);
    }

    #[test]
    fn custom_pins_rotate_and_invalid_metadata_is_rejected_atomically() {
        let mut p = Project::empty("Importado");
        let id = ComponentId(1);
        p.next_id = 2;
        p.components.insert(
            id,
            Component {
                id,
                reference: "Q1".into(),
                kind: ComponentKind::Imported,
                value: 0.0,
                position: Point::new(10, 10),
                rotation: 0,
                parameters: Default::default(),
                device: None,
                footprint: None,
                imported: Some(ImportedComponent {
                    source_format: "teste".into(),
                    device: "MOS_TEST".into(),
                    value_text: "MOS".into(),
                    properties: BTreeMap::new(),
                    pins: vec![
                        ImportedPin {
                            number: "2".into(),
                            name: "D".into(),
                            offset: Point::new(5, -3),
                        },
                        ImportedPin {
                            number: "1".into(),
                            name: "G".into(),
                            offset: Point::new(-4, 0),
                        },
                        ImportedPin {
                            number: "3".into(),
                            name: "S".into(),
                            offset: Point::new(5, 3),
                        },
                    ],
                }),
            },
        );
        let mut engine = Engine::new(p).unwrap();
        let original = engine.project().clone();
        engine.execute(Command::RotateComponent { id }).unwrap();
        assert_eq!(
            engine.project().components[&id].pins()[0].position,
            Point::new(13, 15)
        );
        engine.undo();
        assert_eq!(*engine.project(), original);
        let mut bad = original;
        let imported = bad
            .components
            .get_mut(&id)
            .unwrap()
            .imported
            .as_mut()
            .unwrap();
        imported.pins[1].number = "2".into();
        assert!(bad.validate().is_err());
    }
    #[test]
    fn invalid_batch_is_atomic() {
        let mut e = Engine::new(Project::empty("teste")).unwrap();
        let original = e.project().clone();
        assert!(
            e.execute(Command::Batch(vec![
                Command::PlaceComponent {
                    kind: ComponentKind::Resistor,
                    position: Point::new(0, 0)
                },
                Command::AddWire {
                    start: Point::new(0, 0),
                    end: Point::new(1, 1)
                }
            ]))
            .is_err()
        );
        assert_eq!(*e.project(), original);
        assert_eq!(e.revision(), 0);
        assert!(!e.can_undo());
    }
    #[test]
    fn undo_redo_preserves_ids() {
        let mut e = Engine::new(Project::empty("teste")).unwrap();
        e.execute(Command::PlaceComponent {
            kind: ComponentKind::Resistor,
            position: Point::new(0, 0),
        })
        .unwrap();
        let p = e.project().clone();
        for _ in 0..10 {
            assert!(e.undo());
            assert!(e.redo());
            assert_eq!(*e.project(), p);
        }
        assert_eq!(e.revision(), 21);
    }
    #[test]
    fn bad_value_and_duplicate_rejected() {
        let mut e = Engine::new(examples::voltage_divider()).unwrap();
        let c = e.project().components.values().next().unwrap().clone();
        let p = e.project().clone();
        assert!(
            e.execute(Command::SetComponent {
                id: c.id,
                reference: c.reference,
                value: f64::NAN
            })
            .is_err()
        );
        assert_eq!(*e.project(), p);
    }
    #[test]
    fn rotation_preserves_pin_numbers() {
        let c = Component {
            id: ComponentId(1),
            reference: "R1".into(),
            kind: ComponentKind::Resistor,
            value: 1.0,
            position: Point::new(0, 0),
            rotation: 1,
            parameters: Default::default(),
            device: None,
            footprint: None,
            imported: None,
        };
        assert_eq!(c.pins()[0].position, Point::new(0, -3));
        assert_eq!(c.pins()[1].reference.pin, 1);
    }
    #[test]
    fn deletion_and_replay() {
        let mut e = Engine::new(examples::voltage_divider()).unwrap();
        let id = *e.project().components.keys().next().unwrap();
        e.execute(Command::Delete {
            object: ObjectId::Component(id),
        })
        .unwrap();
        assert!(!e.project().components.contains_key(&id));
        assert!(e.undo());
        assert!(e.project().components.contains_key(&id));
        assert!(e.redo());
        assert!(e.project().validate().is_ok());
    }

    #[test]
    fn duplicate_reference_does_not_change_state_or_history() {
        let mut e = Engine::new(examples::voltage_divider()).unwrap();
        let id = e
            .project()
            .components
            .values()
            .find(|c| c.reference == "R2")
            .unwrap()
            .id;
        let before = e.project().clone();
        assert!(
            e.execute(Command::SetComponent {
                id,
                reference: "r1".into(),
                value: 2200.0
            })
            .is_err()
        );
        assert_eq!(*e.project(), before);
        assert!(!e.can_undo());
    }

    #[test]
    fn exhausted_id_and_outside_grid_rejected_atomically() {
        let mut p = Project::empty("Limites");
        p.next_id = u64::MAX;
        let mut e = Engine::new(p).unwrap();
        let before = e.project().clone();
        assert!(
            e.execute(Command::PlaceComponent {
                kind: ComponentKind::Ground,
                position: Point::new(0, 0)
            })
            .is_err()
        );
        assert_eq!(*e.project(), before);
        let mut e = Engine::new(Project::empty("Coordenadas")).unwrap();
        assert!(
            e.execute(Command::PlaceComponent {
                kind: ComponentKind::Ground,
                position: Point::new(i32::MIN, 0)
            })
            .is_err()
        );
        assert!(e.project().components.is_empty());
    }
    #[test]
    fn model_edits_validate_atomically_and_undo_restores_parameters() {
        let mut engine = Engine::new(examples::rc_filter()).unwrap();
        let id = ComponentId(1);
        let mut parameters = ModelParameters {
            frequency_hz: 2000.0,
            ..Default::default()
        };
        engine
            .execute(Command::SetModel {
                id,
                parameters: parameters.clone(),
            })
            .unwrap();
        parameters.duty_cycle = f64::NAN;
        assert!(
            engine
                .execute(Command::SetModel { id, parameters })
                .is_err()
        );
        assert_eq!(
            engine.project().components[&id].parameters.frequency_hz,
            2000.0
        );
        assert!(engine.undo());
        assert!(engine.project().components[&id].parameters.is_default());
        assert!(engine.redo());
        assert_eq!(
            engine.project().components[&id].parameters.frequency_hz,
            2000.0
        );
    }
}
