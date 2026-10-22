use super::*;
use eda_core::{ComponentId, Placement, Track, Via};
use serde::{Deserialize, Serialize};

const PREFIX: &str = "PROTOCRAFT_COMPONENT_V1\n";
const MAX_BYTES: usize = eda_storage::MAX_FILE_BYTES as usize;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    token: String,
    component: Component,
    placement: Option<Placement>,
}

#[cfg(test)]
mod tests;

#[derive(Clone)]
struct Cut {
    payload: Payload,
    pcb_only: bool,
    tracks: Vec<Track>,
    vias: Vec<Via>,
}
#[derive(Default)]
pub(super) struct ClipboardState {
    payload: Option<Payload>,
    count: usize,
    cut: Option<Cut>,
}
impl ClipboardState {
    pub(super) fn discard_cut(&mut self) {
        self.cut = None;
    }
}
#[derive(Clone)]
enum Action {
    Copy,
    Cut,
    Paste(Option<String>),
}

fn encode(payload: &Payload) -> Result<String, String> {
    let text = serde_json::to_string(payload).map_err(|e| e.to_string())?;
    if text.len() + PREFIX.len() > MAX_BYTES {
        return Err("Componente excede o limite de 8 MiB do clipboard.".into());
    }
    Ok(format!("{PREFIX}{text}"))
}
fn decode(text: &str) -> Result<Payload, String> {
    if text.len() > MAX_BYTES {
        return Err("Área de transferência excede 8 MiB.".into());
    }
    let json = text
        .strip_prefix(PREFIX)
        .ok_or("A área de transferência não contém um componente do ProtoCraft.")?;
    let payload: Payload =
        serde_json::from_str(json).map_err(|e| format!("Componente inválido no clipboard: {e}"))?;
    if payload.token.is_empty() || payload.token.len() > 128 {
        return Err("Identificador do clipboard inválido.".into());
    }
    // Valida o snapshot externo pelos mesmos contratos usados em uma colagem.
    let mut engine = Engine::new(Project::empty("Clipboard")).map_err(|e| e.to_string())?;
    engine
        .execute(Command::PasteComponent {
            component: payload.component.clone(),
            position: payload.component.position,
        })
        .map_err(|e| e.to_string())?;
    if payload
        .placement
        .is_some_and(|p| !p.position.valid() || p.rotation > 3)
    {
        return Err("Posicionamento inválido no clipboard.".into());
    }
    Ok(payload)
}
impl ProtoCraft {
    pub(crate) fn clipboard_selection(&self) -> Option<ComponentId> {
        let id = if self.pcb.active {
            self.pcb.selected
        } else {
            match self.selected {
                Some(ObjectId::Component(id)) => Some(id),
                _ => None,
            }
        }?;
        self.engine
            .project()
            .components
            .contains_key(&id)
            .then_some(id)
    }
    pub(crate) fn clipboard_can_cut(&self) -> bool {
        self.clipboard_selection().is_some_and(|id| {
            !self.pcb.active
                || self
                    .engine
                    .project()
                    .board
                    .as_ref()
                    .is_some_and(|b| b.placements.contains_key(&id))
        })
    }
    fn selected_payload(&self) -> Result<Payload, String> {
        let id = self
            .clipboard_selection()
            .ok_or("Selecione um componente para copiar ou recortar.")?;
        static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let token = format!(
            "{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        Ok(Payload {
            token,
            component: self.engine.project().components[&id].clone(),
            placement: self
                .engine
                .project()
                .board
                .as_ref()
                .and_then(|b| b.placements.get(&id))
                .copied(),
        })
    }
    pub(crate) fn copy_component(&mut self, ctx: &egui::Context) {
        match self
            .selected_payload()
            .and_then(|p| encode(&p).map(|text| (p, text)))
        {
            Ok((payload, text)) => {
                self.clipboard = ClipboardState {
                    payload: Some(payload),
                    ..Default::default()
                };
                ctx.copy_text(text);
                self.message("Componente copiado. Use Ctrl+V para colar.", false);
            }
            Err(e) => self.message(e, true),
        }
    }
    pub(crate) fn cut_component(&mut self, ctx: &egui::Context) {
        let result: Result<(), String> = (|| {
            let payload = self.selected_payload()?;
            if self.pcb.active && payload.placement.is_none() {
                return Err("Selecione um footprint posicionado para recortar na PCB.".into());
            }
            let text = encode(&payload)?;
            let id = payload.component.id;
            let (tracks, vias) = self.engine.project().board.as_ref().map_or_else(
                || (vec![], vec![]),
                |b| {
                    (
                        b.tracks
                            .values()
                            .filter(|t| t.net.component == id)
                            .cloned()
                            .collect(),
                        b.vias
                            .values()
                            .filter(|v| v.net.component == id)
                            .cloned()
                            .collect(),
                    )
                },
            );
            let command = if self.pcb.active {
                Command::RemoveFromBoard { id }
            } else {
                Command::Delete {
                    object: ObjectId::Component(id),
                }
            };
            self.engine.execute(command).map_err(|e| e.to_string())?;
            let cut = Cut {
                payload: payload.clone(),
                pcb_only: self.pcb.active,
                tracks,
                vias,
            };
            self.clipboard = ClipboardState {
                payload: Some(payload),
                count: 0,
                cut: Some(cut),
            };
            self.choose(None);
            self.pcb.clipboard_selected(None);
            self.drag = None;
            self.wire_start = None;
            ctx.copy_text(text);
            Ok(())
        })();
        match result {
            Ok(())=>self.message(if self.pcb.active {"Footprint recortado. Ctrl+V reposiciona; Ctrl+Z restaura. O componente permanece no esquema."} else {"Componente recortado. Ctrl+V move o componente; Ctrl+Z restaura."},false),
            Err(e)=>self.message(e,true)
        }
    }
    fn schematic_clipboard_position(
        &self,
        ctx: &egui::Context,
        component: &Component,
        count: usize,
    ) -> Point {
        let n = i32::try_from(count).unwrap_or(i32::MAX).saturating_add(1);
        let fallback = Point::new(
            component.position.x.saturating_add(n.saturating_mul(4)),
            component.position.y.saturating_add(n.saturating_mul(4)),
        );
        ctx.input(|i| i.pointer.hover_pos())
            .zip(self.canvas_rect)
            .filter(|(p, r)| r.contains(*p))
            .map(|(p, r)| self.placement_point(p, r))
            .unwrap_or(fallback)
    }
    // Uma duplicação iniciada na PCB precisa de um símbolo isolado no esquema,
    // evitando sobrepor pinos/condutores existentes e criar redes acidentais.
    fn isolated_schematic_position(&self, component: &Component) -> Point {
        let project = self.engine.project();
        let mut right = project
            .components
            .values()
            .flat_map(|c| {
                c.pins()
                    .into_iter()
                    .map(|p| p.position.x)
                    .chain(std::iter::once(c.position.x))
            })
            .chain(project.wires.values().flat_map(|w| [w.start.x, w.end.x]))
            .chain(project.labels.values().map(|l| l.position.x))
            .chain(project.junctions.values().map(|j| j.position.x))
            .max()
            .unwrap_or(0);
        let left_offset = component
            .pins()
            .iter()
            .map(|p| p.position.x - component.position.x)
            .min()
            .unwrap_or(0)
            .min(0);
        right = right.saturating_sub(left_offset).saturating_add(8);
        Point::new(right, component.position.y)
    }
    fn restore_copper(&self, cut: &Cut, commands: &mut Vec<Command>) -> Result<(), String> {
        let board = self
            .engine
            .project()
            .board
            .as_ref()
            .ok_or("PCB ausente para restaurar o recorte.")?;
        let mut track_ids: std::collections::BTreeSet<_> = board.tracks.keys().copied().collect();
        let mut via_ids: std::collections::BTreeSet<_> = board.vias.keys().copied().collect();
        for mut track in cut.tracks.clone() {
            if track_ids.contains(&track.id) {
                track.id = track_ids
                    .last()
                    .copied()
                    .unwrap_or(0)
                    .checked_add(1)
                    .ok_or("Limite de IDs de trilhas")?;
            }
            track_ids.insert(track.id);
            commands.push(Command::AddTrack { track });
        }
        for mut via in cut.vias.clone() {
            if via_ids.contains(&via.id) {
                via.id = via_ids
                    .last()
                    .copied()
                    .unwrap_or(0)
                    .checked_add(1)
                    .ok_or("Limite de IDs de vias")?;
            }
            via_ids.insert(via.id);
            commands.push(Command::AddVia { via });
        }
        Ok(())
    }
    fn paste_payload(&mut self, ctx: &egui::Context, payload: Payload) -> Result<(), String> {
        let paste_index = if self.clipboard.payload.as_ref() == Some(&payload) {
            self.clipboard.count
        } else {
            0
        };
        let local_cut = self
            .clipboard
            .cut
            .as_ref()
            .filter(|cut| cut.payload == payload)
            .filter(|cut| {
                let project = self.engine.project();
                let id = cut.payload.component.id;
                if cut.pcb_only {
                    project.components.contains_key(&id)
                        && project
                            .board
                            .as_ref()
                            .is_some_and(|b| !b.placements.contains_key(&id))
                } else {
                    !project.components.contains_key(&id)
                }
            })
            .cloned();
        if local_cut.as_ref().is_some_and(|c| c.pcb_only) && !self.pcb.active {
            return Err(
                "Cole o footprint recortado na visualização PCB, ou use Ctrl+Z para restaurá-lo."
                    .into(),
            );
        }
        if self.pcb.active && payload.component.footprint.is_none() {
            return Err("O componente precisa de footprint para ser colado na PCB.".into());
        }
        let restoring = local_cut.is_some();
        let id = local_cut
            .as_ref()
            .map_or(ComponentId(self.engine.project().next_id), |c| {
                c.payload.component.id
            });
        let position = if self.pcb.active {
            if restoring {
                payload.component.position
            } else {
                self.isolated_schematic_position(&payload.component)
            }
        } else {
            self.schematic_clipboard_position(ctx, &payload.component, paste_index)
        };
        let mut commands = Vec::new();
        if let Some(cut) = &local_cut {
            if !cut.pcb_only {
                let mut component = cut.payload.component.clone();
                component.position = position;
                commands.push(Command::RestoreComponent {
                    component: Box::new(component),
                });
            }
        } else {
            commands.push(Command::PasteComponent {
                component: payload.component.clone(),
                position,
            });
        }
        let placement = if self.pcb.active {
            let board = self
                .engine
                .project()
                .board
                .as_ref()
                .ok_or("Crie a PCB antes de colar um footprint.")?;
            Some(self.pcb.clipboard_placement(
                ctx,
                board,
                payload.placement,
                &payload.component,
                paste_index,
            ))
        } else {
            local_cut.as_ref().and_then(|c| c.payload.placement)
        };
        if let Some(placement) = placement {
            commands.push(Command::PlaceOnBoard { id, placement });
        }
        if let Some(cut) = &local_cut
            && cut.payload.placement.is_some()
        {
            self.restore_copper(cut, &mut commands)?;
        }
        let command = Command::Batch(commands);
        if self.pcb.active {
            eda_pcb::check_edit(self.engine.project(), command.clone())?;
        }
        self.engine.execute(command).map_err(|e| e.to_string())?;
        let same_payload = self.clipboard.payload.as_ref() == Some(&payload);
        self.clipboard.count = if same_payload {
            self.clipboard.count.saturating_add(1)
        } else {
            1
        };
        self.clipboard.payload = Some(payload);
        self.clipboard.cut = None;
        self.tool = Tool::Select;
        self.wire_start = None;
        self.drag = None;
        self.choose(Some(ObjectId::Component(id)));
        self.pcb.clipboard_selected(self.pcb.active.then_some(id));
        self.message(
            if restoring {
                "Componente movido; ID e referência preservados."
            } else {
                "Componente colado com novo ID e referência única."
            },
            false,
        );
        Ok(())
    }
    pub(crate) fn paste_component(&mut self, ctx: &egui::Context) {
        let result = self
            .clipboard
            .payload
            .clone()
            .ok_or("Copie um componente antes de colar.".to_owned())
            .and_then(|p| self.paste_payload(ctx, p));
        if let Err(e) = result {
            self.message(format!("Não foi possível colar: {e}"), true);
        }
    }
    pub(crate) fn clipboard_shortcuts(&mut self, ctx: &egui::Context) -> bool {
        if self.pending.is_some() || ctx.wants_keyboard_input() {
            return false;
        }
        let actions = ctx.input_mut(|input| {
            let native_copy = input.events.iter().any(|e| matches!(e, egui::Event::Copy));
            let native_cut = input.events.iter().any(|e| matches!(e, egui::Event::Cut));
            let native_paste = input
                .events
                .iter()
                .any(|e| matches!(e, egui::Event::Paste(_)));
            let mut actions = Vec::new();
            let mut copy = false;
            let mut cut = false;
            let mut paste = false;
            input.events.retain(|event| {
                let action = match event {
                    egui::Event::Copy => Some(Action::Copy),
                    egui::Event::Cut => Some(Action::Cut),
                    egui::Event::Paste(s) => Some(Action::Paste(Some(s.clone()))),
                    egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } if (modifiers.command || modifiers.ctrl) && !modifiers.alt => match key {
                        Key::C if !native_copy => Some(Action::Copy),
                        Key::X if !native_cut => Some(Action::Cut),
                        Key::V if !native_paste => Some(Action::Paste(None)),
                        Key::C | Key::X | Key::V => return false,
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(action) = action {
                    let seen = match &action {
                        Action::Copy => &mut copy,
                        Action::Cut => &mut cut,
                        Action::Paste(_) => &mut paste,
                    };
                    if !*seen {
                        *seen = true;
                        actions.push(action);
                    }
                    false
                } else {
                    true
                }
            });
            actions
        });
        let handled = !actions.is_empty();
        for action in actions {
            match action {
                Action::Copy => self.copy_component(ctx),
                Action::Cut => self.cut_component(ctx),
                Action::Paste(None) => self.paste_component(ctx),
                Action::Paste(Some(text)) => {
                    if let Err(e) = decode(&text).and_then(|p| self.paste_payload(ctx, p)) {
                        self.message(format!("Não foi possível colar: {e}"), true);
                    }
                }
            }
        }
        handled
    }
}
