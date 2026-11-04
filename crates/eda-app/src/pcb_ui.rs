use super::*;
use eda_core::{
    Board, BoardSide, ComponentId, MountingHole, NM_PER_MM, NmPoint, PinRef, Placement, Track, Via,
};
mod edit;
#[cfg(test)]
mod workflow_tests;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GeometrySelection {
    Track(u64),
    Via(u64),
    Hole(u64),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum PcbInspectorTab {
    #[default]
    Board,
    Layers,
    Drc,
}

pub(super) struct PcbUi {
    pub active: bool,
    pub fit: bool,
    pub report: Option<eda_pcb::PcbReport>,
    inspector_tab: PcbInspectorTab,
    pub(super) show_inspector: bool,
    report_revision: u64,
    canvas_rect: Option<Rect>,
    pub route: Option<(NmPoint, PinRef)>,
    pub selected: Option<ComponentId>,
    selected_track: Option<u64>,
    selected_via: Option<u64>,
    selected_hole: Option<u64>,
    hole_tool: bool,
    grid_mm: f64,
    via_diameter_mm: f64,
    via_drill_mm: f64,
    hole_diameter_mm: f64,
    route_mode: eda_pcb::RouteMode,
    route_flip: bool,
    route_history: Vec<(NmPoint, PinRef, BoardSide)>,
    via_requested: bool,
    geometry_drag: Option<(GeometrySelection, NmPoint)>,
    editor_stamp: Option<(u64, GeometrySelection)>,
    edit_position: [f64; 2],
    edit_end: [f64; 2],
    edit_width: f64,
    edit_drill: f64,
    edit_layer: BoardSide,
    pub(super) layer: BoardSide,
    width: f64,
    zoom: f32,
    pan: Vec2,
    drag: Option<(ComponentId, NmPoint, NmPoint)>,
    routing: bool,
    width_mm: f64,
    height_mm: f64,
    clearance_mm: f64,
    net_class_name: String,
    net_class_net: String,
    net_class_width_mm: f64,
    net_class_clearance_mm: f64,
    net_class_via_mm: f64,
    net_class_drill_mm: f64,
    dimensions_revision: u64,
    show_airwires: bool,
    preserve_connections: bool,
    routing_metrics: Option<(u64, eda_pcb::metrics::RoutingMetrics)>,
    fabrication: Option<mpsc::Receiver<Result<PathBuf, String>>>,
    autorouting: Option<mpsc::Receiver<Result<eda_pcb::AutoroutePlan, String>>>,
    autorouting_revision: u64,
}

impl Default for PcbUi {
    fn default() -> Self {
        Self {
            active: false,
            fit: true,
            report: None,
            inspector_tab: PcbInspectorTab::Board,
            show_inspector: false,
            report_revision: u64::MAX,
            canvas_rect: None,
            route: None,
            selected: None,
            selected_track: None,
            selected_via: None,
            selected_hole: None,
            hole_tool: false,
            grid_mm: 0.25,
            via_diameter_mm: 0.6,
            via_drill_mm: 0.3,
            hole_diameter_mm: 3.2,
            route_mode: Default::default(),
            route_flip: false,
            route_history: Vec::new(),
            via_requested: false,
            geometry_drag: None,
            editor_stamp: None,
            edit_position: [0.0; 2],
            edit_end: [0.0; 2],
            edit_width: 0.25,
            edit_drill: 0.3,
            edit_layer: BoardSide::Top,
            layer: BoardSide::Top,
            width: 0.25,
            zoom: 8.0,
            pan: Vec2::ZERO,
            drag: None,
            routing: false,
            width_mm: 100.0,
            height_mm: 80.0,
            clearance_mm: 0.2,
            net_class_name: "ALIMENTACAO".into(),
            net_class_net: String::new(),
            net_class_width_mm: 0.35,
            net_class_clearance_mm: 0.2,
            net_class_via_mm: 0.6,
            net_class_drill_mm: 0.3,
            dimensions_revision: u64::MAX,
            show_airwires: true,
            preserve_connections: true,
            routing_metrics: None,
            fabrication: None,
            autorouting: None,
            autorouting_revision: u64::MAX,
        }
    }
}
fn nm(mm: f64) -> i64 {
    (mm * NM_PER_MM as f64).round() as i64
}
fn point_distance(p: NmPoint, a: NmPoint, b: NmPoint) -> f64 {
    let (x, y) = ((b.x - a.x) as f64, (b.y - a.y) as f64);
    let t = (((p.x - a.x) as f64 * x + (p.y - a.y) as f64 * y) / (x * x + y * y).max(1.0))
        .clamp(0.0, 1.0);
    ((p.x - a.x) as f64 - t * x).hypot((p.y - a.y) as f64 - t * y)
}
impl ProtoCraft {
    fn move_board_component(&mut self, id: ComponentId, placement: Placement) {
        if !self.pcb.preserve_connections {
            self.pcb_checked_command(Command::PlaceOnBoard { id, placement });
            return;
        }
        match eda_pcb::placement::plan_placement(self.engine.project(), id, placement) {
            Ok(commands) => {
                self.pcb_checked_command(Command::Batch(commands));
            }
            Err(error) => self.message(eda_i18n::text(&error), true),
        }
    }
    pub(super) fn pcb_zoom_percent(&self) -> f32 {
        self.pcb.zoom / 8.0 * 100.0
    }

