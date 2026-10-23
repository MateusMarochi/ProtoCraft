use super::*;

impl PcbUi {
    pub(crate) fn clipboard_selected(&mut self, id: Option<ComponentId>) {
        self.finish_route();
        self.routing = false;
        self.hole_tool = false;
        self.clear_geometry_selection();
        self.selected = id;
        self.drag = None;
        self.report = None;
        self.report_revision = u64::MAX;
    }
    pub(crate) fn clipboard_placement(
        &self,
        ctx: &egui::Context,
        board: &Board,
        source: Option<Placement>,
        component: &Component,
        count: usize,
    ) -> Placement {
        let mut placement = source.unwrap_or(Placement {
            position: NmPoint::new(board.width_nm / 2, board.height_nm / 2),
            rotation: 0,
            side: self.layer,
        });
        let grid = nm(self.grid_mm);
        if let Some((pointer, rect)) = ctx
            .input(|i| i.pointer.hover_pos())
            .zip(self.canvas_rect)
            .filter(|(p, r)| r.contains(*p))
        {
            let point = (pointer - rect.center() - self.pan) / self.zoom;
            placement.position = NmPoint::new(
                (point.x as f64 * 1e6 / grid as f64).round() as i64 * grid,
                (point.y as f64 * 1e6 / grid as f64).round() as i64 * grid,
            );
        } else {
            let size = component
                .footprint
                .as_ref()
                .map_or(0, |f| f.body_bounds.width().max(f.body_bounds.height()));
            let step = size.saturating_add(5_000_000);
            let offset =
                step.saturating_mul(i64::try_from(count).unwrap_or(i64::MAX).saturating_add(1));
            placement.position.x = placement.position.x.saturating_add(offset);
            placement.position.y = placement.position.y.saturating_add(offset);
        }
        placement
    }
    pub(crate) fn reset_for_project(&mut self) {
        let active = self.active;
        *self = Self::default();
        self.active = active;
    }

