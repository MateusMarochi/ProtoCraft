use super::*;
use eda_core::{
    Board, BoardSide, ComponentId, MountingHole, NM_PER_MM, NmPoint, PinRef, Placement, Track, Via,
};
mod edit;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GeometrySelection {
    Track(u64),
    Via(u64),
    Hole(u64),
}

pub(super) struct PcbUi {
    pub active: bool,
    pub fit: bool,
    pub report: Option<eda_pcb::PcbReport>,
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
    layer: BoardSide,
    width: f64,
    zoom: f32,
    pan: Vec2,
    drag: Option<(ComponentId, NmPoint, NmPoint)>,
    routing: bool,
    width_mm: f64,
    height_mm: f64,
    clearance_mm: f64,
    dimensions_revision: u64,
    show_airwires: bool,
    fabrication: Option<mpsc::Receiver<Result<PathBuf, String>>>,
}

impl Default for PcbUi {
    fn default() -> Self {
        Self {
            active: false,
            fit: true,
            report: None,
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
            dimensions_revision: u64::MAX,
            show_airwires: true,
            fabrication: None,
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
    pub(super) fn sync_board(&mut self) {
        self.pcb.finish_route();
        let board = self
            .engine
            .project()
            .board
            .as_deref()
            .cloned()
            .unwrap_or_default();
        let mut commands = vec![Command::SetBoard {
            board: board.clone(),
        }];
        let (mut x, mut y, mut row_height) = (5_000_000, 5_000_000, 0);
        // Preserva posições existentes; novos footprints entram em uma prateleira livre abaixo delas.
        for (id, p) in &board.placements {
            if let Some(f) = self
                .engine
                .project()
                .components
                .get(id)
                .and_then(|c| c.footprint.as_ref())
            {
                y = y.max(p.position.y + f.body_bounds.height() + 5_000_000);
            }
        }
        for c in self.engine.project().components.values() {
            if board.placements.contains_key(&c.id) {
                continue;
            }
            if let Some(f) = &c.footprint {
                let w = f.body_bounds.width() + 6_000_000;
                let h = f.body_bounds.height() + 6_000_000;
                if x + w > board.width_nm {
                    x = 5_000_000;
                    y += row_height;
                    row_height = 0;
                }
                commands.push(Command::PlaceOnBoard {
                    id: c.id,
                    placement: Placement {
                        position: NmPoint::new(
                            x - f.body_bounds.min.x + 3_000_000,
                            y - f.body_bounds.min.y + 3_000_000,
                        ),
                        rotation: 0,
                        side: BoardSide::Top,
                    },
                });
                x += w;
                row_height = row_height.max(h);
            }
        }
        self.command(Command::Batch(commands));
        self.pcb.fit = true;
        self.pcb.report = None;
        self.message(
            "PCB sincronizada. Posições existentes preservadas; confira o DRC.",
            false,
        );
    }
    pub(super) fn pcb_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() || self.pending.is_some() {
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
            .add_filter("PCB", &[ext])
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
                Ok(()) => self.message(format!("PCB exportada: {}", path.display()), false),
                Err(e) => self.message(e, true),
            }
        }
    }
    pub(super) fn pcb_tools(&mut self, ctx: &egui::Context) {
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
                            format!("ZIP para JLCPCB gerado: {}", zip.display())
                        } else {
                            format!("Gerber e Excellon gerados: {}", path.display())
                        },
                        false,
                    );
                }
                Err(e) => self.message(e, true),
            }
        }
        if self.pcb.fabrication.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }

        egui::SidePanel::left("pcb_tools").default_width(310.0).width_range(285.0..=440.0).show(ctx,|ui| {
            egui::ScrollArea::vertical().show(ui,|ui| {
                ui.heading("PCB · duas camadas");ui.small("Dimensões e furação em milímetros.");
                if ui.button("Voltar ao esquemático").clicked() {self.pcb.active=false;return;}
                if ui.button("Criar / sincronizar footprints").clicked() {self.sync_board();}
                let Some(board)=self.engine.project().board.as_deref().cloned() else {ui.label("Crie a PCB para adicionar trilhas e furos.");return;};
                ui.separator();self.pcb_edit_controls(ui,&board);
                egui::CollapsingHeader::new("Dimensões e regras da placa").show(ui,|ui| {
                    if self.pcb.dimensions_revision!=self.engine.revision() {self.pcb.width_mm=board.width_nm as f64/1e6;self.pcb.height_mm=board.height_nm as f64/1e6;self.pcb.clearance_mm=board.clearance_nm as f64/1e6;self.pcb.dimensions_revision=self.engine.revision();}
                    ui.horizontal(|ui|{ui.label("Largura");ui.add(egui::DragValue::new(&mut self.pcb.width_mm).speed(1.0).range(1.0..=2000.0));});
                    ui.horizontal(|ui|{ui.label("Altura");ui.add(egui::DragValue::new(&mut self.pcb.height_mm).speed(1.0).range(1.0..=2000.0));});
                    ui.horizontal(|ui|{ui.label("Folga de cobre");ui.add(egui::DragValue::new(&mut self.pcb.clearance_mm).speed(0.025).range(0.0..=10.0));});
                    if ui.button("Aplicar dimensões e folga").clicked() {let mut b=board.clone();b.width_nm=nm(self.pcb.width_mm);b.height_nm=nm(self.pcb.height_mm);b.clearance_nm=nm(self.pcb.clearance_mm);self.command(Command::SetBoard{board:b});self.pcb.finish_route();self.pcb.fit=true;self.pcb.report=None;}
                });
                if ui.button("Enquadrar · F").clicked() {self.pcb.fit=true;}
                egui::CollapsingHeader::new("Fabricação e exportação").show(ui,|ui| {
                    self.pcb_fabrication_controls(ui);
                    if ui.button("Exportar PCB KiCad…").clicked(){self.export_pcb_dialog(false);}
                    if ui.button("Exportar desenho SVG em escala…").clicked(){self.export_pcb_dialog(true);}
                });
                egui::CollapsingHeader::new("Componentes e posicionamento").show(ui,|ui| {
                    let rows:Vec<_>=self.engine.project().components.values().filter(|c|c.footprint.is_some()).map(|c|(c.id,c.reference.clone(),c.device.as_ref().map_or(c.kind.label(),|d|d.id.as_str()).to_owned())).collect();
                    for (id,reference,name) in rows {if ui.selectable_label(self.pcb.selected==Some(id),format!("{reference} · {name}")).clicked() {self.pcb.clear_geometry_selection();self.pcb.selected=Some(id);self.pcb.finish_route();self.pcb.routing=false;self.pcb.hole_tool=false;}}
                    if let Some(id)=self.pcb.selected && let Some(c)=self.engine.project().components.get(&id).cloned() {
                        if let Some(f)=&c.footprint {ui.small(&f.id);ui.label(format!("Corpo {:.3} × {:.3} mm",f.body_bounds.width() as f64/1e6,f.body_bounds.height() as f64/1e6));}
                        if let Some(p)=board.placements.get(&id) {ui.label(format!("X {:.3} · Y {:.3} mm · {}°",p.position.x as f64/1e6,p.position.y as f64/1e6,u16::from(p.rotation)*90));
                            if ui.button("Girar 90° · R").clicked(){self.rotate_board_selection();}
                            if ui.button("Trocar face").clicked(){let mut p=*p;p.side=if p.side==BoardSide::Top{BoardSide::Bottom}else{BoardSide::Top};self.command(Command::PlaceOnBoard{id,placement:p});self.pcb.report=None;}
                        } else {ui.label("Clique no canvas para posicionar.");}
                    }
                });
                if let Some(report)=&self.pcb.report {egui::CollapsingHeader::new("Diagnósticos DRC").show(ui,|ui| {for d in report.diagnostics.iter().take(80) {ui.colored_label(if d.severity=="error" {egui::Color32::from_rgb(221,92,87)} else {Palette::new(ctx).muted},format!("{}: {}",d.code,d.message));}});}
                ui.small("As trilhas usam as redes do esquemático. DRC é conferido antes de confirmar cobre e furos.");
            });
        });
    }
    fn pcb_fabrication_controls(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.strong("Fabricação");
        let before = self.settings.fabrication_preset;
        ui.add_enabled_ui(self.pcb.fabrication.is_none(), |ui| {
            egui::ComboBox::from_id_salt("fabrication_preset")
                .selected_text(self.settings.fabrication_preset.label())
                .show_ui(ui, |ui| {
                    for preset in [
                        eda_storage::fabrication::ManufacturingPreset::Jlcpcb,
                        eda_storage::fabrication::ManufacturingPreset::Standard,
                    ] {
                        ui.selectable_value(
                            &mut self.settings.fabrication_preset,
                            preset,
                            preset.label(),
                        );
                    }
                });
        });
        if before != self.settings.fabrication_preset
            && let Err(e) = self.settings.save()
        {
            self.message(e, true);
        }
        if self.settings.fabrication_preset == eda_storage::fabrication::ManufacturingPreset::Jlcpcb
        {
            ui.small("RS-274X · Protel · mm · 1:1 · origem absoluta");
            ui.small("ZIP com Gerbers e furos Excellon PTH/NPTH.");
        }
        let generate_label = if self.settings.fabrication_preset
            == eda_storage::fabrication::ManufacturingPreset::Jlcpcb
        {
            "Gerar ZIP JLCPCB…"
        } else {
            "Gerar Gerber / Excellon…"
        };
        if ui
            .add_enabled(
                self.pcb.fabrication.is_none(),
                egui::Button::new(generate_label),
            )
            .clicked()
        {
            self.start_fabrication_dialog();
        }
        if self.pcb.fabrication.is_some() {
            ui.label("Validando e gerando arquivos em segundo plano…");
        }
        ui.separator();
    }
    fn start_fabrication_dialog(&mut self) {
        match eda_pcb::check(self.engine.project()) {
            Ok(r) if r.errors() > 0 => {
                let count = r.errors();
                self.pcb.report = Some(r);
                self.message(
                    format!("Corrija os {count} erros DRC antes de fabricar."),
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
                    .set_title("Selecione kicad-cli.exe oficial")
                    .add_filter("KiCad CLI", &["exe"])
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
            .set_title("Pasta onde criar o pacote de fabricação")
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
                self.message("Validando DRC e gerando Gerber/Excellon…", false);
            }
            Err(e) => self.message(e.to_string(), true),
        }
    }
    pub(super) fn pcb_canvas(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let (response, painter) =
                    ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
                let rect = response.rect;
                self.pcb.canvas_rect = Some(rect);
                painter.rect_filled(
                    rect,
                    0.0,
                    if self.dark {
                        Color32::from_rgb(22, 28, 28)
                    } else {
                        Color32::from_rgb(230, 234, 229)
                    },
                );
                let Some(board) = self.engine.project().board.as_deref().cloned() else {
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Crie ou sincronize a PCB na lateral.",
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
                painter.rect_filled(outline, 0.0, Color32::from_rgb(25, 72, 52));
                painter.rect_stroke(
                    outline,
                    0.0,
                    Stroke::new(2.0_f32, Color32::from_rgb(189, 198, 162)),
                    egui::StrokeKind::Inside,
                );
                let mut grid_step = grid;
                while grid_step as f32 / 1e6 * zoom < 8.0 {
                    grid_step *= 2;
                }
                let visible_min = world_raw(rect.left_top());
                let visible_max = world_raw(rect.right_bottom());
                let minx = visible_min.x.max(0) / grid_step * grid_step;
                let miny = visible_min.y.max(0) / grid_step * grid_step;
                let maxx = visible_max.x.min(board.width_nm);
                let maxy = visible_max.y.min(board.height_nm);
                if minx <= maxx && miny <= maxy {
                    for x in (minx..=maxx).step_by(grid_step as usize).take(1000) {
                        for y in (miny..=maxy).step_by(grid_step as usize).take(1000) {
                            painter.circle_filled(
                                screen(NmPoint::new(x, y)),
                                0.65,
                                Color32::from_rgba_unmultiplied(180, 200, 180, 45),
                            );
                        }
                    }
                }
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
                            c.device.as_ref().map_or(c.kind.label(), |d| d.id.as_str())
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
                    painter.line_segment(
                        [screen(t.start), screen(t.end)],
                        Stroke::new(
                            (t.width_nm as f32 / 1e6 * zoom).max(1.0),
                            if self.pcb.selected_track == Some(t.id) {
                                Color32::WHITE
                            } else if t.layer == BoardSide::Top {
                                Color32::from_rgb(232, 101, 107)
                            } else {
                                Color32::from_rgb(90, 151, 230)
                            },
                        ),
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
                            format!("H{} · Ø{:.2}", h.id, h.drill_nm as f64 / 1e6),
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
                        response.clone().on_hover_text(format!(
                            "{} · pad {}\n{}\nX {:.3} · Y {:.3} mm",
                            c.reference,
                            pad.number,
                            pad.pin.map_or("", |p| c.pin_name(p.pin)),
                            pad.position.x as f64 / 1e6,
                            pad.position.y as f64 / 1e6
                        ));
                    }
                    let point = hover_target.map_or_else(|| world(pos), |t| t.position);
                    painter.text(
                        rect.right_bottom() - Vec2::splat(12.0),
                        egui::Align2::RIGHT_BOTTOM,
                        format!(
                            "X {:.3} · Y {:.3} mm · grade {} mm · {}",
                            point.x as f64 / 1e6,
                            point.y as f64 / 1e6,
                            self.pcb.grid_mm,
                            if self.pcb.layer == BoardSide::Top {
                                "Superior"
                            } else {
                                "Inferior"
                            }
                        ),
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
                        self.message("Posicione o cursor sobre a PCB para inserir a via.", false);
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
                            self.message("Inicie em pad, via ou trilha da camada ativa.", false);
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
                        self.command(Command::PlaceOnBoard { id, placement: p });
                        self.pcb.report = None;
                    }
                }
                painter.text(
                    rect.left_top() + Vec2::splat(15.0),
                    egui::Align2::LEFT_TOP,
                    format!(
                        "PCB · {:.2} × {:.2} mm · {} footprints · {} trilhas · {} vias · {} furos",
                        board.width_nm as f64 / 1e6,
                        board.height_nm as f64 / 1e6,
                        board.placements.len(),
                        board.tracks.len(),
                        board.vias.len(),
                        board.holes.len()
                    ),
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
        let mut app = ProtoCraft::new(Engine::new(project).unwrap(), None, None, None, false, None);
        app.pcb.active = true;
        let ctx = egui::Context::default();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));
        let frame = |app: &mut ProtoCraft, events, time| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(rect),
                    events,
                    time: Some(time),
                    ..Default::default()
                },
                |ctx| app.pcb_canvas(ctx),
            );
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
        let mut app = ProtoCraft::new(Engine::new(project).unwrap(), None, None, None, false, None);
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