    pub(super) fn sync_board(&mut self) {
        self.pcb.finish_route();
        match eda_pcb::sync::plan(self.engine.project()) {
            Ok(plan) => {
                if let Err(error) = self.engine.execute(Command::Batch(plan.commands)) {
                    self.message(error.to_string(), true);
                    return;
                }
                self.pcb.fit = true;
                self.pcb.report = None;
                self.message(
                    "PCB sincronizada. Posições existentes preservadas; confira o DRC.",
                    false,
                );
            }
            Err(error) => self.message(error.to_string(), true),
        }
    }
    pub(super) fn pcb_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() || self.pending.is_some() {
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.pcb.route.is_some() {
                self.pcb.finish_route();
            } else {
                self.pcb.routing = false;
                self.pcb.hole_tool = false;
            }
            self.pcb.drag = None;
            self.pcb.geometry_drag = None;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F)) {
            self.pcb.fit = true;
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z)) {
            self.engine.undo();
            self.pcb.finish_route();
            self.pcb.report = None;
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Y)) {
            self.engine.redo();
            self.pcb.finish_route();
            self.pcb.report = None;
        }
        if ctx.input(|i| i.modifiers.command || i.modifiers.ctrl || i.modifiers.alt) {
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Backspace))
            && self.pcb.route.is_some()
            && let Some((position, anchor, layer)) = self.pcb.route_history.pop()
        {
            self.engine.undo();
            self.pcb.route = Some((position, anchor));
            self.pcb.layer = layer;
            self.pcb.report = None;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::X)) {
            self.pcb.routing = true;
            self.pcb.hole_tool = false;
            self.pcb.selected = None;
            self.pcb.clear_geometry_selection();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::H)) {
            self.pcb.finish_route();
            self.pcb.hole_tool = true;
            self.pcb.routing = false;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Space)) && self.pcb.routing {
            self.pcb.route_flip = !self.pcb.route_flip;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::V)) && self.pcb.route.is_some() {
            self.pcb.via_requested = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::R)) {
            self.rotate_board_selection();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
            self.pcb_delete_selection();
        }
    }
    fn rotate_board_selection(&mut self) {
        if let Some(id) = self.pcb.selected
            && let Some(mut p) = self
                .engine
                .project()
                .board
                .as_ref()
                .and_then(|b| b.placements.get(&id))
                .copied()
        {
            p.rotation = (p.rotation + 1) % 4;
            self.command(Command::PlaceOnBoard { id, placement: p });
            self.pcb.report = None;
        }
    }
    fn export_pcb_dialog(&mut self, svg: bool) {
        let ext = if svg { "svg" } else { "kicad_pcb" };
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(tr!("PCB"), &[ext])
            .set_file_name(format!("projeto.{ext}"))
            .save_file()
        {
            if let Err(e) = ensure_export_destination(&path, self.path.as_deref()) {
                self.message(e, true);
                return;
            }
            let result = if svg {
                eda_pcb::export_svg(self.engine.project())
            } else {
                eda_pcb::export_kicad(self.engine.project())
            };
            match result.and_then(|text| std::fs::write(&path, text).map_err(|e| e.to_string())) {
                Ok(()) => self.message(trf!("PCB exportada: {path}", path = path.display()), false),
                Err(e) => self.message(e, true),
            }
        }
    }
    pub(super) fn pcb_tools(&mut self, root_ui: &mut egui::Ui, ctx: &egui::Context) {
        let autoroute_result =
            self.pcb
                .autorouting
                .as_ref()
                .and_then(|receiver| match receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(mpsc::TryRecvError::Empty) => None,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        Some(Err("A tarefa de autorroteamento foi encerrada.".into()))
                    }
                });
        if let Some(result) = autoroute_result {
            self.pcb.autorouting = None;
            match result {
                Ok(plan) if self.engine.revision() == self.pcb.autorouting_revision => {
                    let stats = plan.stats.clone();
                    match self.engine.execute(Command::Batch(plan.commands)) {
                        Ok(()) => {
                            self.pcb.report = None;
                            self.pcb.finish_route();
                            self.message(
                                trf!("Autorroteamento: {routed} conexões, {tracks} trilhas, {vias} vias, {pending} pendentes, {failed} sem caminho.", routed=stats.routed_connections, tracks=stats.track_count, vias=stats.via_count, pending=stats.remaining_airwires, failed=stats.unrouted_connections),
                                stats.unrouted_connections > 0,
                            );
                        }
                        Err(error) => self.message(error.to_string(), true),
                    }
                }
                Ok(_) => self.message(
                    tr!("A placa mudou durante a busca; o resultado antigo foi descartado."),
                    true,
                ),
                Err(error) => self.message(error, true),
            }
        }
        let completed = self
            .pcb
            .fabrication
            .as_ref()
            .and_then(|r| r.try_recv().ok());
        if let Some(result) = completed {
            self.pcb.fabrication = None;
            match result {
                Ok(path) => {
                    let zip = path.join("JLCPCB.zip");
                    self.message(
                        if zip.is_file() {
                            trf!("ZIP para JLCPCB gerado: {path}", path = zip.display())
                        } else {
                            trf!("Gerber e Excellon gerados: {path}", path = path.display())
                        },
                        false,
                    );
                }
                Err(e) => self.message(e, true),
            }
        }
        if self.pcb.fabrication.is_some() || self.pcb.autorouting.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }

        let panel_fill = if self.dark {
            egui::Color32::from_rgb(32, 41, 47)
        } else {
            egui::Color32::from_rgb(225, 236, 242)
        };
        egui::Panel::left("pcb_tools")
            .default_size(160.0)
            .size_range(140.0..=250.0)
            .frame(egui::Frame::new().fill(panel_fill))
            .show(root_ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(5.0);
                    ui.heading(egui::RichText::new(tr!("Componentes")).strong());
                    ui.small(tr!("PCB · duas camadas · milímetros"));
                    let Some(board) = self.engine.project().board.as_deref().cloned() else {
                        ui.label(tr!("Crie a PCB para adicionar trilhas e furos."));
                        return;
                    };

                    let (preview, _) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), 92.0),
                        egui::Sense::hover(),
                    );
                    let painter = ui.painter();
                    painter.rect_filled(preview, 0.0, egui::Color32::from_rgb(11, 14, 18));
                    for x in (preview.left() as i32..preview.right() as i32).step_by(14) {
                        painter.line_segment(
                            [Pos2::new(x as f32, preview.top()), Pos2::new(x as f32, preview.bottom())],
                            Stroke::new(0.5_f32, egui::Color32::from_rgb(38, 44, 50)),
                        );
                    }
                    for y in (preview.top() as i32..preview.bottom() as i32).step_by(14) {
                        painter.line_segment(
                            [Pos2::new(preview.left(), y as f32), Pos2::new(preview.right(), y as f32)],
                            Stroke::new(0.5_f32, egui::Color32::from_rgb(38, 44, 50)),
                        );
                    }
                    let scale = ((preview.width() - 28.0) / (board.width_nm as f32 / 1e6))
                        .min((preview.height() - 20.0) / (board.height_nm as f32 / 1e6));
                    let board_size = Vec2::new(
                        board.width_nm as f32 / 1e6 * scale,
                        board.height_nm as f32 / 1e6 * scale,
                    );
                    let board_rect = Rect::from_center_size(preview.center(), board_size);
                    painter.rect_stroke(
                        board_rect,
                        0.0,
                        Stroke::new(1.5_f32, egui::Color32::from_rgb(48, 126, 207)),
                        egui::StrokeKind::Inside,
                    );
                    for placement in board.placements.values() {
                        let position = Pos2::new(
                            board_rect.left() + placement.position.x as f32 / 1e6 * scale,
                            board_rect.top() + placement.position.y as f32 / 1e6 * scale,
                        );
                        painter.circle_filled(position, 2.0, egui::Color32::from_rgb(216, 173, 75));
                    }
                    ui.add_space(4.0);

                    egui::CollapsingHeader::new(tr!("Componentes"))
                        .default_open(true)
                        .show(ui, |ui| {
                            let rows: Vec<_> = self
                                .engine
                                .project()
                                .components
                                .values()
                                .filter(|component| component.footprint.is_some())
                                .map(|component| {
                                    (
                                        component.id,
                                        component.reference.clone(),
                                        component.device.as_ref().map_or_else(
                                            || localized_component_label(component.kind),
                                            |device| device.id.clone(),
                                        ),
                                    )
                                })
                                .collect();
                            for (id, reference, name) in rows {
                                if ui
                                    .selectable_label(
                                        self.pcb.selected == Some(id),
                                        trf!("{reference} · {name}", reference = reference, name = name),
                                    )
                                    .clicked()
                                {
                                    self.pcb.clear_geometry_selection();
                                    self.pcb.selected = Some(id);
                                    self.pcb.finish_route();
                                    self.pcb.routing = false;
                                    self.pcb.hole_tool = false;
                                }
                            }
                            if let Some(id) = self.pcb.selected
                                && let Some(component) = self.engine.project().components.get(&id).cloned()
                            {
                                if let Some(footprint) = &component.footprint {
                                    ui.small(&footprint.id);
                                    ui.label(trf!(
                                        "Corpo {width} × {height} mm",
                                        width = format!("{:.3}", footprint.body_bounds.width() as f64 / 1e6),
                                        height = format!("{:.3}", footprint.body_bounds.height() as f64 / 1e6)
                                    ));
                                }
                                if let Some(placement) = board.placements.get(&id) {
                                    ui.label(trf!(
                                        "X {x} · Y {y} mm · {rotation}°",
                                        x = format!("{:.3}", placement.position.x as f64 / 1e6),
                                        y = format!("{:.3}", placement.position.y as f64 / 1e6),
                                        rotation = u16::from(placement.rotation) * 90
                                    ));
                                    ui.horizontal(|ui| {
                                        if ui.button(tr!("Girar 90° · R")).clicked() {
                                            self.rotate_board_selection();
                                        }
                                        if ui.button(tr!("Trocar face")).clicked() {
                                            let mut placement = *placement;
                                            placement.side = if placement.side == BoardSide::Top {
                                                BoardSide::Bottom
                                            } else {
                                                BoardSide::Top
                                            };
                                            self.command(Command::PlaceOnBoard { id, placement });
                                            self.pcb.report = None;
                                        }
                                    });
                                } else {
                                    ui.label(tr!("Clique no canvas para posicionar."));
                                }
                            }
                        });

                    ui.separator();
                    egui::CollapsingHeader::new(tr!("Trilhas e furos"))
                        .default_open(false)
                        .show(ui, |ui| self.pcb_edit_controls(ui, &board));
                    egui::CollapsingHeader::new(tr!("Configuração da placa")).show(ui, |ui| {
                        if self.pcb.dimensions_revision != self.engine.revision() {
                            self.pcb.width_mm = board.width_nm as f64 / 1e6;
                            self.pcb.height_mm = board.height_nm as f64 / 1e6;
                            self.pcb.clearance_mm = board.clearance_nm as f64 / 1e6;
                            if let Some((name, rules)) = board.net_classes.first_key_value() {
                                self.pcb.net_class_name.clone_from(name);
                                self.pcb.net_class_width_mm = rules.track_width_nm as f64 / 1e6;
                                self.pcb.net_class_clearance_mm = rules.clearance_nm as f64 / 1e6;
                                self.pcb.net_class_via_mm = rules.via_diameter_nm as f64 / 1e6;
                                self.pcb.net_class_drill_mm = rules.via_drill_nm as f64 / 1e6;
                            }
                            self.pcb.dimensions_revision = self.engine.revision();
                        }
                        ui.horizontal(|ui| {
                            ui.label(tr!("Largura"));
                            ui.add(egui::DragValue::new(&mut self.pcb.width_mm).speed(1.0).range(1.0..=2000.0));
                        });
                        ui.horizontal(|ui| {
                            ui.label(tr!("Altura"));
                            ui.add(egui::DragValue::new(&mut self.pcb.height_mm).speed(1.0).range(1.0..=2000.0));
                        });
                        ui.horizontal(|ui| {
                            ui.label(tr!("Folga de cobre"));
                            ui.add(egui::DragValue::new(&mut self.pcb.clearance_mm).speed(0.025).range(0.0..=10.0));
                        });
                        if ui.button(tr!("Aplicar dimensões e folga")).clicked() {
                            let mut board = board.clone();
                            board.width_nm = nm(self.pcb.width_mm);
                            board.height_nm = nm(self.pcb.height_mm);
                            board.clearance_nm = nm(self.pcb.clearance_mm);
                            self.command(Command::SetBoard { board });
                            self.pcb.finish_route();
                            self.pcb.fit = true;
                            self.pcb.report = None;
                        }
                        ui.separator();
                        ui.strong(tr!("Classes de rede"));
                        ui.label(tr!("As regras nomeadas afetam folgas, largura mínima e o autorroteador."));
                        for (name, rules) in &board.net_classes {
                            let nets = rules.net_names.iter().cloned().collect::<Vec<_>>().join(", ");
                            ui.label(trf!("{name}: {nets} · largura {width} mm · folga {clearance} mm", name=name, nets=nets, width=format!("{:.3}", rules.track_width_nm as f64 / 1e6), clearance=format!("{:.3}", rules.clearance_nm as f64 / 1e6)));
                        }
                        if !self.netlist.nets.is_empty() {
                            egui::ComboBox::from_id_salt("pcb-net-class-network")
                                .selected_text(if self.pcb.net_class_net.is_empty() {
                                    tr!("Selecione uma rede")
                                } else {
                                    self.pcb.net_class_net.clone()
                                })
                                .show_ui(ui, |ui| {
                                    for net in &self.netlist.nets {
                                        ui.selectable_value(
                                            &mut self.pcb.net_class_net,
                                            net.name.clone(),
                                            &net.name,
                                        );
                                    }
                                });
                        }
                        ui.horizontal(|ui| {
                            ui.label(tr!("Nome"));
                            ui.text_edit_singleline(&mut self.pcb.net_class_name);
                        });
                        ui.horizontal(|ui| {
                            ui.label(tr!("Largura mín. (mm)"));
                            ui.add(egui::DragValue::new(&mut self.pcb.net_class_width_mm).speed(0.05).range(0.05..=10.0));
                        });
                        ui.horizontal(|ui| {
                            ui.label(tr!("Folga mín. (mm)"));
                            ui.add(egui::DragValue::new(&mut self.pcb.net_class_clearance_mm).speed(0.025).range(0.0..=10.0));
                        });
                        ui.horizontal(|ui| {
                            ui.label(tr!("Via: diâmetro/furo (mm)"));
                            ui.add(egui::DragValue::new(&mut self.pcb.net_class_via_mm).speed(0.05).range(0.3..=20.0));
                            ui.add(egui::DragValue::new(&mut self.pcb.net_class_drill_mm).speed(0.05).range(0.1..=19.9));
                        });
                        if ui
                            .add_enabled(
                                !self.pcb.net_class_name.trim().is_empty()
                                    && !self.pcb.net_class_net.is_empty(),
                                egui::Button::new(tr!("Adicionar rede à classe")),
                            )
                            .clicked()
                        {
                            let mut updated = board.clone();
                            let rules = updated
                                .net_classes
                                .entry(self.pcb.net_class_name.trim().to_owned())
                                .or_insert_with(|| eda_core::NetClass {
                                    net_names: Default::default(),
                                    clearance_nm: nm(self.pcb.net_class_clearance_mm),
                                    track_width_nm: nm(self.pcb.net_class_width_mm),
                                    via_diameter_nm: nm(self.pcb.net_class_via_mm),
                                    via_drill_nm: nm(self.pcb.net_class_drill_mm),
                                    pad_escape: None,
                                });
                            rules.net_names.insert(self.pcb.net_class_net.clone());
                            rules.clearance_nm = nm(self.pcb.net_class_clearance_mm);
                            rules.track_width_nm = nm(self.pcb.net_class_width_mm);
                            rules.via_diameter_nm = nm(self.pcb.net_class_via_mm);
                            rules.via_drill_nm = nm(self.pcb.net_class_drill_mm);
                            self.command(Command::SetBoard { board: updated });
                            self.pcb.report = None;
                        }
                        let removals: Vec<_> = board
                            .net_classes
                            .keys()
                            .filter(|name| {
                                ui.button(trf!("Remover classe {name}", name=name)).clicked()
                            })
                            .cloned()
                            .collect();
                        if !removals.is_empty() {
                            let mut updated = board.clone();
                            for name in removals {
                                updated.net_classes.remove(&name);
                            }
                            self.command(Command::SetBoard { board: updated });
                            self.pcb.report = None;
                        }
                    });
                    if ui.button(tr!("Enquadrar · F")).clicked() {
                        self.pcb.fit = true;
                    }
                    ui.checkbox(&mut self.pcb.preserve_connections, tr!("Manter trilhas conectadas ao arrastar"))
                        .on_hover_text(tr!("Translação na mesma face. Vias, ramificações e contatos ambíguos são recusados."));
                    egui::CollapsingHeader::new(tr!("Fabricação")).show(ui, |ui| {
                        self.pcb_fabrication_controls(ui);
                        if ui.button(tr!("Exportar PCB KiCad…")).clicked() {
                            self.export_pcb_dialog(false);
                        }
                        if ui.button(tr!("Exportar desenho SVG em escala…")).clicked() {
                            self.export_pcb_dialog(true);
                        }
                    });
                    if let Some(report) = &self.pcb.report {
                        egui::CollapsingHeader::new(tr!("Diagnósticos DRC")).show(ui, |ui| {
                            for diagnostic in report.diagnostics.iter().take(80) {
                                let color = if diagnostic.severity == "error" {
                                    egui::Color32::from_rgb(221, 92, 87)
                                } else {
                                    Palette::new(ctx).muted
                                };
                                ui.colored_label(
                                    color,
                                    trf!(
                                        "{code}: {message}",
                                        code = &diagnostic.code,
                                        message = eda_i18n::text(&diagnostic.message)
                                    ),
                                );
                            }
                        });
                    }
                    ui.small(tr!("As trilhas usam as redes do esquemático. DRC é conferido antes de confirmar cobre e furos."));
                });
            });
    }
    pub(super) fn pcb_mode_strip(&mut self, root_ui: &mut egui::Ui, _ctx: &egui::Context) {
        let strip_fill = if self.dark {
            egui::Color32::from_rgb(36, 47, 54)
        } else {
            egui::Color32::from_rgb(212, 228, 237)
        };
        egui::Panel::left("pcb_mode_strip")
            .exact_size(46.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(strip_fill))
            .show(root_ui, |ui| {
                ui.add_space(8.0);
                let tools = [
                    (
                        ToolIcon::Select,
                        tr!("Selecionar"),
                        !self.pcb.routing && !self.pcb.hole_tool,
                    ),
                    (ToolIcon::Wire, tr!("Trilha · X"), self.pcb.routing),
                    (ToolIcon::Hole, tr!("Furo · H"), self.pcb.hole_tool),
                ];
                for (icon, tooltip, active) in tools {
                    let response =
                        ui.add_sized([34.0, 30.0], egui::Button::new("").selected(active));
                    let color = if active {
                        ui.visuals().text_color()
                    } else {
                        ui.visuals().widgets.inactive.fg_stroke.color
                    };
                    paint_tool_icon(ui.painter(), response.rect.shrink(7.0), icon, color);
                    if response.on_hover_text(tooltip).clicked() {
                        match icon {
                            ToolIcon::Select => {
                                self.pcb.routing = false;
                                self.pcb.hole_tool = false;
                                self.pcb.finish_route();
                            }
                            ToolIcon::Wire => {
                                self.pcb.routing = true;
                                self.pcb.hole_tool = false;
                                self.pcb.selected = None;
                                self.pcb.clear_geometry_selection();
                            }
                            ToolIcon::Hole => {
                                self.pcb.finish_route();
                                self.pcb.routing = false;
                                self.pcb.hole_tool = true;
                                self.pcb.selected = None;
                            }
                            _ => {}
                        }
                    }
                }
                ui.separator();
                ui.label(egui::RichText::new(tr!("Camada")).small());
                for (side, name) in [(BoardSide::Top, "T"), (BoardSide::Bottom, "B")] {
                    let active = self.pcb.layer == side;
                    let tooltip = if side == BoardSide::Top {
                        tr!("Camada superior de cobre")
                    } else {
                        tr!("Camada inferior de cobre")
                    };
                    if ui
                        .add_sized([34.0, 28.0], egui::Button::new(name).selected(active))
                        .on_hover_text(tooltip)
                        .clicked()
                    {
                        if self.pcb.route.is_some() {
                            self.message(
                                tr!("Use V para inserir uma via e trocar de camada."),
                                false,
                            );
                        } else {
                            self.pcb.layer = side;
                        }
                    }
                }
                ui.separator();
                let autoroute_enabled = self.engine.project().board.is_some()
                    && self.pcb.autorouting.is_none()
                    && self.pcb.fabrication.is_none();
                let response =
                    ui.add_enabled(autoroute_enabled, egui::Button::new("").selected(false));
                let color = ui.visuals().widgets.inactive.fg_stroke.color;
                paint_tool_icon(
                    ui.painter(),
                    response.rect.shrink(7.0),
                    ToolIcon::AutoRoute,
                    color,
                );
                if response
                    .on_hover_text(tr!("Autorrotear conexões pendentes"))
                    .clicked()
                {
                    self.start_autorouting();
                }
            });
    }

    pub(super) fn pcb_inspector(&mut self, root_ui: &mut egui::Ui, ctx: &egui::Context) {
        let theme = Palette::new(ctx);
        let Some(board) = self.engine.project().board.as_deref().cloned() else {
            return;
        };
        egui::Panel::right("pcb_inspector")
            .default_size(265.0)
            .size_range(220.0..=380.0)
            .show(root_ui, |ui| {
                ui.add_space(8.0);
                ui.heading(tr!("Explorador da PCB"));
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(
                            self.pcb.inspector_tab == PcbInspectorTab::Board,
                            tr!("Placa"),
                        )
                        .clicked()
                    {
                        self.pcb.inspector_tab = PcbInspectorTab::Board;
                    }
                    if ui
                        .selectable_label(
                            self.pcb.inspector_tab == PcbInspectorTab::Layers,
                            tr!("Camadas"),
                        )
                        .clicked()
                    {
                        self.pcb.inspector_tab = PcbInspectorTab::Layers;
                    }
                    if ui
                        .selectable_label(
                            self.pcb.inspector_tab == PcbInspectorTab::Drc,
                            tr!("DRC"),
                        )
                        .clicked()
                    {
                        self.pcb.inspector_tab = PcbInspectorTab::Drc;
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| match self.pcb.inspector_tab {
                    PcbInspectorTab::Board => {
                        ui.strong(tr!("Visão geral"));
                        ui.label(trf!(
                            "Dimensões: {width} × {height} mm",
                            width = format!("{:.2}", board.width_nm as f64 / 1e6),
                            height = format!("{:.2}", board.height_nm as f64 / 1e6)
                        ));
                        ui.label(trf!(
                            "{count} componentes · {tracks} trilhas · {vias} vias",
                            count = board.placements.len(),
                            tracks = board.tracks.len(),
                            vias = board.vias.len()
                        ));
                        ui.label(trf!("{count} furos de montagem", count = board.holes.len()));
                        ui.horizontal(|ui| {
                            ui.label(tr!("Camada ativa"));
                            ui.strong(if self.pcb.layer == BoardSide::Top {
                                tr!("Superior")
                            } else {
                                tr!("Inferior")
                            });
                        });
                        ui.checkbox(
                            &mut self.pcb.show_airwires,
                            tr!("Mostrar conexões não roteadas"),
                        );

                        ui.separator();
                        ui.strong(tr!("Componentes e posicionamento"));
                        let rows: Vec<_> = self
                            .engine
                            .project()
                            .components
                            .values()
                            .filter(|component| component.footprint.is_some())
                            .map(|component| {
                                (
                                    component.id,
                                    component.reference.clone(),
                                    component.device.as_ref().map_or_else(
                                        || localized_component_label(component.kind),
                                        |device| device.id.clone(),
                                    ),
                                )
                            })
                            .collect();
                        for (id, reference, name) in rows {
                            if ui
                                .selectable_label(
                                    self.pcb.selected == Some(id),
                                    format!("{reference} · {name}"),
                                )
                                .clicked()
                            {
                                self.pcb.clear_geometry_selection();
                                self.pcb.selected = Some(id);
                                self.pcb.finish_route();
                                self.pcb.routing = false;
                                self.pcb.hole_tool = false;
                            }
                        }
                        if let Some(id) = self.pcb.selected
                            && let Some(component) = self.engine.project().components.get(&id)
                        {
                            ui.separator();
                            ui.strong(&component.reference);
                            if let Some(footprint) = &component.footprint {
                                ui.label(&footprint.id);
                                ui.label(trf!(
                                    "Corpo {width} × {height} mm",
                                    width = format!(
                                        "{:.3}",
                                        footprint.body_bounds.width() as f64 / 1e6
                                    ),
                                    height = format!(
                                        "{:.3}",
                                        footprint.body_bounds.height() as f64 / 1e6
                                    )
                                ));
                            }
                            if let Some(placement) = board.placements.get(&id) {
                                ui.label(trf!(
                                    "X {x} · Y {y} mm",
                                    x = format!("{:.3}", placement.position.x as f64 / 1e6),
                                    y = format!("{:.3}", placement.position.y as f64 / 1e6)
                                ));
                                ui.horizontal(|ui| {
                                    if ui.button(tr!("Girar 90° · R")).clicked() {
                                        self.rotate_board_selection();
                                    }
                                    if ui.button(tr!("Trocar face")).clicked() {
                                        let mut placement = *placement;
                                        placement.side = if placement.side == BoardSide::Top {
                                            BoardSide::Bottom
                                        } else {
                                            BoardSide::Top
                                        };
                                        self.command(Command::PlaceOnBoard { id, placement });
                                        self.pcb.report = None;
                                    }
                                });
                            }
                        } else if let Some(id) = self.pcb.selected_track
                            && let Some(track) = board.tracks.get(&id)
                        {
                            ui.separator();
                            ui.strong(tr!("Trilha selecionada"));
                            ui.label(trf!(
                                "Camada: {layer}",
                                layer = if track.layer == BoardSide::Top {
                                    tr!("Superior")
                                } else {
                                    tr!("Inferior")
                                }
                            ));
                            ui.label(trf!(
                                "Largura: {width} mm",
                                width = format!("{:.3}", track.width_nm as f64 / 1e6)
                            ));
                            if let Some(component) =
                                self.engine.project().components.get(&track.net.component)
                            {
                                ui.label(trf!(
                                    "Rede de {reference} · pino {pin}",
                                    reference = &component.reference,
                                    pin = component.pin_number(track.net.pin)
                                ));
                            }
                        } else if let Some(id) = self.pcb.selected_via
                            && let Some(via) = board.vias.get(&id)
                        {
                            ui.separator();
                            ui.strong(tr!("Via selecionada"));
                            ui.label(trf!(
                                "Diâmetro: {diameter} mm · furação: {drill} mm",
                                diameter = format!("{:.3}", via.diameter_nm as f64 / 1e6),
                                drill = format!("{:.3}", via.drill_nm as f64 / 1e6)
                            ));
                        } else if let Some(id) = self.pcb.selected_hole
                            && let Some(hole) = board.holes.get(&id)
                        {
                            ui.separator();
                            ui.strong(tr!("Furo selecionado"));
                            ui.label(trf!(
                                "Diâmetro de furação: {drill} mm",
                                drill = format!("{:.3}", hole.drill_nm as f64 / 1e6)
                            ));
                        } else if let Some(report) = &self.pcb.report {
                            ui.separator();
                            ui.strong(tr!("Diagnósticos DRC"));
                            ui.label(trf!("{count} erros", count = report.errors()));
                        } else {
                            ui.separator();
                            ui.label(
                                egui::RichText::new(tr!("Nenhum componente selecionado."))
                                    .color(theme.muted),
                            );
                        }
                    }
                    PcbInspectorTab::Layers => {
                        ui.strong(tr!("Camadas de cobre"));
                        ui.add_enabled_ui(self.pcb.route.is_none(), |ui| {
                            ui.selectable_value(
                                &mut self.pcb.layer,
                                BoardSide::Top,
                                tr!("Superior"),
                            );
                            ui.selectable_value(
                                &mut self.pcb.layer,
                                BoardSide::Bottom,
                                tr!("Inferior"),
                            );
                        });
                        ui.separator();
                        ui.label(trf!(
                            "Camada superior: {count} trilhas",
                            count = board
                                .tracks
                                .values()
                                .filter(|track| track.layer == BoardSide::Top)
                                .count()
                        ));
                        ui.label(trf!(
                            "Camada inferior: {count} trilhas",
                            count = board
                                .tracks
                                .values()
                                .filter(|track| track.layer == BoardSide::Bottom)
                                .count()
                        ));
                        ui.label(trf!("{count} vias atravessantes", count = board.vias.len()));
                        ui.checkbox(
                            &mut self.pcb.show_airwires,
                            tr!("Mostrar conexões não roteadas"),
                        );
                    }
                    PcbInspectorTab::Drc => {
                        if ui.button(tr!("Verificar DRC e conexões")).clicked() {
                            match eda_pcb::check(self.engine.project()) {
                                Ok(report) => {
                                    let errors = report.errors();
                                    self.pcb.report = Some(report);
                                    self.message(
                                        trf!("DRC concluído: {errors} erros.", errors = errors),
                                        errors > 0,
                                    );
                                }
                                Err(error) => self.message(error, true),
                            }
                        }
                        if let Some(report) = &self.pcb.report {
                            ui.label(trf!(
                                "{errors} erros · {warnings} avisos",
                                errors = report.errors(),
                                warnings = report.diagnostics.len().saturating_sub(report.errors())
                            ));
                            for diagnostic in report.diagnostics.iter().take(100) {
                                let color = if diagnostic.severity == "error" {
                                    theme.error
                                } else {
                                    theme.muted
                                };
                                ui.colored_label(
                                    color,
                                    trf!(
                                        "{code}: {message}",
                                        code = &diagnostic.code,
                                        message = eda_i18n::text(&diagnostic.message)
                                    ),
                                );
                            }
                        } else {
                            ui.label(
                                egui::RichText::new(tr!("Nenhum diagnóstico disponível."))
                                    .color(theme.muted),
                            );
                        }
                    }
                });
            });
    }

    pub(super) fn start_autorouting(&mut self) {
        if self.pcb.autorouting.is_some() || self.pcb.fabrication.is_some() {
            return;
        }
        let config = eda_pcb::AutorouteConfig {
            track_width_nm: nm(self.pcb.width),
            grid_nm: nm(self.pcb.grid_mm),
            via_diameter_nm: nm(self.pcb.via_diameter_mm),
            via_drill_nm: nm(self.pcb.via_drill_mm),
            ..eda_pcb::AutorouteConfig::default()
        };
        let project = self.engine.project().clone();
        let revision = self.engine.revision();
        let (sender, receiver) = mpsc::channel();
        match std::thread::Builder::new()
            .name("protocraft-autoroute".into())
            .spawn(move || {
                let _ = sender.send(eda_pcb::autoroute(&project, &config));
            }) {
            Ok(_) => {
                self.pcb.autorouting_revision = revision;
                self.pcb.autorouting = Some(receiver);
                self.message(tr!("Buscando rotas em segundo plano…"), false);
            }
            Err(error) => self.message(error.to_string(), true),
        }
    }
    fn pcb_fabrication_controls(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.strong(tr!("Fabricação"));
        if ui.button(tr!("Exportar lista de materiais CSV…")).clicked() {
            self.export_assembly_dialog(false);
        }
        if ui
            .button(tr!("Exportar posições de montagem CSV…"))
            .clicked()
        {
            self.export_assembly_dialog(true);
        }
        if ui.button(tr!("Calcular comprimentos por rede")).clicked() {
            match eda_pcb::metrics::routing_metrics(self.engine.project()) {
                Ok(metrics) => self.pcb.routing_metrics = Some((self.engine.revision(), metrics)),
                Err(error) => self.message(error, true),
            }
        }
        if let Some((revision, metrics)) = &self.pcb.routing_metrics {
            if *revision != self.engine.revision() {
                ui.label(tr!(
                    "Métricas desatualizadas; calcule novamente após editar."
                ));
            } else {
                ui.small(tr!(
                    "Soma geométrica dos segmentos; exclui o comprimento vertical de vias."
                ));
                egui::ScrollArea::vertical()
                    .id_salt("routing_metrics")
                    .max_height(180.0)
                    .show(ui, |ui| {
                        for net in &metrics.nets {
                            ui.label(trf!(
                                "{name}: {length} mm · {vias} vias",
                                name = &net.name,
                                length = format!("{:.3}", net.length_mm),
                                vias = net.vias
                            ));
                        }
                    });
            }
        }
        let before = self.settings.fabrication_preset;
        ui.add_enabled_ui(
            self.pcb.fabrication.is_none() && self.pcb.autorouting.is_none(),
            |ui| {
                egui::ComboBox::from_id_salt("fabrication_preset")
                    .selected_text(eda_i18n::text(self.settings.fabrication_preset.label()))
                    .show_ui(ui, |ui| {
                        for preset in [
                            eda_storage::fabrication::ManufacturingPreset::Jlcpcb,
                            eda_storage::fabrication::ManufacturingPreset::Standard,
                        ] {
                            ui.selectable_value(
                                &mut self.settings.fabrication_preset,
                                preset,
                                eda_i18n::text(preset.label()),
                            );
                        }
                    });
            },
        );
        if before != self.settings.fabrication_preset
            && let Err(e) = self.settings.save()
        {
            self.message(e, true);
        }
        if self.settings.fabrication_preset == eda_storage::fabrication::ManufacturingPreset::Jlcpcb
        {
            ui.small(tr!("RS-274X · Protel · mm · 1:1 · origem absoluta"));
            ui.small(tr!("ZIP com Gerbers e furos Excellon PTH/NPTH."));
        }
        let generate_label = if self.settings.fabrication_preset
            == eda_storage::fabrication::ManufacturingPreset::Jlcpcb
        {
            tr!("Gerar ZIP JLCPCB…")
        } else {
            tr!("Gerar Gerber / Excellon…")
        };
        if ui
            .add_enabled(
                self.pcb.fabrication.is_none() && self.pcb.autorouting.is_none(),
                egui::Button::new(generate_label),
            )
            .clicked()
        {
            self.start_fabrication_dialog();
        }
        if self.pcb.fabrication.is_some() {
            ui.label(tr!("Validando e gerando arquivos em segundo plano…"));
        }
        ui.separator();
    }
    fn export_assembly_dialog(&mut self, placements: bool) {
        let report = eda_pcb::assembly::assembly_report(self.engine.project());
        let contents = if placements {
            eda_pcb::assembly::placements_csv(&report)
        } else {
            eda_pcb::assembly::bom_csv(&report)
        };
        let Some(path) = rfd::FileDialog::new()
            .set_title(tr!("Salvar relatório de montagem em arquivo novo"))
            .add_filter("CSV", &["csv"])
            .set_file_name(if placements {
                "placements.csv"
            } else {
                "bom.csv"
            })
            .save_file()
        else {
            return;
        };
        let result = (|| -> std::io::Result<()> {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?;
            file.write_all(contents.as_bytes())?;
            file.sync_all()
        })();
        match result {
            Ok(()) => self.message(tr!("Relatório exportado. Confira unidades, rotação e componentes pendentes antes de montar."), false),
            Err(error) => self.message(error.to_string(), true),
        }
    }
    fn start_fabrication_dialog(&mut self) {
        match eda_pcb::check(self.engine.project()) {
            Ok(r) if r.errors() > 0 => {
                let count = r.errors();
                self.pcb.report = Some(r);
                self.message(
                    trf!(
                        "Corrija os {count} erros DRC antes de fabricar.",
                        count = count
                    ),
                    true,
                );
                return;
            }
            Err(e) => {
                self.message(e, true);
                return;
            }
            _ => {}
        }
        let tool = match self.settings.kicad_cli.clone().filter(|p| p.is_file()) {
            Some(p) => p,
            None => {
                let Some(path) = rfd::FileDialog::new()
                    .set_title(tr!("Selecione kicad-cli.exe oficial"))
                    .add_filter(tr!("KiCad CLI"), &["exe"])
                    .pick_file()
                else {
                    return;
                };
                self.settings.kicad_cli = Some(path.clone());
                if let Err(e) = self.settings.save() {
                    self.message(e, true);
                }
                path
            }
        };
        let Some(parent) = rfd::FileDialog::new()
            .set_title(tr!("Pasta onde criar o pacote de fabricação"))
            .pick_folder()
        else {
            return;
        };
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        let destination = parent.join(format!("ProtoCraft-fabricacao-{stamp}"));
        let project = self.engine.project().clone();
        let preset = self.settings.fabrication_preset;
        let (sender, receiver) = mpsc::channel();
        match std::thread::Builder::new()
            .name("protocraft-fabrication".into())
            .spawn(move || {
                let _ = sender.send(eda_storage::fabrication::generate_with_preset(
                    &project,
                    &destination,
                    &tool,
                    preset,
                ));
            }) {
            Ok(_) => {
                self.pcb.fabrication = Some(receiver);
                self.message(tr!("Validando DRC e gerando Gerber/Excellon…"), false);
            }
            Err(e) => self.message(e.to_string(), true),
        }
    }
    pub(super) fn pcb_canvas(&mut self, root_ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root_ui, |ui| {
                let (response, painter) =
                    ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
                let rect = response.rect;
                self.pcb.canvas_rect = Some(rect);
                let canvas = Color32::from_rgb(10, 12, 15);
                painter.rect_filled(rect, 0.0, canvas);
                let grid_pixels = (self.pcb.grid_mm as f32 * self.pcb.zoom).clamp(8.0, 42.0);
                let grid_origin = rect.center() + self.pcb.pan;
                let first_x = rect.left() + (grid_origin.x - rect.left()).rem_euclid(grid_pixels);
                let first_y = rect.top() + (grid_origin.y - rect.top()).rem_euclid(grid_pixels);
                let mut x = first_x;
                let mut column = ((first_x - grid_origin.x) / grid_pixels).round() as i32;
                while x <= rect.right() {
                    let major = column.rem_euclid(5) == 0;
                    painter.line_segment(
                        [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                        Stroke::new(
                            if major { 1.0_f32 } else { 0.55_f32 },
                            if major {
                                Color32::from_rgb(51, 56, 62)
                            } else {
                                Color32::from_rgb(28, 32, 37)
                            },
                        ),
                    );
                    x += grid_pixels;
                    column += 1;
                }
                let mut y = first_y;
                let mut row = ((first_y - grid_origin.y) / grid_pixels).round() as i32;
                while y <= rect.bottom() {
                    let major = row.rem_euclid(5) == 0;
                    painter.line_segment(
                        [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                        Stroke::new(
                            if major { 1.0_f32 } else { 0.55_f32 },
                            if major {
                                Color32::from_rgb(51, 56, 62)
                            } else {
                                Color32::from_rgb(28, 32, 37)
                            },
                        ),
                    );
                    y += grid_pixels;
                    row += 1;
                }
                let Some(board) = self.engine.project().board.as_deref().cloned() else {
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        tr!("Crie ou sincronize a PCB na lateral."),
                        egui::FontId::proportional(18.0),
                        Palette::new(ctx).muted,
                    );
                    return;
                };
                if self.pcb.fit {
                    self.pcb.zoom = ((rect.width() - 80.0) / (board.width_nm as f32 / 1e6))
                        .min((rect.height() - 80.0) / (board.height_nm as f32 / 1e6))
                        .clamp(0.1, 100.0);
                    self.pcb.pan =
                        Vec2::new(-board.width_nm as f32 / 2e6, -board.height_nm as f32 / 2e6)
                            * self.pcb.zoom;
                    self.pcb.fit = false;
                }
                if response.hovered() {
                    let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
                    if scroll != 0.0
                        && let Some(pos) = response.hover_pos()
                    {
                        let before = pos - rect.center() - self.pcb.pan;
                        let old = self.pcb.zoom;
                        self.pcb.zoom = (old * (scroll * 0.002).exp()).clamp(0.1, 300.0);
                        self.pcb.pan += before * (1.0 - self.pcb.zoom / old);
                    }
                }
                if response.dragged_by(egui::PointerButton::Middle)
                    || response.dragged_by(egui::PointerButton::Secondary)
                {
                    self.pcb.pan += ctx.input(|i| i.pointer.delta());
                }
                let zoom = self.pcb.zoom;
                let origin = rect.center() + self.pcb.pan;
                let screen =
                    |p: NmPoint| origin + Vec2::new(p.x as f32 / 1e6, p.y as f32 / 1e6) * zoom;
                let grid = nm(self.pcb.grid_mm);
                let world_raw = |p: Pos2| {
                    let p = (p - origin) / zoom;
                    NmPoint::new(
                        nm(p.x as f64).clamp(-eda_core::PHYSICAL_LIMIT, eda_core::PHYSICAL_LIMIT),
                        nm(p.y as f64).clamp(-eda_core::PHYSICAL_LIMIT, eda_core::PHYSICAL_LIMIT),
                    )
                };
                let world = |p: Pos2| {
                    let p = world_raw(p);
                    NmPoint::new(
                        (p.x as f64 / grid as f64).round() as i64 * grid,
                        (p.y as f64 / grid as f64).round() as i64 * grid,
                    )
                };
                let outline = Rect::from_min_max(
                    screen(NmPoint::new(0, 0)),
                    screen(NmPoint::new(board.width_nm, board.height_nm)),
                );
                painter.rect_stroke(
                    outline,
                    egui::CornerRadius::same(
                        (board.corner_radius_nm as f32 / 1_000_000.0 * zoom)
                            .round()
                            .clamp(0.0, 255.0) as u8,
                    ),
                    Stroke::new(1.5_f32, Color32::from_rgb(39, 108, 184)),
                    egui::StrokeKind::Inside,
                );
                if self.pcb.report_revision != self.engine.revision() {
                    self.pcb.report_revision = self.engine.revision();
                    match eda_pcb::check(self.engine.project()) {
                        Ok(r) => self.pcb.report = Some(r),
                        Err(e) => {
                            self.pcb.report = None;
                            self.message(e, true);
                        }
                    }
                }
                let pads = match eda_pcb::placed_pads(self.engine.project()) {
                    Ok(p) => p,
                    Err(e) => {
                        self.message(e, true);
                        return;
                    }
                };
                let mut bodies = Vec::new();
                for (id, place) in &board.placements {
                    let c = &self.engine.project().components[id];
                    let Some(f) = &c.footprint else {
                        continue;
                    };
                    let min = f.body_bounds.min;
                    let max = f.body_bounds.max;
                    let body: Vec<_> = [
                        min,
                        NmPoint::new(max.x, min.y),
                        max,
                        NmPoint::new(min.x, max.y),
                    ]
                    .iter()
                    .map(|p| screen(place.transform(*p)))
                    .collect();
                    let bounds = Rect::from_points(&body);
                    bodies.push((*id, bounds));
                    painter.add(egui::Shape::convex_polygon(
                        body,
                        Color32::from_rgba_unmultiplied(28, 29, 31, 110),
                        Stroke::new(
                            if self.pcb.selected == Some(*id) {
                                2.0_f32
                            } else {
                                1.0_f32
                            },
                            if self.pcb.selected == Some(*id) {
                                Palette::new(ctx).gold
                            } else {
                                Color32::from_gray(180)
                            },
                        ),
                    ));
                    for l in &f.lines {
                        if matches!(
                            l.layer.as_str(),
                            "F.Fab" | "Eco2.User" | "F.CrtYd" | "F.SilkS"
                        ) {
                            painter.line_segment(
                                [
                                    screen(place.transform(l.start)),
                                    screen(place.transform(l.end)),
                                ],
                                Stroke::new(
                                    (l.width_nm as f32 / 1e6 * zoom).max(0.5),
                                    if l.layer == "F.CrtYd" {
                                        Color32::from_gray(100)
                                    } else {
                                        Color32::from_gray(210)
                                    },
                                ),
                            );
                        }
                    }
                    for k in &f.keepouts {
                        let poly: Vec<_> = k
                            .polygon
                            .iter()
                            .map(|p| screen(place.transform(*p)))
                            .collect();
                        if poly.len() >= 3 {
                            painter.add(egui::Shape::convex_polygon(
                                poly,
                                Color32::from_rgba_unmultiplied(240, 90, 80, 45),
                                Stroke::new(1.0_f32, Color32::from_rgb(230, 115, 80)),
                            ));
                        }
                    }
                    painter.text(
                        bounds.center_top() - Vec2::new(0.0, 5.0),
                        egui::Align2::CENTER_BOTTOM,
                        format!(
                            "{} · {}",
                            c.reference,
                            c.device.as_ref().map_or_else(|| localized_component_label(c.kind), |d| d.id.clone())
                        ),
                        egui::FontId::monospace(11.0),
                        Color32::WHITE,
                    );
                }
                for p in &pads {
                    if p.layers == 0 {
                        continue;
                    }
                    for polygon in &p.polygons {
                        let poly: Vec<_> = polygon.iter().map(|p| screen(*p)).collect();
                        painter.add(egui::Shape::convex_polygon(
                            poly,
                            if p.layers & 1 != 0 {
                                Color32::from_rgb(230, 186, 79)
                            } else {
                                Color32::from_rgb(100, 157, 233)
                            },
                            Stroke::NONE,
                        ));
                    }
                    if let Some(d) = p.pad.drill {
                        painter.circle_filled(
                            screen(p.position),
                            d.x as f32 / 2e6 * zoom,
                            Color32::from_gray(20),
                        );
                    }
                    if zoom > 20.0 {
                        painter.text(
                            screen(p.position),
                            egui::Align2::CENTER_CENTER,
                            &p.number,
                            egui::FontId::monospace(8.0),
                            Color32::BLACK,
                        );
                    }
                }
                for t in board.tracks.values() {
                    let color = if self.pcb.selected_track == Some(t.id) {
                        Color32::WHITE
                    } else if t.layer == BoardSide::Top {
                        Color32::from_rgb(232, 101, 107)
                    } else {
                        Color32::from_rgb(90, 151, 230)
                    };
                    let width = (t.width_nm as f32 / 1e6 * zoom).max(1.0);
                    painter.line_segment(
                        [screen(t.start), screen(t.end)],
                        Stroke::new(width, color),
                    );
                    painter.circle_filled(screen(t.start), width * 0.5, color);
                    painter.circle_filled(screen(t.end), width * 0.5, color);
                }
                // Repaint the bore above copper so tracks cannot visually fill the holes.
                for pad in pads.iter().filter(|pad| pad.pad.drill.is_some()) {
                    let drill = pad.pad.drill.expect("filtered pad has a drill");
                    painter.circle_filled(
                        screen(pad.position),
                        drill.x.max(drill.y) as f32 / 2e6 * zoom,
                        Color32::from_gray(20),
                    );
                }
                for v in board.vias.values() {
                    painter.circle_filled(
                        screen(v.position),
                        v.diameter_nm as f32 / 2e6 * zoom,
                        Palette::new(ctx).gold,
                    );
                    painter.circle_filled(
                        screen(v.position),
                        v.drill_nm as f32 / 2e6 * zoom,
                        Color32::from_gray(20),
                    );
                }
                for h in board.holes.values() {
                    let pos = screen(h.position);
                    let radius = h.drill_nm as f32 / 2e6 * zoom;
                    painter.circle_filled(pos, radius, Color32::from_gray(18));
                    painter.circle_stroke(
                        pos,
                        radius,
                        Stroke::new(
                            if self.pcb.selected_hole == Some(h.id) {
                                2.0_f32
                            } else {
                                1.0_f32
                            },
                            if self.pcb.selected_hole == Some(h.id) {
                                Color32::WHITE
                            } else {
                                Color32::from_gray(170)
                            },
                        ),
                    );
                    painter.line_segment(
                        [pos - Vec2::new(3.0, 0.0), pos + Vec2::new(3.0, 0.0)],
                        Stroke::new(1.0_f32, Color32::from_gray(150)),
                    );
                    painter.line_segment(
                        [pos - Vec2::new(0.0, 3.0), pos + Vec2::new(0.0, 3.0)],
                        Stroke::new(1.0_f32, Color32::from_gray(150)),
                    );
                    if zoom > 8.0 {
                        painter.text(
                            pos + Vec2::new(0.0, radius + 4.0),
                            egui::Align2::CENTER_TOP,
                            trf!("Furo {id} · Ø{drill}", id=h.id, drill=format!("{:.2}",h.drill_nm as f64 / 1e6)),
                            egui::FontId::monospace(10.0),
                            Color32::from_gray(210),
                        );
                    }
                }
                if let Some(id) = self.pcb.selected_via
                    && let Some(v) = board.vias.get(&id)
                {
                    painter.circle_stroke(
                        screen(v.position),
                        v.diameter_nm as f32 / 2e6 * zoom + 2.0,
                        Stroke::new(2.0_f32, Color32::WHITE),
                    );
                }
                if self.pcb.show_airwires
                    && let Some(report) = &self.pcb.report
                {
                    for a in &report.airwires {
                        painter.line_segment(
                            [screen(a.start), screen(a.end)],
                            Stroke::new(0.8_f32, Color32::from_rgb(162, 187, 141)),
                        );
                    }
                }
                let hover = response.hover_pos();
                let tolerance = (7.0 / zoom as f64 * 1e6).min(2_000_000.0) as i64;
                let hover_target = hover.and_then(|pos| {
                    eda_pcb::routing_target(
                        self.engine.project(),
                        &pads,
                        &self.netlist,
                        self.pcb.layer,
                        world_raw(pos),
                        tolerance,
                    )
                });
                let pad_at = |pos: Pos2| {
                    pads.iter()
                        .filter(|p| p.layers != 0)
                        .min_by(|a, b| {
                            screen(a.position)
                                .distance_sq(pos)
                                .total_cmp(&screen(b.position).distance_sq(pos))
                        })
                        .filter(|p| {
                            screen(p.position).distance(pos)
                                < (p.pad.size.x.min(p.pad.size.y) as f32 / 2e6 * zoom).max(7.0)
                        })
                };
                if let Some(pos) = hover {
                    if let Some(pad) = pad_at(pos) {
                        let c = &self.engine.project().components[&pad.component];
                        response.clone().on_hover_text(trf!(
                            "Referência {reference} · pad {pad}\nPino: {pin_name}\nX {x} · Y {y} mm",
                            reference = &c.reference,
                            pad = pad.number,
                            pin_name = pad.pin.map_or("", |p| c.pin_name(p.pin)),
                            x = format!("{:.3}", pad.position.x as f64 / 1e6),
                            y = format!("{:.3}", pad.position.y as f64 / 1e6)
                        ));
                    }
                    let point = hover_target.map_or_else(|| world(pos), |t| t.position);
                    painter.text(
                        rect.right_bottom() - Vec2::splat(12.0),
                        egui::Align2::RIGHT_BOTTOM,
                        trf!("X {x} · Y {y} mm · grade {grid} mm · {layer}", x=format!("{:.3}", point.x as f64 / 1e6), y=format!("{:.3}", point.y as f64 / 1e6), grid=self.pcb.grid_mm, layer=eda_i18n::text(if self.pcb.layer == BoardSide::Top { "Superior" } else { "Inferior" })),
                        egui::FontId::monospace(12.0),
                        Palette::new(ctx).muted,
                    );
                    if self.pcb.hole_tool {
                        painter.circle_stroke(
                            screen(world(pos)),
                            self.pcb.hole_diameter_mm as f32 / 2.0 * zoom,
                            Stroke::new(1.5_f32, Palette::new(ctx).gold),
                        );
                    }
                    if self.pcb.routing {
                        let color = if let Some((_, anchor)) = self.pcb.route
                            && hover_target
                                .is_some_and(|t| Some(t.net) != self.netlist.net_for_pin(anchor))
                        {
                            Color32::from_rgb(255, 80, 80)
                        } else {
                            Palette::new(ctx).gold
                        };
                        if let Some((start, _)) = self.pcb.route {
                            for pair in eda_pcb::manual_path(
                                start,
                                point,
                                self.pcb.route_mode,
                                self.pcb.route_flip,
                            )
                            .windows(2)
                            {
                                painter.line_segment(
                                    [screen(pair[0]), screen(pair[1])],
                                    Stroke::new((self.pcb.width as f32 * zoom).max(1.0), color),
                                );
                            }
                        }
                        if hover_target.is_some() {
                            painter.circle_stroke(screen(point), 6.0, Stroke::new(1.5_f32, color));
                        }
                    }
                    if let Some((selection, press)) = self.pcb.geometry_drag {
                        let end = world(pos);
                        let delta = NmPoint::new(end.x - press.x, end.y - press.y);
                        if let Some(command) =
                            self.pcb_geometry_drag_command(&board, selection, delta)
                        {
                            match command {
                                Command::UpdateTrack { track } => {
                                    painter.line_segment(
                                        [screen(track.start), screen(track.end)],
                                        Stroke::new(
                                            (track.width_nm as f32 / 1e6 * zoom).max(1.0),
                                            Palette::new(ctx).gold,
                                        ),
                                    );
                                }
                                Command::UpdateVia { via } => {
                                    painter.circle_stroke(
                                        screen(via.position),
                                        via.diameter_nm as f32 / 2e6 * zoom,
                                        Stroke::new(1.5_f32, Palette::new(ctx).gold),
                                    );
                                }
                                Command::UpdateHole { hole } => {
                                    painter.circle_stroke(
                                        screen(hole.position),
                                        hole.drill_nm as f32 / 2e6 * zoom,
                                        Stroke::new(1.5_f32, Palette::new(ctx).gold),
                                    );
                                }
                                _ => {}
                            };
                        }
                    }
                }
                if response.secondary_clicked() {
                    self.pcb.finish_route();
                }
                if std::mem::take(&mut self.pcb.via_requested) {
                    if let Some(pos) = hover {
                        let point = hover_target.map_or_else(|| world(pos), |t| t.position);
                        self.pcb_route_to(point, hover_target, true);
                    } else {
                        self.message(tr!("Posicione o cursor sobre a PCB para inserir a via."), false);
                    }
                }
                if response.clicked()
                    && let Some(pos) = response.interact_pointer_pos()
                {
                    let target = eda_pcb::routing_target(
                        self.engine.project(),
                        &pads,
                        &self.netlist,
                        self.pcb.layer,
                        world_raw(pos),
                        tolerance,
                    );
                    if self.pcb.hole_tool {
                        self.pcb_add_hole(world(pos));
                    } else if self.pcb.routing {
                        if self.pcb.route.is_some() {
                            self.pcb_route_to(
                                target.map_or_else(|| world(pos), |t| t.position),
                                target,
                                false,
                            );
                        } else if let Some(target) = target {
                            self.pcb.route = Some((target.position, target.anchor));
                            self.pcb.route_history.clear();
                            self.pcb.selected = None;
                            self.pcb.clear_geometry_selection();
                        } else {
                            self.message(tr!("Inicie em pad, via ou trilha da camada ativa."), false);
                        }
                    } else if let Some(selection) =
                        self.pcb
                            .geometry_at(&board, world_raw(pos), tolerance as f64)
                    {
                        self.pcb_select_geometry(Some(selection));
                    } else if let Some((id, _)) = bodies.iter().rev().find(|(_, b)| b.contains(pos))
                    {
                        self.pcb.clear_geometry_selection();
                        self.pcb.selected = Some(*id);
                    } else if let Some(id) = self
                        .pcb
                        .selected
                        .filter(|id| !board.placements.contains_key(id))
                    {
                        self.command(Command::PlaceOnBoard {
                            id,
                            placement: Placement {
                                position: world(pos),
                                rotation: 0,
                                side: self.pcb.layer,
                            },
                        });
                        self.pcb.report = None;
                    } else {
                        self.pcb_select_geometry(None);
                    }
                }
                if !self.pcb.routing
                    && !self.pcb.hole_tool
                    && response.drag_started_by(egui::PointerButton::Primary)
                    && let Some(pos) = ctx.input(|i| i.pointer.press_origin())
                {
                    if let Some(selection) =
                        self.pcb
                            .geometry_at(&board, world_raw(pos), tolerance as f64)
                    {
                        self.pcb_select_geometry(Some(selection));
                        self.pcb.geometry_drag = Some((selection, world(pos)));
                        self.pcb.drag = None;
                    } else if let Some((id, _)) = bodies.iter().rev().find(|(_, b)| b.contains(pos))
                    {
                        self.pcb.clear_geometry_selection();
                        self.pcb.selected = Some(*id);
                        self.pcb.drag = Some((*id, board.placements[id].position, world(pos)));
                    }
                }
                if response.drag_stopped_by(egui::PointerButton::Primary) {
                    if let Some((selection, start)) = self.pcb.geometry_drag.take()
                        && let Some(pos) = response.interact_pointer_pos()
                    {
                        let end = world(pos);
                        if end != start
                            && let Some(command) = self.pcb_geometry_drag_command(
                                &board,
                                selection,
                                NmPoint::new(end.x - start.x, end.y - start.y),
                            )
                        {
                            self.pcb_checked_command(command);
                        }
                    }
                    if let Some((id, start, press)) = self.pcb.drag.take()
                        && let Some(pos) = response.interact_pointer_pos()
                        && let Some(mut p) = board.placements.get(&id).copied()
                    {
                        let end = world(pos);
                        p.position =
                            NmPoint::new(start.x + end.x - press.x, start.y + end.y - press.y);
                        self.move_board_component(id, p);
                    }
                }
                painter.text(
                    rect.left_top() + Vec2::splat(15.0),
                    egui::Align2::LEFT_TOP,
                    trf!("PCB · {width} × {height} mm · {footprints} footprints · {tracks} trilhas · {vias} vias · {holes} furos", width=format!("{:.2}", board.width_nm as f64 / 1e6), height=format!("{:.2}", board.height_nm as f64 / 1e6), footprints=board.placements.len(), tracks=board.tracks.len(), vias=board.vias.len(), holes=board.holes.len()),
                    egui::FontId::proportional(14.0),
                    Palette::new(ctx).muted,
                );
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pcb_drag_moves_only_physical_placement_and_is_undoable() {
        let project = eda_library::physical_example().unwrap();
        let mut app = ProtoCraft::new(
            Engine::new(project).unwrap(),
            None,
            None,
            None,
            false,
            None,
            false,
        );
        app.pcb.active = true;
        let ctx = egui::Context::default();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));
        let frame = |app: &mut ProtoCraft, events, time| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(rect),
                    events,
                    time: Some(time),
                    ..Default::default()
                },
                |ui| {
                    let ctx = ui.ctx().clone();
                    app.pcb_canvas(ui, &ctx);
                },
            );
            output.textures_delta.clear();
        };
        frame(&mut app, vec![], 0.0);
        let id = ComponentId(1);
        let before = app.engine.project().board.as_ref().unwrap().placements[&id];
        let schematic = app.engine.project().components[&id].position;
        let pos = rect.center()
            + app.pcb.pan
            + Vec2::new(
                before.position.x as f32 / 1e6,
                before.position.y as f32 / 1e6,
            ) * app.pcb.zoom;
        frame(
            &mut app,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
            0.1,
        );
        let end = pos + Vec2::new(50.0, 25.0);
        frame(&mut app, vec![egui::Event::PointerMoved(end)], 0.2);
        frame(
            &mut app,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
            0.3,
        );
        assert_ne!(
            app.engine.project().board.as_ref().unwrap().placements[&id],
            before
        );
        assert_eq!(app.engine.project().components[&id].position, schematic);
        app.engine.undo();
        assert_eq!(
            app.engine.project().board.as_ref().unwrap().placements[&id],
            before
        );
    }
    #[test]
    fn sync_keeps_existing_placements_and_device_copy_preserves_pin_pad_data() {
        let project = eda_library::physical_example().unwrap();
        let mut app = ProtoCraft::new(
            Engine::new(project).unwrap(),
            None,
            None,
            None,
            false,
            None,
            false,
        );
        let before = app
            .engine
            .project()
            .board
            .as_ref()
            .unwrap()
            .placements
            .clone();
        app.sync_board();
        assert_eq!(
            app.engine.project().board.as_ref().unwrap().placements,
            before
        );
        let original = app.engine.project().components[&ComponentId(1)].clone();
        let id = ComponentId(app.engine.project().next_id);
        app.command(Command::PasteComponent {
            component: original.clone(),
            position: Point::new(100, 100),
        });
        let copy = &app.engine.project().components[&id];
        assert_eq!(copy.device, original.device);
        assert_eq!(copy.footprint, original.footprint);
        assert_ne!(copy.reference, original.reference);
        app.sync_board();
        assert!(
            app.engine
                .project()
                .board
                .as_ref()
                .unwrap()
                .placements
                .contains_key(&id)
        );
        for (id, p) in before {
            assert_eq!(
                app.engine.project().board.as_ref().unwrap().placements[&id],
                p
            );
        }
    }
}
