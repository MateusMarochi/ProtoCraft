use eda_core::{Command, ComponentId, ComponentKind, JunctionId, LabelId, ObjectId, Point, WireId};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(
    tag = "command",
    content = "params",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Action {
    PlaceDevice {
        device: String,
        position: Point,
    },
    AssignFootprint {
        id: u64,
        footprint: String,
    },
    SetBoard {
        board: eda_core::Board,
    },
    PlaceOnBoard {
        id: u64,
        placement: eda_core::Placement,
    },
    RemoveFromBoard {
        id: u64,
    },
    AddTrack {
        track: eda_core::Track,
    },
    UpdateTrack {
        track: eda_core::Track,
    },
    DeleteTrack {
        id: u64,
    },
    AddVia {
        via: eda_core::Via,
    },
    UpdateVia {
        via: eda_core::Via,
    },
    DeleteVia {
        id: u64,
    },
    AddHole {
        hole: eda_core::MountingHole,
    },
    UpdateHole {
        hole: eda_core::MountingHole,
    },
    DeleteHole {
        id: u64,
    },
    PlaceComponent {
        kind: ComponentKind,
        position: Point,
    },
    MoveComponent {
        id: u64,
        position: Point,
    },
    RotateComponent {
        id: u64,
    },
    SetComponent {
        id: u64,
        reference: String,
        value: f64,
    },
    SetModel {
        id: u64,
        parameters: eda_core::ModelParameters,
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
        object: ObjectRef,
    },
    RenameProject {
        name: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectRef {
    pub kind: ObjectKind,
    pub id: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectKind {
    Component,
    Wire,
    Junction,
    Label,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WrappedActions {
    actions: Vec<Action>,
}

impl Action {
    pub fn into_command(self) -> Result<Command, String> {
        Ok(match self {
            Self::PlaceDevice { device, position } => {
                return eda_library::catalog()?.place_command(&device, position);
            }
            Self::AssignFootprint { id, footprint } => Command::AssignFootprint {
                id: ComponentId(id),
                footprint: Box::new(
                    eda_library::catalog()?
                        .footprints
                        .get(&footprint)
                        .ok_or("Footprint ausente")?
                        .clone(),
                ),
            },
            Self::SetBoard { board } => Command::SetBoard { board },
            Self::PlaceOnBoard { id, placement } => Command::PlaceOnBoard {
                id: ComponentId(id),
                placement,
            },
            Self::RemoveFromBoard { id } => Command::RemoveFromBoard {
                id: ComponentId(id),
            },
            Self::AddTrack { track } => Command::AddTrack { track },
            Self::UpdateTrack { track } => Command::UpdateTrack { track },
            Self::DeleteTrack { id } => Command::DeleteTrack { id },
            Self::AddVia { via } => Command::AddVia { via },
            Self::UpdateVia { via } => Command::UpdateVia { via },
            Self::DeleteVia { id } => Command::DeleteVia { id },
            Self::AddHole { hole } => Command::AddHole { hole },
            Self::UpdateHole { hole } => Command::UpdateHole { hole },
            Self::DeleteHole { id } => Command::DeleteHole { id },
            Self::PlaceComponent { kind, position } => Command::PlaceComponent { kind, position },
            Self::MoveComponent { id, position } => Command::MoveComponent {
                id: ComponentId(id),
                position,
            },
            Self::RotateComponent { id } => Command::RotateComponent {
                id: ComponentId(id),
            },
            Self::SetComponent {
                id,
                reference,
                value,
            } => Command::SetComponent {
                id: ComponentId(id),
                reference,
                value,
            },
            Self::SetModel { id, parameters } => Command::SetModel {
                id: ComponentId(id),
                parameters,
            },
            Self::AddWire { start, end } => Command::AddWire { start, end },
            Self::AddJunction { position } => Command::AddJunction { position },
            Self::AddLabel { position, name } => Command::AddLabel { position, name },
            Self::Delete { object } => Command::Delete {
                object: match object.kind {
                    ObjectKind::Component => ObjectId::Component(ComponentId(object.id)),
                    ObjectKind::Wire => ObjectId::Wire(WireId(object.id)),
                    ObjectKind::Junction => ObjectId::Junction(JunctionId(object.id)),
                    ObjectKind::Label => ObjectId::Label(LabelId(object.id)),
                },
            },
            Self::RenameProject { name } => Command::RenameProject { name },
        })
    }
}

pub fn parse_actions(bytes: &[u8]) -> Result<Vec<Action>, serde_json::Error> {
    // Evita o buffer de enum untagged: ele perde a conversão de chaves JSON
    // para IDs numéricos quando set_board contém placements/tracks existentes.
    if bytes.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'[') {
        serde_json::from_slice(bytes)
    } else {
        serde_json::from_slice::<WrappedActions>(bytes).map(|wrapped| wrapped.actions)
    }
}

pub fn parse_action(command: &str, params: &str) -> Result<Action, Box<dyn std::error::Error>> {
    let params: serde_json::Value = serde_json::from_str(params)?;
    Ok(serde_json::from_value(serde_json::json!({
        "command": command,
        "params": params
    }))?)
}

pub struct CommandInfo {
    pub id: &'static str,
    pub description: &'static str,
    pub params: &'static str,
}

pub const COMMANDS: &[CommandInfo] = &[
    CommandInfo {
        id: "place_device",
        description: "Insere dispositivo do catálogo com pinagem e footprint incorporados.",
        params: "device, position",
    },
    CommandInfo {
        id: "assign_footprint",
        description: "Associa um footprint do catálogo a um componente.",
        params: "id, footprint",
    },
    CommandInfo {
        id: "set_board",
        description: "Define dimensões, regras e estado da PCB em nanômetros.",
        params: "board",
    },
    CommandInfo {
        id: "place_on_board",
        description: "Posiciona ou move um footprint na PCB.",
        params: "id, placement: {position:{x,y},rotation,side}",
    },
    CommandInfo {
        id: "remove_from_board",
        description: "Remove posicionamento e cobre referenciado pelo componente.",
        params: "id",
    },
    CommandInfo {
        id: "add_track",
        description: "Adiciona trilha com rede referenciada por pino estável.",
        params: "track: {id,start,end,width_nm,layer,net:{component,pin}}",
    },
    CommandInfo {
        id: "delete_track",
        description: "Exclui trilha por ID.",
        params: "id",
    },
    CommandInfo {
        id: "add_via",
        description: "Adiciona via entre as duas camadas.",
        params: "via: {id,position,diameter_nm,drill_nm,net}",
    },
    CommandInfo {
        id: "delete_via",
        description: "Exclui via por ID.",
        params: "id",
    },
    CommandInfo {
        id: "update_track",
        description: "Edita geometria, largura e camada de uma trilha existente.",
        params: "track: {id,start,end,width_nm,layer,net}",
    },
    CommandInfo {
        id: "update_via",
        description: "Edita posição, diâmetro e furação de uma via existente.",
        params: "via: {id,position,diameter_nm,drill_nm,net}",
    },
    CommandInfo {
        id: "add_hole",
        description: "Adiciona furo mecânico circular não metalizado (NPTH).",
        params: "hole: {id,position:{x,y},drill_nm}",
    },
    CommandInfo {
        id: "update_hole",
        description: "Edita posição e diâmetro de um furo mecânico existente.",
        params: "hole: {id,position:{x,y},drill_nm}",
    },
    CommandInfo {
        id: "delete_hole",
        description: "Exclui furo mecânico por ID.",
        params: "id",
    },
    CommandInfo {
        id: "set_model",
        description: "Edita parâmetros de forma de onda, chave, potenciômetro e modelos genéricos.",
        params: "id, parameters: {frequency_hz, offset_v, phase_deg, duty_cycle, closed, wiper, ...}",
    },
    CommandInfo {
        id: "place_component",
        description: "Insere um componente básico.",
        params: "kind, position",
    },
    CommandInfo {
        id: "move_component",
        description: "Move um componente sem mover os fios.",
        params: "id, position",
    },
    CommandInfo {
        id: "rotate_component",
        description: "Gira um componente em 90 graus.",
        params: "id",
    },
    CommandInfo {
        id: "set_component",
        description: "Altera referência e valor elétrico.",
        params: "id, reference, value",
    },
    CommandInfo {
        id: "add_wire",
        description: "Adiciona um segmento de fio.",
        params: "start, end",
    },
    CommandInfo {
        id: "add_junction",
        description: "Adiciona uma junção elétrica.",
        params: "position",
    },
    CommandInfo {
        id: "add_label",
        description: "Adiciona um nome de rede.",
        params: "position, name",
    },
    CommandInfo {
        id: "delete",
        description: "Exclui um objeto por tipo e ID.",
        params: "object: {kind, id}",
    },
    CommandInfo {
        id: "rename_project",
        description: "Altera o nome do projeto.",
        params: "name",
    },
];