    fn geometry_selection(&self) -> Option<GeometrySelection> {
        self.selected_track
            .map(GeometrySelection::Track)
            .or(self.selected_via.map(GeometrySelection::Via))
            .or(self.selected_hole.map(GeometrySelection::Hole))
    }
    pub(super) fn clear_geometry_selection(&mut self) {
        self.selected_track = None;
        self.selected_via = None;
        self.selected_hole = None;
        self.geometry_drag = None;
        self.editor_stamp = None;
    }
    fn select_geometry(&mut self, selection: Option<GeometrySelection>) {
        self.clear_geometry_selection();
        self.selected = None;
        match selection {
            Some(GeometrySelection::Track(id)) => self.selected_track = Some(id),
            Some(GeometrySelection::Via(id)) => self.selected_via = Some(id),
            Some(GeometrySelection::Hole(id)) => self.selected_hole = Some(id),
            None => {}
        }
    }
    pub(super) fn finish_route(&mut self) {
        self.route = None;
        self.route_history.clear();
        self.via_requested = false;
    }
    pub(super) fn geometry_at(
        &self,
        board: &Board,
        p: NmPoint,
        tolerance: f64,
    ) -> Option<GeometrySelection> {
        board
            .holes
            .values()
            .filter_map(|h| {
                let distance = ((p.x - h.position.x) as f64).hypot((p.y - h.position.y) as f64);
                (distance <= tolerance.max(h.drill_nm as f64 / 2.0))
                    .then_some((distance, GeometrySelection::Hole(h.id)))
            })
            .chain(board.vias.values().filter_map(|v| {
                let distance = ((p.x - v.position.x) as f64).hypot((p.y - v.position.y) as f64);
                (distance <= tolerance.max(v.diameter_nm as f64 / 2.0))
                    .then_some((distance, GeometrySelection::Via(v.id)))
            }))
            .chain(
                board
                    .tracks
                    .values()
                    .filter(|t| t.layer == self.layer)
                    .filter_map(|t| {
                        let distance = point_distance(p, t.start, t.end);
                        (distance <= tolerance.max(t.width_nm as f64 / 2.0))
                            .then_some((distance, GeometrySelection::Track(t.id)))
                    }),
            )
            .min_by(|(a, _), (b, _)| a.total_cmp(b))
            .map(|(_, s)| s)
    }
}
impl ProtoCraft {
    pub(super) fn pcb_checked_command(&mut self, command: Command) -> bool {
        match eda_pcb::check_edit(self.engine.project(), command.clone()) {
            Ok(report) => match self.engine.execute(command) {
                Ok(()) => {
                    self.pcb.report = Some(report);
                    self.pcb.report_revision = self.engine.revision();
                    self.message("Edição PCB aplicada.", false);
                    true
                }
                Err(e) => {
                    self.message(e.to_string(), true);
                    false
                }
            },
            Err(e) => {
                self.message(trf!("Edição recusada: {error}", error = e), true);
                false
            }
        }
    }
    pub(super) fn pcb_delete_selection(&mut self) {
        let command = match self.pcb.geometry_selection() {
            Some(GeometrySelection::Track(id)) => Some(Command::DeleteTrack { id }),
            Some(GeometrySelection::Via(id)) => Some(Command::DeleteVia { id }),
            Some(GeometrySelection::Hole(id)) => Some(Command::DeleteHole { id }),
            None => None,
        };
        if let Some(command) = command {
            self.command(command);
            self.pcb.clear_geometry_selection();
        } else if let Some(id) = self.pcb.selected.take() {
            self.command(Command::RemoveFromBoard { id });
        }
        self.pcb.finish_route();
        self.pcb.report = None;
    }
    pub(super) fn pcb_route_to(
        &mut self,
        end: NmPoint,
        target: Option<eda_pcb::RoutingTarget>,
        via: bool,
    ) {
        let Some((start, anchor)) = self.pcb.route else {
            return;
        };
        if target.is_some_and(|t| Some(t.net) != self.netlist.net_for_pin(anchor)) {
            self.message(
                "O destino pertence a outra rede. Confira o esquemático.",
                true,
            );
            return;
        }
        let Some(board) = self.engine.project().board.as_ref() else {
            return;
        };
        let mut next = board.tracks.keys().next_back().copied().unwrap_or(0);
        let mut commands = Vec::new();
        let path = eda_pcb::manual_path(start, end, self.pcb.route_mode, self.pcb.route_flip);
        for segment in path.windows(2) {
            let Some(id) = next.checked_add(1) else {
                self.message("Limite de IDs de trilhas.", true);
                return;
            };
            next = id;
            commands.push(Command::AddTrack {
                track: Track {
                    id,
                    start: segment[0],
                    end: segment[1],
                    width_nm: nm(self.pcb.width),
                    layer: self.pcb.layer,
                    net: anchor,
                },
            });
        }
        if via {
            let net = self.netlist.net_for_pin(anchor);
            let existing = board
                .vias
                .values()
                .any(|v| v.position == end && self.netlist.net_for_pin(v.net) == net)
                || eda_pcb::placed_pads(self.engine.project()).is_ok_and(|pads| {
                    pads.iter().any(|p| {
                        p.position == end
                            && p.layers == 3
                            && p.pad.technology == eda_core::PadTechnology::ThroughHole
                            && p.net == net
                    })
                });
            if !existing {
                let Some(id) = board
                    .vias
                    .keys()
                    .next_back()
                    .copied()
                    .unwrap_or(0)
                    .checked_add(1)
                else {
                    self.message("Limite de IDs de vias.", true);
                    return;
                };
                commands.push(Command::AddVia {
                    via: Via {
                        id,
                        position: end,
                        diameter_nm: nm(self.pcb.via_diameter_mm),
                        drill_nm: nm(self.pcb.via_drill_mm),
                        net: anchor,
                    },
                });
            }
        }
        let changed = !commands.is_empty();
        if changed && !self.pcb_checked_command(Command::Batch(commands)) {
            return;
        }
        if changed {
            self.pcb.route_history.push((start, anchor, self.pcb.layer));
        }
        if via {
            self.pcb.layer = if self.pcb.layer == BoardSide::Top {
                BoardSide::Bottom
            } else {
                BoardSide::Top
            };
        }
        self.pcb.route = if target.is_some() && !via {
            None
        } else {
            Some((end, anchor))
        };
        if self.pcb.route.is_none() {
            self.pcb.route_history.clear();
        }
    }
    pub(super) fn pcb_add_hole(&mut self, position: NmPoint) {
        let Some(board) = self.engine.project().board.as_ref() else {
            return;
        };
        let Some(id) = board
            .holes
            .keys()
            .next_back()
            .copied()
            .unwrap_or(0)
            .checked_add(1)
        else {
            self.message("Limite de IDs de furos.", true);
            return;
        };
        if self.pcb_checked_command(Command::AddHole {
            hole: MountingHole {
                id,
                position,
                drill_nm: nm(self.pcb.hole_diameter_mm),
            },
        }) {
            self.pcb.select_geometry(Some(GeometrySelection::Hole(id)));
        }
    }
    pub(super) fn pcb_edit_controls(&mut self, ui: &mut egui::Ui, board: &Board) {
        ui.strong(tr!("Trilhas e furos"));
        ui.horizontal(|ui| {
            if ui
                .selectable_label(!self.pcb.routing && !self.pcb.hole_tool, tr!("Selecionar"))
                .clicked()
            {
                self.pcb.routing = false;
                self.pcb.hole_tool = false;
                self.pcb.finish_route();
            }
            if ui
                .selectable_label(self.pcb.routing, tr!("Trilha · X"))
                .clicked()
            {
                self.pcb.routing = true;
                self.pcb.hole_tool = false;
                self.pcb.selected = None;
                self.pcb.clear_geometry_selection();
            }
            if ui
                .selectable_label(self.pcb.hole_tool, tr!("Furo · H"))
                .clicked()
            {
                self.pcb.finish_route();
                self.pcb.routing = false;
                self.pcb.hole_tool = true;
                self.pcb.selected = None;
            }
        });
        ui.horizontal(|ui| {
            ui.label(tr!("Camada"));
            let old = self.pcb.layer;
            ui.selectable_value(&mut self.pcb.layer, BoardSide::Top, tr!("Superior"));
            ui.selectable_value(&mut self.pcb.layer, BoardSide::Bottom, tr!("Inferior"));
            if old != self.pcb.layer && self.pcb.route.is_some() {
                self.pcb.layer = old;
                self.message("Use V para inserir uma via e trocar de camada.", false);
            }
        });
        ui.horizontal(|ui| {
            ui.label(tr!("Grade (mm)"));
            egui::ComboBox::from_id_salt("pcb_grid")
                .selected_text(format!("{}", self.pcb.grid_mm))
                .show_ui(ui, |ui| {
                    for value in [0.05, 0.1, 0.25, 0.5, 1.0, 2.54] {
                        ui.selectable_value(&mut self.pcb.grid_mm, value, format!("{value}"));
                    }
                });
        });
        if !self.pcb.hole_tool {
            ui.horizontal(|ui| {
                ui.label(tr!("Largura (mm)"));
                ui.add(
                    egui::DragValue::new(&mut self.pcb.width)
                        .speed(0.025)
                        .range(0.05..=10.0),
                );
            });
            ui.horizontal(|ui| {
                ui.label(tr!("Ângulo"));
                for mode in [
                    eda_pcb::RouteMode::Diagonal,
                    eda_pcb::RouteMode::Orthogonal,
                    eda_pcb::RouteMode::Free,
                ] {
                    ui.selectable_value(
                        &mut self.pcb.route_mode,
                        mode,
                        eda_i18n::text(mode.label()),
                    );
                }
            });
            ui.horizontal(|ui| {
                ui.label(tr!("Via / furo (mm)"));
                ui.add(
                    egui::DragValue::new(&mut self.pcb.via_diameter_mm)
                        .speed(0.05)
                        .range(0.2..=20.0),
                );
                ui.add(
                    egui::DragValue::new(&mut self.pcb.via_drill_mm)
                        .speed(0.05)
                        .range(0.1..=10.0),
                );
            });
            if let Some((position, anchor)) = self.pcb.route {
                if let Some(c) = self.engine.project().components.get(&anchor.component) {
                    ui.label(trf!(
                        "Rede de {reference} · pino {pin}",
                        reference = &c.reference,
                        pin = c.pin_number(anchor.pin)
                    ));
                }
                if ui.button(tr!("Via neste ponto · trocar camada")).clicked() {
                    self.pcb_route_to(position, None, true);
                }
                ui.small(tr!("Clique: fixa trecho · Espaço: inverte dobra · V: via no cursor · Backspace: desfaz trecho · Esc: encerra."));
            } else {
                ui.small(tr!("X: iniciar trilha. Clique em pad, via ou trilha da camada ativa. A rede vem do esquemático."));
            }
        } else {
            ui.horizontal(|ui| {
                ui.label(tr!("Furo NPTH (mm)"));
                ui.add(
                    egui::DragValue::new(&mut self.pcb.hole_diameter_mm)
                        .speed(0.1)
                        .range(0.1..=50.0),
                );
            });
            ui.small(tr!(
                "Clique para colocar furos mecânicos não metalizados. Esc volta à seleção."
            ));
        }
        self.pcb_geometry_inspector(ui, board);
        ui.separator();
        ui.checkbox(
            &mut self.pcb.show_airwires,
            tr!("Mostrar conexões pendentes"),
        );
        if ui
            .add_enabled(
                self.pcb.autorouting.is_none() && self.pcb.fabrication.is_none(),
                egui::Button::new(tr!("Autorrotear conexões pendentes")),
            )
            .clicked()
        {
            self.start_autorouting();
        }
        if self.pcb.autorouting.is_some() {
            ui.small(tr!(
                "A busca está em execução em segundo plano; edições novas descartam o resultado."
            ));
        }
        if ui.button(tr!("Verificar DRC e conexões")).clicked() {
            match eda_pcb::check(self.engine.project()) {
                Ok(r) => {
                    self.message(
                        trf!(
                            "DRC: {errors} erros · {airwires} ligações pendentes",
                            errors = r.errors(),
                            airwires = r.airwires.len()
                        ),
                        r.errors() > 0,
                    );
                    self.pcb.report = Some(r);
                }
                Err(e) => self.message(e, true),
            }
        }
        if let Some(report) = &self.pcb.report {
            ui.label(trf!(
                "DRC: {errors} erros · {airwires} pendentes",
                errors = report.errors(),
                airwires = report.airwires.len()
            ));
        }
    }
    fn pcb_geometry_inspector(&mut self, ui: &mut egui::Ui, board: &Board) {
        let Some(selection) = self.pcb.geometry_selection() else {
            return;
        };
        let stamp = (self.engine.revision(), selection);
        if self.pcb.editor_stamp != Some(stamp) {
            let (position, end, width, drill, layer) = match selection {
                GeometrySelection::Track(id) => {
                    let Some(t) = board.tracks.get(&id) else {
                        self.pcb.clear_geometry_selection();
                        return;
                    };
                    (t.start, t.end, t.width_nm, 0, t.layer)
                }
                GeometrySelection::Via(id) => {
                    let Some(v) = board.vias.get(&id) else {
                        self.pcb.clear_geometry_selection();
                        return;
                    };
                    (
                        v.position,
                        v.position,
                        v.diameter_nm,
                        v.drill_nm,
                        BoardSide::Top,
                    )
                }
                GeometrySelection::Hole(id) => {
                    let Some(h) = board.holes.get(&id) else {
                        self.pcb.clear_geometry_selection();
                        return;
                    };
                    (
                        h.position,
                        h.position,
                        h.drill_nm,
                        h.drill_nm,
                        BoardSide::Top,
                    )
                }
            };
            self.pcb.edit_position = [position.x as f64 / 1e6, position.y as f64 / 1e6];
            self.pcb.edit_end = [end.x as f64 / 1e6, end.y as f64 / 1e6];
            self.pcb.edit_width = width as f64 / 1e6;
            self.pcb.edit_drill = drill as f64 / 1e6;
            self.pcb.edit_layer = layer;
            self.pcb.editor_stamp = Some(stamp);
        }
        ui.separator();
        ui.strong(match selection {
            GeometrySelection::Track(id) => trf!("Trilha {id}", id = id),
            GeometrySelection::Via(id) => trf!("Via {id} · metalizada", id = id),
            GeometrySelection::Hole(id) => trf!("Furo {id} · NPTH", id = id),
        });
        ui.horizontal(|ui| {
            ui.label(tr!("X / Y (mm)"));
            for v in &mut self.pcb.edit_position {
                ui.add(
                    egui::DragValue::new(v)
                        .speed(self.pcb.grid_mm)
                        .range(0.0..=2000.0),
                );
            }
        });
        if matches!(selection, GeometrySelection::Track(_)) {
            ui.horizontal(|ui| {
                ui.label(tr!("Fim X / Y"));
                for v in &mut self.pcb.edit_end {
                    ui.add(
                        egui::DragValue::new(v)
                            .speed(self.pcb.grid_mm)
                            .range(0.0..=2000.0),
                    );
                }
            });
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.pcb.edit_layer, BoardSide::Top, tr!("Superior"));
                ui.selectable_value(&mut self.pcb.edit_layer, BoardSide::Bottom, tr!("Inferior"));
            });
        }
        ui.horizontal(|ui| {
            ui.label(if matches!(selection, GeometrySelection::Track(_)) {
                tr!("Largura (mm)")
            } else {
                tr!("Diâmetro (mm)")
            });
            ui.add(
                egui::DragValue::new(&mut self.pcb.edit_width)
                    .speed(0.05)
                    .range(0.05..=50.0),
            );
        });
        if matches!(selection, GeometrySelection::Via(_)) {
            ui.horizontal(|ui| {
                ui.label(tr!("Furação (mm)"));
                ui.add(
                    egui::DragValue::new(&mut self.pcb.edit_drill)
                        .speed(0.05)
                        .range(0.1..=10.0),
                );
            });
        }
        ui.horizontal(|ui| {
            if ui.button(tr!("Aplicar edição")).clicked() {
                let position =
                    NmPoint::new(nm(self.pcb.edit_position[0]), nm(self.pcb.edit_position[1]));
                let command = match selection {
                    GeometrySelection::Track(id) => {
                        let mut track = board.tracks[&id].clone();
                        track.start = position;
                        track.end =
                            NmPoint::new(nm(self.pcb.edit_end[0]), nm(self.pcb.edit_end[1]));
                        track.width_nm = nm(self.pcb.edit_width);
                        track.layer = self.pcb.edit_layer;
                        Command::UpdateTrack { track }
                    }
                    GeometrySelection::Via(id) => {
                        let mut via = board.vias[&id].clone();
                        via.position = position;
                        via.diameter_nm = nm(self.pcb.edit_width);
                        via.drill_nm = nm(self.pcb.edit_drill);
                        Command::UpdateVia { via }
                    }
                    GeometrySelection::Hole(id) => Command::UpdateHole {
                        hole: MountingHole {
                            id,
                            position,
                            drill_nm: nm(self.pcb.edit_width),
                        },
                    },
                };
                self.pcb_checked_command(command);
            }
            if ui.button(tr!("Excluir · Del")).clicked() {
                self.pcb_delete_selection();
            }
        });
        ui.small(tr!(
            "Arraste para mover. Ao mover cobre, confira as conexões pendentes."
        ));
    }
    pub(super) fn pcb_geometry_drag_command(
        &self,
        board: &Board,
        selection: GeometrySelection,
        delta: NmPoint,
    ) -> Option<Command> {
        let translate = |p: NmPoint| NmPoint::new(p.x + delta.x, p.y + delta.y);
        Some(match selection {
            GeometrySelection::Track(id) => {
                let mut track = board.tracks.get(&id)?.clone();
                track.start = translate(track.start);
                track.end = translate(track.end);
                Command::UpdateTrack { track }
            }
            GeometrySelection::Via(id) => {
                let mut via = board.vias.get(&id)?.clone();
                via.position = translate(via.position);
                Command::UpdateVia { via }
            }
            GeometrySelection::Hole(id) => {
                let mut hole = board.holes.get(&id)?.clone();
                hole.position = translate(hole.position);
                Command::UpdateHole { hole }
            }
        })
    }
    pub(super) fn pcb_select_geometry(&mut self, selection: Option<GeometrySelection>) {
        self.pcb.select_geometry(selection);
    }
}
