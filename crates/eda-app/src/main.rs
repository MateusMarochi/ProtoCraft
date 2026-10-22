mod clipboard;
mod device_ui;
mod geometry;
mod live;
mod pcb_ui;
mod settings;
mod simulation_ui;

use eda_core::{Command, Component, ComponentKind, Engine, ObjectId, Point, Project, examples};
use eda_netlist::{Diagnostic, Netlist, Severity, build_netlist, erc};
use eda_simulation::{Analysis, SimulationRequest, SimulationResult};
use eframe::egui::{self, Color32, Key, Pos2, Rect, Sense, Stroke, Vec2};
use std::{
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

struct Palette {
    accent: Color32,
    wire: Color32,
    gold: Color32,
    muted: Color32,
    error: Color32,
}
impl Palette {
    fn new(ctx: &egui::Context) -> Self {
        if ctx.style().visuals.dark_mode {
            Self {
                accent: Color32::from_rgb(69, 213, 176),
                wire: Color32::from_rgb(98, 187, 223),
                gold: Color32::from_rgb(255, 205, 106),
                muted: Color32::from_rgb(144, 158, 181),
                error: Color32::LIGHT_RED,
            }
        } else {
            Self {
                accent: Color32::from_rgb(8, 120, 102),
                wire: Color32::from_rgb(18, 111, 148),
                gold: Color32::from_rgb(147, 91, 9),
                muted: Color32::from_rgb(82, 101, 123),
                error: Color32::from_rgb(181, 36, 47),
            }
        }
    }
}

fn apply_theme(ctx: &egui::Context, dark: bool) {
    let mut visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.panel_fill = if dark {
        Color32::from_rgb(38, 40, 43)
    } else {
        Color32::from_rgb(239, 240, 242)
    };
    visuals.window_fill = visuals.panel_fill;
    visuals.extreme_bg_color = if dark {
        Color32::from_rgb(25, 27, 30)
    } else {
        Color32::from_rgb(255, 255, 255)
    };
    visuals.faint_bg_color = if dark {
        Color32::from_rgb(45, 47, 51)
    } else {
        Color32::from_rgb(232, 234, 237)
    };
    visuals.widgets.noninteractive.bg_fill = visuals.panel_fill;
    visuals.widgets.inactive.bg_fill = if dark {
        Color32::from_rgb(49, 52, 56)
    } else {
        Color32::from_rgb(247, 248, 249)
    };
    visuals.widgets.hovered.bg_fill = if dark {
        Color32::from_rgb(62, 67, 72)
    } else {
        Color32::from_rgb(226, 237, 237)
    };
    visuals.widgets.active.bg_fill = if dark {
        Color32::from_rgb(16, 115, 100)
    } else {
        Color32::from_rgb(196, 228, 220)
    };
    ctx.set_visuals(visuals);
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(8.0, 7.0);
    style.spacing.button_padding = Vec2::new(8.0, 4.0);
    style.visuals.selection.bg_fill = if dark {
        Color32::from_rgb(30, 97, 89)
    } else {
        Color32::from_rgb(192, 233, 221)
    };
    style.visuals.selection.stroke.color = if dark {
        Color32::WHITE
    } else {
        Color32::from_rgb(7, 82, 72)
    };
    ctx.set_style(style);
}

#[derive(Clone, Copy)]
enum Tool {
    Select,
    Place(ComponentKind),
    Wire,
    Junction,
    Label,
}
enum Pending {
    New(Project),
    Open,
    Import,
    Close,
}
#[derive(Clone, Copy, PartialEq)]
enum BottomTab {
    Results,
    Erc,
    Netlist,
    Help,
    Import,
}

#[derive(Clone, Copy, PartialEq)]
enum InspectorTab {
    Properties,
    Components,
    Nets,
}

struct ProtoCraft {
    library_mcu: bool,
    library_device: Option<String>,
    pcb: pcb_ui::PcbUi,
    engine: Engine,
    logo: Option<egui::TextureHandle>,
    path: Option<PathBuf>,
    saved_revision: u64,
    unsaved_import: bool,
    import_source: Option<PathBuf>,
    import_notes: Vec<String>,
    tool: Tool,
    placement_rotation: u8,
    selected: Option<ObjectId>,
    clipboard: clipboard::ClipboardState,
    canvas_rect: Option<Rect>,
    drag: Option<(eda_core::ComponentId, Point, Point)>,
    wire_start: Option<Point>,
    wire_vertical: bool,
    label_name: String,
    component_filter: String,
    inspector_tab: InspectorTab,
    show_library: bool,
    show_inspector: bool,
    show_analysis: bool,
    property_reference: String,
    property_value: String,
    project_name: String,
    zoom: f32,
    pan: Vec2,
    center_next: bool,
    snap: bool,
    dark: bool,
    status: String,
    status_error: bool,
    pending: Option<Pending>,
    close_allowed: bool,
    netlist: Netlist,
    diagnostics: Vec<Diagnostic>,
    derived_revision: u64,
    tab: BottomTab,
    result: Option<SimulationResult>,
    simulation: Option<mpsc::Receiver<Result<SimulationResult, String>>>,
    simulation_control: Option<eda_simulation::SimulationControl>,
    transient_step: String,
    transient_stop: String,
    selected_trace: usize,
    smoke_started: Option<Instant>,
    capture_path: Option<PathBuf>,
    capture_requested: bool,
    capture_started: Instant,
    settings: settings::Settings,
    show_settings: bool,
    property_parameters: eda_core::ModelParameters,
    live: Option<live::LiveWorker>,
}

impl ProtoCraft {
    fn new(
        engine: Engine,
        logo: Option<egui::TextureHandle>,
        path: Option<PathBuf>,
        startup_message: Option<String>,
        smoke: bool,
        capture_path: Option<PathBuf>,
    ) -> Self {
        let netlist = build_netlist(engine.project());
        let diagnostics = erc(engine.project(), &netlist);
        let project_name = engine.project().name.clone();
        let import_notes = stored_import_notes(engine.project());
        let initial_tab = if import_notes.is_empty() {
            BottomTab::Results
        } else {
            BottomTab::Import
        };
        let default_status = if engine.project().import_info.is_some() && path.is_some() {
            "Projeto importado aberto. Consulte a aba Importação."
        } else if path.is_some() {
            "Projeto aberto."
        } else {
            "Pronto. Divisor de 10 V: execute DC para obter 5 V na saída."
        };
        Self {
            library_mcu: false,
            library_device: None,
            pcb: Default::default(),
            engine,
            logo,
            path,
            saved_revision: 0,
            unsaved_import: false,
            import_source: None,
            import_notes,
            tool: Tool::Select,
            placement_rotation: 0,
            selected: None,
            clipboard: Default::default(),
            canvas_rect: None,
            drag: None,
            wire_start: None,
            wire_vertical: false,
            label_name: "OUT".into(),
            component_filter: String::new(),
            inspector_tab: InspectorTab::Properties,
            show_library: true,
            show_inspector: true,
            show_analysis: true,
            property_reference: String::new(),
            property_value: String::new(),
            project_name,
            zoom: 18.0,
            pan: Vec2::ZERO,
            center_next: true,
            snap: true,
            dark: true,
            status_error: startup_message.is_some(),
            status: startup_message.unwrap_or_else(|| default_status.into()),
            pending: None,
            close_allowed: false,
            netlist,
            diagnostics,
            derived_revision: 0,
            tab: initial_tab,
            result: None,
            simulation: None,
            simulation_control: None,
            transient_step: "10u".into(),
            transient_stop: "5m".into(),
            selected_trace: 0,
            smoke_started: if smoke { Some(Instant::now()) } else { None },
            capture_path,
            capture_requested: false,
            capture_started: Instant::now(),
            settings: settings::Settings::default(),
            show_settings: false,
            property_parameters: Default::default(),
            live: None,
        }
    }

    fn dirty(&self) -> bool {
        self.unsaved_import || self.engine.revision() != self.saved_revision
    }
    fn message(&mut self, message: impl Into<String>, error: bool) {
        self.status = message.into();
        self.status_error = error;
    }
    fn command(&mut self, command: Command) {
        match self.engine.execute(command) {
            Ok(()) => self.message("Alteração aplicada.", false),
            Err(error) => self.message(format!("Não foi possível editar: {error}"), true),
        }
    }
    fn refresh_derived(&mut self) {
        if self.derived_revision != self.engine.revision() {
            self.netlist = build_netlist(self.engine.project());
            self.diagnostics = erc(self.engine.project(), &self.netlist);
            self.derived_revision = self.engine.revision();
        }
    }
    fn choose(&mut self, object: Option<ObjectId>) {
        self.selected = object;
        if let Some(ObjectId::Component(id)) = object
            && let Some(component) = self.engine.project().components.get(&id)
        {
            self.property_reference = component.reference.clone();
            self.property_value = format!("{}", component.value);
            self.property_parameters = component.parameters.clone();
        }
    }
    fn request(&mut self, action: Pending, ctx: &egui::Context) {
        if self.dirty() {
            self.pending = Some(action);
        } else {
            self.perform(action, ctx);
        }
    }
    fn perform(&mut self, action: Pending, ctx: &egui::Context) {
        match action {
            Pending::New(project) => self.replace(project, None),
            Pending::Open => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(
                        "Projeto ProtoCraft e backups",
                        &["protocraft", "json", "bak"],
                    )
                    .pick_file()
                {
                    match eda_storage::load(&path) {
                        Ok(project) => {
                            self.replace(project, Some(path));
                            self.message("Projeto aberto.", false);
                        }
                        Err(error) => self.message(format!("Falha ao abrir: {error}"), true),
                    }
                }
            }
            Pending::Import => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Projeto Proteus experimental", &["pdsprj", "dsn"])
                    .pick_file()
                {
                    self.import_path(&path);
                }
            }
            Pending::Close => {
                self.close_allowed = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
    fn replace(&mut self, project: Project, path: Option<PathBuf>) {
        self.stop_live();
        match Engine::new(project) {
            Ok(engine) => {
                self.engine = engine;
                self.pcb.reset_for_project();
                self.clipboard.discard_cut();
                if self
                    .engine
                    .project()
                    .components
                    .values()
                    .any(|c| c.kind.requires_mixed())
                {
                    self.settings.engine = eda_simulation::SimulationEngine::Mixed;
                }
                self.path = path;
                self.saved_revision = 0;
                self.unsaved_import = false;
                self.import_source = None;
                self.import_notes = stored_import_notes(self.engine.project());
                self.tab = if self.import_notes.is_empty() {
                    BottomTab::Results
                } else {
                    BottomTab::Import
                };
                self.selected = None;
                self.wire_start = None;
                self.drag = None;
                self.tool = Tool::Select;
                self.focus_device_library();
                self.project_name = self.engine.project().name.clone();
                self.center_next = true;
                // Um novo documento reinicia a revisão; o worker anterior não pode fornecer seus resultados.
                self.result = None;
                if let Some(control) = self.simulation_control.take() {
                    control.cancel();
                }
                self.simulation = None;
                self.derived_revision = u64::MAX;
                self.refresh_derived();
                self.message(
                    "Projeto carregado. Salve uma cópia para preservar suas alterações.",
                    false,
                );
            }
            Err(error) => self.message(format!("Projeto rejeitado: {error}"), true),
        }
    }
    fn save(&mut self, save_as: bool) -> bool {
        let path = if save_as || self.path.is_none() {
            rfd::FileDialog::new()
                .add_filter("Projeto ProtoCraft", &["protocraft"])
                .set_file_name(format!("{}.protocraft", self.engine.project().name))
                .save_file()
        } else {
            self.path.clone()
        };
        let Some(mut path) = path else {
            return false;
        };
        if path.extension().is_none() {
            path.set_extension("protocraft");
        }
        if let Err(error) = ensure_export_destination(&path, self.import_source.as_deref()) {
            self.message(
                format!("Escolha um destino diferente do projeto Proteus original: {error}"),
                true,
            );
            return false;
        }
        match eda_storage::save(&path, self.engine.project()) {
            Ok(()) => {
                self.saved_revision = self.engine.revision();
                self.unsaved_import = false;
                self.message(format!("Salvo em {}", path.display()), false);
                self.path = Some(path);
                true
            }
            Err(error) => {
                self.message(format!("Falha ao salvar: {error}"), true);
                false
            }
        }
    }
    fn import_path(&mut self, path: &std::path::Path) {
        match eda_import::import_proteus(path) {
            Ok(import) => {
                // A validação precisa anteceder a substituição do documento atual.
                if let Err(error) = import.project.validate() {
                    self.message(format!("Projeto importado rejeitado: {error}"), true);
                    return;
                }
                let report = import.report;
                self.replace(import.project, None);
                self.unsaved_import = true;
                self.import_source = Some(path.to_path_buf());
                self.import_notes = import_report_notes(report);
                self.tab = BottomTab::Import;
                self.message(
                    "Proteus importado. Salve como .protocraft para preservar o resultado.",
                    false,
                );
            }
            Err(error) => self.message(format!("Falha ao importar Proteus: {error}"), true),
        }
    }
    fn confirmation(&mut self, ctx: &egui::Context) {
        if self.pending.is_none() {
            return;
        }
        let mut choice = 0;
        egui::Modal::new(egui::Id::new("save_confirmation")).show(ctx, |ui| {
            ui.heading("Salvar alterações?");
            ui.label("O projeto tem alterações que ainda não foram salvas.");
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button("Salvar e continuar").clicked() {
                    choice = 1;
                }
                if ui.button("Descartar alterações").clicked() {
                    choice = 2;
                }
                if ui.button("Cancelar").clicked() {
                    choice = 3;
                }
            });
        });
        match choice {
            1 if self.save(false) => {
                if let Some(action) = self.pending.take() {
                    self.perform(action, ctx);
                }
            }
            2 => {
                if let Some(action) = self.pending.take() {
                    self.perform(action, ctx);
                }
            }
            3 => self.pending = None,
            _ => {}
        }
    }
    fn simulation(&mut self, analysis: Analysis) {
        self.stop_live();
        if self.simulation.is_some() {
            self.message("Uma análise já está em execução.", false);
            return;
        }
        let request = SimulationRequest {
            project: self.engine.project().clone(),
            revision: self.engine.revision(),
            analysis,
        };
        let (sender, receiver) = mpsc::channel();
        let control = eda_simulation::SimulationControl::default();
        let worker_control = control.clone();
        let backend = self.settings.engine;
        match std::thread::Builder::new()
            .name("protocraft-simulation".into())
            .spawn(move || {
                let result =
                    eda_simulation::simulate_with_engine(&request, backend, &worker_control)
                        .map_err(|error| error.to_string());
                // Se a janela fechar, descartar o resultado é o comportamento esperado.
                if sender.send(result).is_err() {
                    eprintln!("ProtoCraft: janela encerrada antes do resultado da simulação.");
                }
            }) {
            Ok(_) => {
                self.simulation = Some(receiver);
                self.simulation_control = Some(control);
                self.tab = BottomTab::Results;
                self.message("Simulando em segundo plano…", false);
            }
            Err(error) => self.message(
                format!("Não foi possível iniciar a simulação: {error}"),
                true,
            ),
        }
    }
    fn poll_simulation(&mut self, ctx: &egui::Context) {
        let result = self.simulation.as_ref().map(mpsc::Receiver::try_recv);
        match result {
            Some(Ok(result)) => {
                self.simulation = None;
                self.simulation_control = None;
                match result {
                    Ok(result) => {
                        let stale = result.revision != self.engine.revision();
                        self.selected_trace = preferred_trace(&result);
                        self.result = Some(result);
                        self.message(if stale { "Análise concluída para uma revisão anterior. Execute novamente para atualizar." } else { "Análise concluída." }, false);
                    }
                    Err(error) => self.message(format!("Simulação interrompida: {error}"), true),
                }
            }
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                self.simulation = None;
                self.simulation_control = None;
                self.message("O processo de simulação terminou sem resultado.", true);
            }
            Some(Err(mpsc::TryRecvError::Empty)) => {
                ctx.request_repaint_after(Duration::from_millis(50))
            }
            None => {}
        }
    }
    fn delete(&mut self) {
        if let Some(object) = self.selected {
            self.command(Command::Delete { object });
            self.choose(None);
        }
    }
    fn rotate(&mut self) {
        if matches!(self.tool, Tool::Place(_)) {
            self.placement_rotation = (self.placement_rotation + 1) % 4;
            return;
        }
        if let Some(ObjectId::Component(id)) = self.selected {
            self.command(Command::RotateComponent { id });
        }
    }
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.pending.is_some() || ctx.wants_keyboard_input() {
            return;
        }
        if self.clipboard_shortcuts(ctx) {
            return;
        }
        if self.pcb.active {
            self.pcb_shortcuts(ctx);
            return;
        }
        let (ctrl, shift) = ctx.input(|i| (i.modifiers.command, i.modifiers.shift));
        if ctrl {
            if ctx.input(|i| i.key_pressed(Key::S)) {
                self.save(shift);
            }
            if ctx.input(|i| i.key_pressed(Key::O)) {
                self.request(Pending::Open, ctx);
            }
            if ctx.input(|i| i.key_pressed(Key::N)) {
                self.request(Pending::New(Project::empty("Novo projeto")), ctx);
            }
            if ctx.input(|i| i.key_pressed(Key::Z)) {
                if shift {
                    self.engine.redo();
                } else {
                    self.engine.undo();
                }
            }
            if ctx.input(|i| i.key_pressed(Key::Y)) {
                self.engine.redo();
            }
        } else {
            if ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.tool = Tool::Select;
                self.wire_start = None;
                self.drag = None;
            }
            if ctx.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)) {
                self.delete();
            }
            if ctx.input(|i| i.key_pressed(Key::R)) {
                self.rotate();
            }
            if ctx.input(|i| i.key_pressed(Key::W)) {
                self.tool = Tool::Wire;
                self.wire_start = None;
            }
            if ctx.input(|i| i.key_pressed(Key::V)) {
                self.tool = Tool::Select;
            }
            if ctx.input(|i| i.key_pressed(Key::J)) {
                self.tool = Tool::Junction;
            }
            if ctx.input(|i| i.key_pressed(Key::L)) {
                self.tool = Tool::Label;
            }
            if ctx.input(|i| i.key_pressed(Key::F)) {
                self.center_next = true;
            }
            if ctx.input(|i| i.key_pressed(Key::F5)) {
                self.simulation(Analysis::Dc);
            }
        }
    }

    fn top(&mut self, ctx: &egui::Context) {
        let theme = Palette::new(ctx);
        egui::TopBottomPanel::top("main_menu").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(logo) = &self.logo {
                    ui.add(egui::Image::new((logo.id(), Vec2::splat(24.0))));
                }
                ui.label(
                    egui::RichText::new("ProtoCraft")
                        .strong()
                        .color(theme.accent),
                );
                ui.separator();
                ui.menu_button("Arquivo", |ui| {
                    if ui.button("Novo projeto    Ctrl+N").clicked() {
                        self.request(Pending::New(Project::empty("Novo projeto")), ctx);
                        ui.close();
                    }
                    if ui.button("Abrir…    Ctrl+O").clicked() {
                        self.request(Pending::Open, ctx);
                        ui.close();
                    }
                    if ui.button("Importar Proteus…").clicked() {
                        self.request(Pending::Import, ctx);
                        ui.close();
                    }
                    if ui.button("Salvar    Ctrl+S").clicked() {
                        self.save(false);
                        ui.close();
                    }
                    if ui.button("Salvar como…").clicked() {
                        self.save(true);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Exportar netlist SPICE…").clicked() {
                        self.export_spice();
                        ui.close();
                    }
                    if ui.button("Sair").clicked() {
                        self.request(Pending::Close, ctx);
                        ui.close();
                    }
                });
                ui.menu_button("Editar", |ui| {
                    if ui
                        .add_enabled(
                            self.engine.can_undo(),
                            egui::Button::new("Desfazer    Ctrl+Z"),
                        )
                        .clicked()
                    {
                        self.engine.undo();
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.engine.can_redo(),
                            egui::Button::new("Refazer    Ctrl+Y"),
                        )
                        .clicked()
                    {
                        self.engine.redo();
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(
                            self.clipboard_selection().is_some(),
                            egui::Button::new("Copiar componente    Ctrl+C"),
                        )
                        .clicked()
                    {
                        self.copy_component(ctx);
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.clipboard_can_cut(),
                            egui::Button::new("Recortar componente    Ctrl+X"),
                        )
                        .clicked()
                    {
                        self.cut_component(ctx);
                        ui.close();
                    }
                    if ui.button("Colar componente    Ctrl+V").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(
                            self.selected.is_some(),
                            egui::Button::new("Girar seleção    R"),
                        )
                        .clicked()
                    {
                        self.rotate();
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.selected.is_some(),
                            egui::Button::new("Excluir seleção    Del"),
                        )
                        .clicked()
                    {
                        self.delete();
                        ui.close();
                    }
                });
                ui.menu_button("Visualizar", |ui| {
                    let mut dark = self.dark;
                    if ui.checkbox(&mut dark, "Tema escuro").changed() {
                        self.dark = dark;
                        apply_theme(ctx, dark);
                    }
                    ui.separator();
                    if ui
                        .selectable_label(!self.pcb.active, "Esquemático")
                        .clicked()
                    {
                        self.pcb.active = false;
                        ui.close();
                    }
                    if ui
                        .selectable_label(self.pcb.active, "PCB — dimensões reais")
                        .clicked()
                    {
                        self.pcb.active = true;
                        self.pcb.fit = true;
                        ui.close();
                    }
                    ui.checkbox(&mut self.show_library, "Biblioteca e ferramentas");
                    ui.checkbox(&mut self.show_inspector, "Explorador e propriedades");
                    ui.checkbox(&mut self.show_analysis, "Painel inferior");
                    ui.separator();
                    if ui.button("Enquadrar esquema    F").clicked() {
                        self.center_next = true;
                        ui.close();
                    }
                });
                ui.menu_button("Inserir", |ui| {
                    ui.menu_button("Componente", |ui| {
                        for kind in ComponentKind::all() {
                            if ui
                                .selectable_label(
                                    matches!(self.tool, Tool::Place(current) if current == *kind),
                                    kind.label(),
                                )
                                .clicked()
                            {
                                self.tool = Tool::Place(*kind);
                                ui.close();
                            }
                        }
                    });
                    ui.separator();
                    for (label, tool) in [
                        ("Selecionar / mover", Tool::Select),
                        ("Fio ortogonal", Tool::Wire),
                        ("Junção elétrica", Tool::Junction),
                        ("Nome de rede", Tool::Label),
                    ] {
                        if ui.button(label).clicked() {
                            self.tool = tool;
                            self.wire_start = None;
                            ui.close();
                        }
                    }
                });
                ui.menu_button("Projeto", |ui| {
                    if ui
                        .button(format!(
                            "Explorador de componentes ({})",
                            self.engine.project().components.len()
                        ))
                        .clicked()
                    {
                        self.inspector_tab = InspectorTab::Components;
                        self.show_inspector = true;
                        ui.close();
                    }
                    if ui
                        .button(format!("Redes ({})", self.netlist.nets.len()))
                        .clicked()
                    {
                        self.inspector_tab = InspectorTab::Nets;
                        self.show_inspector = true;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Diagnóstico ERC").clicked() {
                        self.tab = BottomTab::Erc;
                        self.show_analysis = true;
                        ui.close();
                    }
                    if ui.button("Relatório de importação").clicked() {
                        self.tab = BottomTab::Import;
                        self.show_analysis = true;
                        ui.close();
                    }
                });
                ui.menu_button("Configurações", |ui| {
                    if ui.button("Motor e visualização…").clicked() {
                        self.show_settings = true;
                        ui.close();
                    }
                });
                ui.menu_button("Simulação", |ui| {
                    if ui.button("Iniciar execução contínua").clicked() {
                        self.start_live();
                        ui.close();
                    }
                    if ui.button("Parar execução contínua").clicked() {
                        self.stop_live();
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(
                            self.simulation.is_none()
                                && !self
                                    .engine
                                    .project()
                                    .components
                                    .values()
                                    .any(|c| c.device.is_some()),
                            egui::Button::new("Executar análise DC    F5"),
                        )
                        .clicked()
                    {
                        self.simulation(Analysis::Dc);
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.simulation.is_none()
                                && !self
                                    .engine
                                    .project()
                                    .components
                                    .values()
                                    .any(|c| c.device.is_some()),
                            egui::Button::new("Executar transiente"),
                        )
                        .clicked()
                    {
                        self.run_transient();
                        ui.close();
                    }
                    if let Some(control) = &self.simulation_control
                        && ui.button("Cancelar análise").clicked()
                    {
                        control.cancel();
                        self.message("Cancelamento solicitado…", false);
                        ui.close();
                    }
                });
                ui.menu_button("Exemplos", |ui| {
                    if ui.button("STM32 + ESP32-S3 · pinagem e PCB").clicked() {
                        match eda_library::physical_example() {
                            Ok(p) => self.request(Pending::New(p), ctx),
                            Err(e) => self.message(e, true),
                        }
                        ui.close();
                    }
                    ui.separator();
                    for (_, name, project) in examples::CATALOG {
                        if ui.button(*name).clicked() {
                            self.request(Pending::New(project()), ctx);
                            ui.close();
                        }
                    }
                });
                ui.menu_button("Ajuda", |ui| {
                    if ui.button("Atalhos e limites").clicked() {
                        self.tab = BottomTab::Help;
                        self.show_analysis = true;
                        ui.close();
                    }
                    ui.label("ProtoCraft · editor esquemático");
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let icon = if self.dark {
                        "Tema: Escuro"
                    } else {
                        "Tema: Claro"
                    };
                    if ui
                        .button(icon)
                        .on_hover_text(if self.dark {
                            "Tema claro"
                        } else {
                            "Tema escuro"
                        })
                        .clicked()
                    {
                        self.dark = !self.dark;
                        apply_theme(ctx, self.dark);
                    }
                });
            });
        });

        egui::TopBottomPanel::top("simulation_controls").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| self.live_controls(ui));
        });
        egui::TopBottomPanel::top("command_toolbar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Novo").clicked() {
                    self.request(Pending::New(Project::empty("Novo projeto")), ctx);
                }
                if ui.button("Abrir").clicked() {
                    self.request(Pending::Open, ctx);
                }
                if ui.button("Salvar").clicked() {
                    self.save(false);
                }
                ui.separator();
                if ui
                    .add_enabled(self.engine.can_undo(), egui::Button::new("Desfazer"))
                    .on_hover_text("Desfazer · Ctrl+Z")
                    .clicked()
                {
                    self.engine.undo();
                }
                if ui
                    .add_enabled(self.engine.can_redo(), egui::Button::new("Refazer"))
                    .on_hover_text("Refazer · Ctrl+Y")
                    .clicked()
                {
                    self.engine.redo();
                }
                ui.separator();
                self.tool_button(
                    ui,
                    "SEL",
                    "Selecionar / mover",
                    matches!(self.tool, Tool::Select),
                    Tool::Select,
                );
                self.tool_button(
                    ui,
                    "CMP",
                    "Selecionar componente",
                    matches!(self.tool, Tool::Place(_)),
                    Tool::Place(ComponentKind::Resistor),
                );
                self.tool_button(
                    ui,
                    "FIO",
                    "Fio ortogonal",
                    matches!(self.tool, Tool::Wire),
                    Tool::Wire,
                );
                self.tool_button(
                    ui,
                    "JUN",
                    "Junção elétrica",
                    matches!(self.tool, Tool::Junction),
                    Tool::Junction,
                );
                self.tool_button(
                    ui,
                    "NET",
                    "Nome de rede",
                    matches!(self.tool, Tool::Label),
                    Tool::Label,
                );
                ui.separator();
                if ui
                    .add_enabled(
                        self.simulation.is_none()
                            && !self
                                .engine
                                .project()
                                .components
                                .values()
                                .any(|c| c.device.is_some()),
                        egui::Button::new(egui::RichText::new("▶ DC").color(Color32::WHITE))
                            .fill(Color32::from_rgb(28, 116, 95)),
                    )
                    .on_hover_text("Executar análise DC · F5")
                    .clicked()
                {
                    self.simulation(Analysis::Dc);
                }
                ui.label("Δt");
                ui.add(egui::TextEdit::singleline(&mut self.transient_step).desired_width(52.0));
                ui.label("Fim");
                ui.add(egui::TextEdit::singleline(&mut self.transient_stop).desired_width(52.0));
                if ui
                    .add_enabled(
                        self.simulation.is_none()
                            && !self
                                .engine
                                .project()
                                .components
                                .values()
                                .any(|c| c.device.is_some()),
                        egui::Button::new("▶ Transiente"),
                    )
                    .clicked()
                {
                    self.run_transient();
                }
                if self.simulation.is_some() {
                    ui.spinner();
                }
            });
        });

        egui::TopBottomPanel::top("document_tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Esquemático 1");
                ui.separator();
                ui.label(
                    egui::RichText::new(format!(
                        "{}{}",
                        self.engine.project().name,
                        if self.dirty() { "  ●" } else { "" }
                    ))
                    .strong(),
                );
                if self.engine.project().import_info.is_some() {
                    ui.label(
                        egui::RichText::new("IMPORTADO · REVISÃO NECESSÁRIA")
                            .small()
                            .color(theme.gold),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} componentes · {} redes",
                            self.engine.project().components.len(),
                            self.netlist.nets.len()
                        ))
                        .small()
                        .color(theme.muted),
                    );
                });
            });
        });
    }

    fn tool_button(
        &mut self,
        ui: &mut egui::Ui,
        icon: &str,
        tooltip: &str,
        active: bool,
        tool: Tool,
    ) {
        if ui
            .add_sized([34.0, 30.0], egui::Button::new(icon).selected(active))
            .on_hover_text(tooltip)
            .clicked()
        {
            self.tool = tool;
            self.wire_start = None;
        }
    }

    fn run_transient(&mut self) {
        match (
            geometry::parse_value(&self.transient_step),
            geometry::parse_value(&self.transient_stop),
        ) {
            (Ok(step), Ok(stop)) => self.simulation(Analysis::Transient { step, stop }),
            _ => self.message("Tempo inválido. Use segundos ou sufixos SI: 10u, 5m.", true),
        }
    }

    fn export_spice(&mut self) {
        match eda_simulation::export_spice(self.engine.project(), Analysis::Dc) {
            Ok(text) => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Netlist SPICE", &["cir"])
                    .set_file_name("circuito.cir")
                    .save_file()
                {
                    let exported = ensure_export_destination(&path, self.path.as_deref())
                        .and_then(|()| {
                            ensure_export_destination(&path, self.import_source.as_deref())
                        })
                        .and_then(|()| {
                            std::fs::write(&path, text).map_err(|error| error.to_string())
                        });
                    match exported {
                        Ok(()) => {
                            self.message(format!("Netlist exportada: {}", path.display()), false)
                        }
                        Err(error) => self.message(format!("Falha ao exportar: {error}"), true),
                    }
                }
            }
            Err(error) => self.message(format!("Não foi possível exportar: {error}"), true),
        }
    }

    fn library(&mut self, ctx: &egui::Context) {
        let theme = Palette::new(ctx);
        egui::SidePanel::left("library").default_width(230.0).width_range(190.0..=320.0).show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.heading("Dispositivos");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label(egui::RichText::new("BIBLIOTECA").small().color(theme.muted)); });
            });
            ui.add(egui::TextEdit::singleline(&mut self.component_filter).hint_text("Pesquisar componente…").desired_width(f32::INFINITY));
            ui.horizontal(|ui| {ui.selectable_value(&mut self.library_mcu,false,"Básicos");ui.selectable_value(&mut self.library_mcu,true,"STM32 / ESP32");});
            ui.add_space(5.0);
            if self.library_mcu {egui::ScrollArea::vertical().show(ui,|ui|self.device_library(ui));return;}
            egui::ScrollArea::vertical().show(ui, |ui| {
                let query = self.component_filter.to_lowercase();
                for kind in ComponentKind::all() {
                    let kind = *kind;
                    if !query.is_empty() && !kind.label().to_lowercase().contains(&query) { continue; }
                    let active = matches!(self.tool, Tool::Place(current) if current == kind);
                    if ui.add_sized([ui.available_width(), 27.0], egui::Button::new(format!("{}    {}", kind.prefix(), kind.label())).selected(active)).clicked() { self.tool = Tool::Place(kind); self.wire_start = None; }
                }
                if !query.is_empty() && !ComponentKind::all().iter().any(|kind| kind.label().to_lowercase().contains(&query)) { ui.label("Nenhum componente encontrado."); }
                ui.add_space(8.0);
                if matches!(self.tool, Tool::Label) { ui.separator(); ui.label("Nome da rede"); ui.text_edit_singleline(&mut self.label_name); }
                if matches!(self.tool, Tool::Wire) { ui.separator(); ui.checkbox(&mut self.wire_vertical, "Primeiro trecho vertical"); }
                ui.separator();
                ui.checkbox(&mut self.snap, "Atração à grade");
                ui.label(egui::RichText::new("Conexões sempre usam coordenadas inteiras. Cruzamento de fios exige junção.").small().color(theme.muted));
            });
        });
    }

    fn mode_strip(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("mode_strip")
            .exact_width(46.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                for (icon, hint, tool, active) in [
                    (
                        "S",
                        "Selecionar · V",
                        Tool::Select,
                        matches!(self.tool, Tool::Select),
                    ),
                    (
                        "C",
                        "Componente",
                        Tool::Place(ComponentKind::Resistor),
                        matches!(self.tool, Tool::Place(_)),
                    ),
                    ("F", "Fio · W", Tool::Wire, matches!(self.tool, Tool::Wire)),
                    (
                        "J",
                        "Junção · J",
                        Tool::Junction,
                        matches!(self.tool, Tool::Junction),
                    ),
                    (
                        "N",
                        "Nome de rede · L",
                        Tool::Label,
                        matches!(self.tool, Tool::Label),
                    ),
                ] {
                    if ui
                        .add_sized([32.0, 34.0], egui::Button::new(icon).selected(active))
                        .on_hover_text(hint)
                        .clicked()
                    {
                        self.tool = tool;
                        self.wire_start = None;
                    }
                }
            });
    }

    fn properties(&mut self, ctx: &egui::Context) {
        if !self.show_inspector {
            return;
        }
        let theme = Palette::new(ctx);
        egui::SidePanel::right("inspector")
            .default_width(270.0)
            .width_range(230.0..=420.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.heading("Explorador");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.small(format!(
                            "ERC {}",
                            self.diagnostics
                                .iter()
                                .filter(|d| d.severity == Severity::Error)
                                .count()
                        ));
                    });
                });
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.inspector_tab,
                        InspectorTab::Properties,
                        "Propriedades",
                    );
                    ui.selectable_value(
                        &mut self.inspector_tab,
                        InspectorTab::Components,
                        "Componentes",
                    );
                    ui.selectable_value(&mut self.inspector_tab, InspectorTab::Nets, "Redes");
                });
                ui.separator();
                match self.inspector_tab {
                    InspectorTab::Properties => self.property_contents(ui, theme),
                    InspectorTab::Components => self.component_explorer(ui),
                    InspectorTab::Nets => self.net_explorer(ui),
                }
            });
    }

    fn property_contents(&mut self, ui: &mut egui::Ui, theme: Palette) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            if let Some(object) = self.selected {
                match object {
                    ObjectId::Component(id) => {
                        if let Some(component) = self.engine.project().components.get(&id).cloned() {
                            ui.label(egui::RichText::new(component.kind.label()).size(18.0).strong().color(theme.accent));
                            ui.label(format!("Posição: {}, {} · {}°", component.position.x, component.position.y, u16::from(component.rotation) * 90));
                            ui.add_space(12.0);
                            ui.label("Referência");
                            ui.text_edit_singleline(&mut self.property_reference);
                            if !component.kind.requires_mixed() && component.device.is_none() {
                            ui.label(if component.kind.is_waveform() {"Amplitude de pico (V)".to_owned()} else {match component.kind {
                                ComponentKind::Diode|ComponentKind::Led=>"Corrente de saturação Is (A)".to_owned(),
                                ComponentKind::Zener=>"Tensão de ruptura (V)".to_owned(),
                                ComponentKind::Npn|ComponentKind::Pnp=>"Ganho de corrente β".to_owned(),
                                ComponentKind::Nmos|ComponentKind::Pmos=>"Parâmetro de condução k (A/V²)".to_owned(),
                                kind if kind.is_gate() || kind==ComponentKind::LogicInput=>"Nível alto (V)".to_owned(),
                                _=>format!("Valor ({})",component.kind.unit()),
                            }});
                            ui.text_edit_singleline(&mut self.property_value);
                            ui.label(egui::RichText::new("SI: 1k = 1000 · 1u = 0,000001").small().color(theme.muted));
                            }
                            self.model_properties(ui,&component);
                            self.physical_properties(ui,&component);
                            if matches!(component.kind,ComponentKind::Switch|ComponentKind::LogicInput) && ui.button("Alternar agora").clicked() {self.toggle_interactive(id);}
                            if ui.button("Aplicar propriedades").clicked() {
                                match geometry::parse_value(&self.property_value) {
                                    Ok(value) => self.command(Command::Batch(vec![Command::SetComponent { id, reference: self.property_reference.trim().to_owned(), value },Command::SetModel {id,parameters:self.property_parameters.clone()}])),
                                    Err(error) => self.message(error, true),
                                }
                            }
                            if ui.button("Girar 90°    R").on_hover_text("Os fios ficam nas posições originais; confira as conexões após girar.").clicked() { self.rotate(); }
                            ui.separator();
                            ui.strong("Pinos elétricos");
                            for pin in component.pins() { ui.label(format!("{} · {} → ({}, {})", component.pin_number(pin.reference.pin), component.pin_name(pin.reference.pin), pin.position.x, pin.position.y)); }
                            if let Some(imported) = &component.imported {
                                ui.separator(); ui.strong("Dados originais Proteus");
                                ui.label(format!("Dispositivo: {}", imported.device));
                                ui.label(format!("Valor original: {}", imported.value_text));
                                ui.label(if component.kind == ComponentKind::Imported { "Modelo elétrico não suportado" } else { "Modelo genérico reconhecido; confira os parâmetros" });
                                for pin in &imported.pins { ui.label(format!("Pino {}: {}", pin.number, pin.name)); }
                                ui.collapsing("Propriedades preservadas", |ui| { for (key, value) in &imported.properties { ui.label(format!("{key}: {value}")); } });
                            }
                        }
                    }
                    ObjectId::Wire(id) => if let Some(wire) = self.engine.project().wires.get(&id) { ui.heading("Fio"); ui.label(format!("({}, {}) para ({}, {})", wire.start.x, wire.start.y, wire.end.x, wire.end.y)); },
                    ObjectId::Junction(_) => { ui.heading("Junção"); ui.label("Junção elétrica explícita"); }
                    ObjectId::Label(id) => if let Some(label) = self.engine.project().labels.get(&id) { ui.heading("Nome de rede"); ui.strong(&label.name); ui.label("Mesmo nome conecta redes nesta folha."); },
                }
                ui.add_space(12.0);
                if ui.button(egui::RichText::new("Excluir seleção    Del").color(theme.error)).clicked() { self.delete(); }
            } else {
                ui.label(egui::RichText::new("Selecione um item no esquema ou no Explorador de componentes.").color(theme.muted));
            }
            ui.add_space(20.0); ui.separator(); ui.heading("Projeto");
            ui.text_edit_singleline(&mut self.project_name);
            if ui.button("Renomear projeto").clicked() { self.command(Command::RenameProject { name: self.project_name.clone() }); }
            ui.label(format!("{} componentes · {} fios", self.engine.project().components.len(), self.engine.project().wires.len()));
            ui.label(format!("{} redes · revisão {}", self.netlist.nets.len(), self.engine.revision()));
            let errors = self.diagnostics.iter().filter(|d| d.severity == Severity::Error).count();
            ui.label(egui::RichText::new(format!("ERC: {errors} erros / {} avisos", self.diagnostics.len() - errors)).color(if errors > 0 { theme.error } else { theme.accent }));
            if let Some(path) = &self.path { ui.label(egui::RichText::new(path.display().to_string()).small().color(theme.muted)); }
        });
    }

    fn component_explorer(&mut self, ui: &mut egui::Ui) {
        ui.label("Design Explorer · componentes");
        ui.add(
            egui::TextEdit::singleline(&mut self.component_filter)
                .hint_text("Filtrar por referência…"),
        );
        let query = self.component_filter.to_lowercase();
        let rows: Vec<_> = self
            .engine
            .project()
            .components
            .values()
            .map(|component| {
                (
                    component.id,
                    component.reference.clone(),
                    component.kind.label().to_owned(),
                    component.value,
                )
            })
            .filter(|(_, reference, kind, _)| {
                query.is_empty()
                    || reference.to_lowercase().contains(&query)
                    || kind.to_lowercase().contains(&query)
            })
            .collect();
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("component_explorer_grid")
                .striped(true)
                .show(ui, |ui| {
                    ui.strong("Ref.");
                    ui.strong("Dispositivo");
                    ui.end_row();
                    for (id, reference, kind, value) in rows {
                        let label = format!("{reference}   {kind}   {}", engineering(value));
                        if ui
                            .selectable_label(self.selected == Some(ObjectId::Component(id)), label)
                            .clicked()
                        {
                            self.choose(Some(ObjectId::Component(id)));
                            self.inspector_tab = InspectorTab::Properties;
                        }
                        ui.end_row();
                    }
                });
        });
    }

    fn net_explorer(&mut self, ui: &mut egui::Ui) {
        ui.label("Design Explorer · conectividade");
        let rows: Vec<_> = self
            .netlist
            .nets
            .iter()
            .map(|net| {
                (
                    net.name.clone(),
                    net.pins.len(),
                    net.points.len(),
                    net.pins.first().map(|pin| pin.component),
                )
            })
            .collect();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (name, pin_count, point_count, first_component) in rows {
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(false, format!("{name}   ·   {pin_count} pinos"))
                        .clicked()
                        && let Some(id) = first_component
                    {
                        self.choose(Some(ObjectId::Component(id)));
                        self.inspector_tab = InspectorTab::Properties;
                    }
                    ui.small(format!("{point_count} nós"));
                });
            }
        });
    }

    fn bottom(&mut self, ctx: &egui::Context) {
        let theme = Palette::new(ctx);
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new(&self.status).color(if self.status_error {
                        theme.error
                    } else {
                        theme.muted
                    }),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!(
                        "{} · {:.0}%",
                        if self.dirty() { "Não salvo" } else { "Salvo" },
                        self.zoom / 18.0 * 100.0
                    ));
                });
            });
        });
        if self.show_analysis {
            egui::TopBottomPanel::bottom("analysis").resizable(true).default_height(205.0).min_height(100.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, BottomTab::Results, "Resultados");
                ui.selectable_value(&mut self.tab, BottomTab::Erc, format!("ERC ({})", self.diagnostics.len()));
                ui.selectable_value(&mut self.tab, BottomTab::Netlist, "Netlist");
                ui.selectable_value(&mut self.tab, BottomTab::Help, "Atalhos e limites");
                ui.selectable_value(&mut self.tab, BottomTab::Import, "Importação");
            }); ui.separator();
            match self.tab {
                BottomTab::Import => { egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.strong("Importação Proteus experimental");
                    if self.import_notes.is_empty() { ui.label("Use Arquivo → Importar Proteus. A conversão preserva os dados reconhecidos e informa limitações por projeto."); }
                    for note in &self.import_notes { ui.label(note); }
                    ui.label("Símbolos importados são representações originais do ProtoCraft, com pinos nas coordenadas convertidas do projeto.");
                }); }
                BottomTab::Results => self.results(ui),
                BottomTab::Erc => {
                    let mut selected = None;
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        if self.diagnostics.is_empty() { ui.colored_label(theme.accent, "✓ Nenhum diagnóstico ERC básico. Isso não substitui revisão elétrica."); }
                        for diagnostic in &self.diagnostics {
                            let color = if matches!(diagnostic.severity, Severity::Error) { theme.error } else { theme.gold };
                            if ui.button(egui::RichText::new(format!("{} [{}] {}", if matches!(diagnostic.severity, Severity::Error) { "ERRO" } else { "AVISO" }, diagnostic.code, diagnostic.message)).color(color)).clicked() { selected = diagnostic.objects.first().copied(); }
                        }
                    });
                    if selected.is_some() { self.choose(selected); }
                }
                BottomTab::Netlist => { egui::ScrollArea::vertical().show(ui, |ui| {
                    for net in &self.netlist.nets { ui.monospace(format!("{}   {} pinos   {} pontos", net.name, net.pins.len(), net.points.len())); }
                }); }
                BottomTab::Help => { egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.label("V selecionar · W fio · J junção · L nome de rede · R girar seleção · Del excluir · Esc cancelar ferramenta · F enquadrar");
                    ui.label("Roda: zoom no cursor · botão central/direito: deslocar · arrastar componente: mover · clique: posicionar / iniciar ou terminar fio");
                    ui.label("Ctrl+N novo · Ctrl+O abrir · Ctrl+S salvar · Ctrl+Shift+S salvar como · Ctrl+Z desfazer · Ctrl+Y refazer · F5 DC");
                    ui.label("Ctrl+C copia · Ctrl+X recorta · Ctrl+V cola no cursor. Componentes preservam valor, rotação, pinagem e footprint. Em campos de texto os atalhos editam o texto.");
                    ui.label("Valores SI: 1k, 2.2M, 1m, 1u, 1n, 1p ou notação científica. M = mega; m = mili.");
                    ui.label("Motores Estável e Dinâmico em Configurações. Modelos genéricos: RLC, fontes de ondas, chave, potenciômetro, LED/Zener, BJT, operacional e lógica combinacional. O motor Misto acrescenta eventos, atrasos, comparadores, 555 e lógica sequencial. Sem AC de pequenos sinais, parasitas, MCU ou PCB.");
                    ui.label("Contínua mantém o estado elétrico entre avanços. Dois cliques alternam chave/entrada lógica. Mudança de topologia reinicia o tempo; mudanças de parâmetros preservam o estado.");
                    ui.label("Mover/rotacionar componente não acompanha fios: confira os pinos e ERC após cada edição. Importação Proteus experimental em Arquivo; salve o resultado em .protocraft.");
                }); }
            }
        });
        }
    }

    fn results(&mut self, ui: &mut egui::Ui) {
        let theme = Palette::new(ui.ctx());
        let Some(result) = self.result.as_ref() else {
            if self
                .engine
                .project()
                .components
                .values()
                .any(|c| c.device.is_some())
            {
                ui.heading("Projeto físico: confira a pinagem e a PCB.");
                ui.label("Esses microcontroladores não executam firmware nem simulam seus periféricos nesta versão.");
                return;
            }
            ui.add_space(16.0);
            ui.label(
                egui::RichText::new("O circuito está pronto para explorar.")
                    .size(18.0)
                    .color(theme.accent),
            );
            ui.label("Execute DC para tensões e correntes; use o exemplo RC para observar a carga do capacitor no transiente.");
            return;
        };
        let mut export = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("{} · revisão {}", result.backend, result.revision));
            if result.revision != self.engine.revision() {
                ui.colored_label(theme.gold, "⚠ Resultado desatualizado");
            }
            if ui.button("Exportar CSV…").clicked() {
                export = true;
            }
            if matches!(result.analysis, Analysis::Transient { .. }) {
                egui::ComboBox::from_id_salt("trace")
                    .selected_text(
                        result
                            .traces
                            .get(self.selected_trace)
                            .map(|trace| trace.name.as_str())
                            .unwrap_or("Sem sinais"),
                    )
                    .show_ui(ui, |ui| {
                        for (index, trace) in result.traces.iter().enumerate() {
                            ui.selectable_value(
                                &mut self.selected_trace,
                                index,
                                format!("{} [{}]", trace.name, trace.unit),
                            );
                        }
                    });
            }
        });
        if !result.warnings.is_empty() {
            ui.collapsing("Modelo e método", |ui| {
                for warning in &result.warnings {
                    ui.add(egui::Label::new(egui::RichText::new(warning).color(theme.gold)).wrap());
                }
            });
        }
        match result.analysis {
            Analysis::Dc => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::Grid::new("dc_results")
                        .num_columns(3)
                        .striped(true)
                        .min_col_width(130.0)
                        .show(ui, |ui| {
                            ui.strong("Sinal");
                            ui.strong("Valor");
                            ui.strong("Unidade");
                            ui.end_row();
                            for trace in &result.traces {
                                ui.label(&trace.name);
                                if let Some(sample) = trace.samples.last() {
                                    ui.monospace(format!("{:.7}", sample[1]));
                                } else {
                                    ui.label("Sem amostras");
                                }
                                ui.label(&trace.unit);
                                ui.end_row();
                            }
                        });
                });
            }
            Analysis::Transient { .. } => {
                if let Some(trace) = result.traces.get(self.selected_trace) {
                    draw_plot(ui, trace);
                }
            }
        }
        if export
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("Resultados CSV", &["csv"])
                .set_file_name("resultado.csv")
                .save_file()
        {
            let exported = ensure_export_destination(&path, self.path.as_deref())
                .and_then(|()| ensure_export_destination(&path, self.import_source.as_deref()))
                .and_then(|()| {
                    eda_storage::export_csv(&path, result).map_err(|error| error.to_string())
                });
            match exported {
                Ok(()) => self.message(format!("Resultados exportados: {}", path.display()), false),
                Err(error) => self.message(format!("Falha ao exportar CSV: {error}"), true),
            }
        }
    }

    fn world(&self, pos: Pos2, rect: Rect) -> [f32; 2] {
        let delta = pos - rect.center() - self.pan;
        [delta.x / self.zoom, delta.y / self.zoom]
    }
    fn screen(&self, point: Point, rect: Rect) -> Pos2 {
        rect.center() + self.pan + Vec2::new(point.x as f32, point.y as f32) * self.zoom
    }
    fn point(&self, pos: Pos2, rect: Rect) -> Point {
        let world = self.world(pos, rect);
        Point {
            x: world[0].round().clamp(-100_000.0, 100_000.0) as i32,
            y: world[1].round().clamp(-100_000.0, 100_000.0) as i32,
        }
    }
    fn hit(&self, pos: Pos2, rect: Rect) -> Option<ObjectId> {
        let p = self.world(pos, rect);
        let tolerance = (8.0 / self.zoom).max(0.45);
        for component in self.engine.project().components.values().rev() {
            if component.imported.is_some() || component.device.is_some() {
                if component_bounds(component)
                    .expand(tolerance)
                    .contains(Pos2::new(p[0], p[1]))
                {
                    return Some(ObjectId::Component(component.id));
                }
                continue;
            }
            let dx = p[0] - component.position.x as f32;
            let dy = p[1] - component.position.y as f32;
            let (major, minor) = if component.kind.pin_count() > 3 {
                if component_bounds(component)
                    .expand(tolerance)
                    .contains(Pos2::new(p[0], p[1]))
                {
                    return Some(ObjectId::Component(component.id));
                }
                continue;
            } else if component.kind.pin_count() > 2 {
                (3.7, 3.7)
            } else if component.rotation.is_multiple_of(2) {
                (dx, dy)
            } else {
                (dy, dx)
            };
            if major.abs() < 3.0 + tolerance && minor.abs() < 1.25 + tolerance {
                return Some(ObjectId::Component(component.id));
            }
        }
        for label in self.engine.project().labels.values().rev() {
            let label_pos = self.screen(label.position, rect);
            if Rect::from_min_size(
                label_pos - Vec2::new(3.0, 22.0),
                Vec2::new((label.name.len() as f32 * 8.0).max(16.0), 28.0),
            )
            .contains(pos)
            {
                return Some(ObjectId::Label(label.id));
            }
        }
        for junction in self.engine.project().junctions.values().rev() {
            if ((p[0] - junction.position.x as f32).powi(2)
                + (p[1] - junction.position.y as f32).powi(2))
            .sqrt()
                < tolerance
            {
                return Some(ObjectId::Junction(junction.id));
            }
        }
        self.engine
            .project()
            .wires
            .values()
            .rev()
            .find(|wire| geometry::distance_to_segment(p, wire.start, wire.end) < tolerance)
            .map(|wire| ObjectId::Wire(wire.id))
    }

    fn fit(&mut self, rect: Rect) {
        let mut points: Vec<Point> = self
            .engine
            .project()
            .components
            .values()
            .flat_map(|component| {
                component
                    .pins()
                    .into_iter()
                    .map(|pin| pin.position)
                    .chain(std::iter::once(component.position))
            })
            .collect();
        points.extend(
            self.engine
                .project()
                .wires
                .values()
                .flat_map(|wire| [wire.start, wire.end]),
        );
        points.extend(
            self.engine
                .project()
                .labels
                .values()
                .map(|label| label.position),
        );
        if points.is_empty() {
            self.zoom = 18.0;
            self.pan = Vec2::ZERO;
            return;
        }
        let min_x = points.iter().map(|p| p.x).min().unwrap_or(0) as f32;
        let max_x = points.iter().map(|p| p.x).max().unwrap_or(0) as f32;
        let min_y = points.iter().map(|p| p.y).min().unwrap_or(0) as f32;
        let max_y = points.iter().map(|p| p.y).max().unwrap_or(0) as f32;
        self.zoom = ((rect.width() - 120.0) / (max_x - min_x + 8.0))
            .min((rect.height() - 100.0) / (max_y - min_y + 8.0))
            .clamp(0.25, 25.0);
        self.pan = -Vec2::new((min_x + max_x) / 2.0, (min_y + max_y) / 2.0) * self.zoom;
    }

    fn canvas(&mut self, ctx: &egui::Context) {
        let theme = Palette::new(ctx);
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let (response, painter) =
                    ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
                let rect = response.rect;
                self.canvas_rect = Some(rect);
                if self.center_next {
                    self.fit(rect);
                    self.center_next = false;
                }
                let bg = if self.dark {
                    Color32::from_rgb(27, 29, 32)
                } else {
                    Color32::from_rgb(250, 250, 247)
                };
                painter.rect_filled(rect, 0.0, bg);
                if self.pending.is_none() {
                    if response.hovered() {
                        let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
                        if scroll != 0.0
                            && let Some(cursor) = response.hover_pos()
                        {
                            let before = cursor - rect.center() - self.pan;
                            let old_zoom = self.zoom;
                            self.zoom = (self.zoom * (scroll * 0.002).exp()).clamp(0.25, 64.0);
                            self.pan += before * (1.0 - self.zoom / old_zoom);
                        }
                    }
                    if response.dragged_by(egui::PointerButton::Middle)
                        || response.dragged_by(egui::PointerButton::Secondary)
                    {
                        self.pan += ctx.input(|i| i.pointer.delta());
                    }
                    if response.clicked_by(egui::PointerButton::Primary)
                        && let Some(pos) = response.interact_pointer_pos()
                    {
                        let point = self.placement_point(pos, rect);
                        match self.tool {
                            Tool::Select => {
                                let hit = self.hit(pos, rect);
                                self.choose(hit);
                            }
                            Tool::Place(kind) => {
                                let id = eda_core::ComponentId(self.engine.project().next_id);
                                let initial = if kind == ComponentKind::Device {
                                    self.library_component(point).and_then(|c| {
                                        Some(Command::PlaceDevice {
                                            device: c.device?,
                                            footprint: c.footprint?,
                                            position: point,
                                        })
                                    })
                                } else {
                                    Some(Command::PlaceComponent {
                                        kind,
                                        position: point,
                                    })
                                };
                                let Some(initial) = initial else {
                                    self.message("Selecione um dispositivo do catálogo.", true);
                                    return;
                                };
                                let mut commands = vec![initial];
                                for _ in 0..self.placement_rotation {
                                    commands.push(Command::RotateComponent { id });
                                }
                                self.command(Command::Batch(commands));
                            }
                            Tool::Junction => {
                                self.command(Command::AddJunction { position: point })
                            }
                            Tool::Label => self.command(Command::AddLabel {
                                position: point,
                                name: self.label_name.trim().to_owned(),
                            }),
                            Tool::Wire => {
                                if let Some(start) = self.wire_start {
                                    if start != point {
                                        let corner = geometry::orthogonal_corner(
                                            start,
                                            point,
                                            self.wire_vertical,
                                        );
                                        let commands: Vec<_> = [(start, corner), (corner, point)]
                                            .into_iter()
                                            .filter(|(a, b)| a != b)
                                            .map(|(start, end)| Command::AddWire { start, end })
                                            .collect();
                                        self.command(Command::Batch(commands));
                                        self.wire_start = Some(point);
                                    }
                                } else {
                                    self.wire_start = Some(point);
                                }
                            }
                        }
                    }
                    if response.drag_started_by(egui::PointerButton::Primary)
                        && matches!(self.tool, Tool::Select)
                        && let Some(pos) = ctx.input(|i| i.pointer.press_origin())
                        && let Some(ObjectId::Component(id)) = self.hit(pos, rect)
                        && let Some(component) = self.engine.project().components.get(&id)
                    {
                        self.drag = Some((id, component.position, self.point(pos, rect)));
                        self.choose(Some(ObjectId::Component(id)));
                    }
                    if response.drag_stopped_by(egui::PointerButton::Primary)
                        && let Some((id, initial, origin)) = self.drag.take()
                        && let Some(pos) = response.interact_pointer_pos()
                    {
                        let end = self.point(pos, rect);
                        let position = Point {
                            x: initial.x + end.x - origin.x,
                            y: initial.y + end.y - origin.y,
                        };
                        if position != initial {
                            self.command(Command::MoveComponent { id, position });
                        }
                    }
                }
                if response.double_clicked()
                    && matches!(self.tool, Tool::Select)
                    && let Some(pos) = response.interact_pointer_pos()
                    && let Some(ObjectId::Component(id)) = self.hit(pos, rect)
                {
                    self.toggle_interactive(id);
                }
                self.draw_grid(&painter, rect);
                let solved = self
                    .result
                    .as_ref()
                    .filter(|r| r.revision == self.engine.revision());
                let mut wire_colors = std::collections::BTreeMap::new();
                if self.settings.voltage_colors
                    && let Some(result) = solved
                {
                    for net in &self.netlist.nets {
                        if let Some(sample) = result
                            .traces
                            .iter()
                            .find(|t| t.name == format!("V({})", net.name))
                            .and_then(|t| t.samples.last())
                        {
                            let color = simulation_ui::voltage_color(
                                sample[1],
                                self.settings.voltage_scale,
                            );
                            for point in &net.points {
                                wire_colors.insert(*point, color);
                            }
                        }
                    }
                }
                for wire in self.engine.project().wires.values() {
                    let selected = self.selected == Some(ObjectId::Wire(wire.id));
                    let wire_color = wire_colors.get(&wire.start).copied().unwrap_or(theme.wire);
                    painter.line_segment(
                        [self.screen(wire.start, rect), self.screen(wire.end, rect)],
                        Stroke::new(
                            if selected { 3.5_f32 } else { 2.0_f32 },
                            if selected { theme.gold } else { wire_color },
                        ),
                    );
                    for point in [wire.start, wire.end] {
                        painter.circle_filled(
                            self.screen(point, rect),
                            2.3,
                            if selected { theme.gold } else { wire_color },
                        );
                    }
                }
                for component in self.engine.project().components.values() {
                    let mut preview = component.clone();
                    if let Some((id, initial, origin)) = self.drag
                        && id == component.id
                        && let Some(pos) = response.interact_pointer_pos()
                    {
                        let end = self.point(pos, rect);
                        preview.position = Point {
                            x: initial.x + end.x - origin.x,
                            y: initial.y + end.y - origin.y,
                        };
                    }
                    let operating = solved.and_then(|r| {
                        r.operating_points
                            .iter()
                            .find(|p| p.component == component.id)
                    });
                    if component.kind == ComponentKind::Led
                        && let Some(point) = operating
                        && point.current > 0.0
                    {
                        let alpha = (point.current / 0.02).clamp(0.0, 1.0) as f32;
                        painter.circle_filled(
                            self.screen(preview.position, rect),
                            self.zoom * 1.8,
                            Color32::from_rgba_unmultiplied(255, 70, 40, (alpha * 120.0) as u8),
                        );
                    }
                    draw_component(
                        &painter,
                        &preview,
                        self.screen(preview.position, rect),
                        self.zoom,
                        self.selected == Some(ObjectId::Component(component.id)),
                        false,
                    );
                    if matches!(
                        component.kind,
                        ComponentKind::Counter4 | ComponentKind::Counter10
                    ) && let Some(state) = operating.and_then(|p| p.digital_state)
                    {
                        let center = self.screen(preview.position, rect);
                        painter.rect_filled(
                            Rect::from_center_size(center, Vec2::new(28.0, 12.0)),
                            0.0,
                            bg,
                        );
                        painter.text(
                            center,
                            egui::Align2::CENTER_CENTER,
                            format!("{state:02}"),
                            egui::FontId::monospace(10.0),
                            theme.gold,
                        );
                    }
                    if self.settings.voltage_colors
                        && let Some(point) = operating
                    {
                        for (pin, voltage) in preview.pins().iter().zip(&point.pin_voltages) {
                            painter.circle_filled(
                                self.screen(pin.position, rect),
                                2.5,
                                simulation_ui::voltage_color(*voltage, self.settings.voltage_scale),
                            );
                        }
                    }
                    if self.settings.current_dots
                        && let Some(point) = operating
                        && component.kind.pin_count() == 2
                        && point.current.abs() > 1e-12
                    {
                        let pins = preview.pins();
                        let a = self.screen(pins[0].position, rect);
                        let b = self.screen(pins[1].position, rect);
                        let clock = if self.live.as_ref().is_some_and(|w| w.paused) {
                            self.result
                                .as_ref()
                                .and_then(|r| r.traces.first())
                                .and_then(|t| t.samples.last())
                                .map_or(0.0, |s| s[0] * 1000.0)
                        } else {
                            ctx.input(|i| i.time)
                        };
                        let phase = (clock
                            * point.current.signum()
                            * (point.current.abs().log10() + 12.0).clamp(0.5, 10.0)
                            * 0.12)
                            .rem_euclid(1.0);
                        for i in 0..4 {
                            let f = (phase + i as f64 / 4.0).rem_euclid(1.0) as f32;
                            if !(0.29..=0.71).contains(&f) {
                                painter.circle_filled(a + (b - a) * f, 2.2, theme.gold);
                            }
                        }
                        if !self.live.as_ref().is_some_and(|w| w.paused) {
                            ctx.request_repaint_after(Duration::from_millis(32));
                        }
                    }
                }
                if let Some(pos) = response.hover_pos()
                    && let Some(ObjectId::Component(id)) = self.hit(pos, rect)
                    && let Some(c) = self.engine.project().components.get(&id)
                {
                    response.clone().on_hover_ui(|ui| {
                        ui.strong(format!("{} · {}", c.reference, c.kind.label()));
                        if matches!(c.kind, ComponentKind::Switch | ComponentKind::LogicInput) {
                            ui.label("Dois cliques para alternar o estado.");
                        }
                        if let Some(point) = solved
                            .and_then(|r| r.operating_points.iter().find(|p| p.component == id))
                        {
                            ui.monospace(format!(
                                "U: {:.6} V · I: {:.6} A · P: {:.6} W",
                                point.voltage, point.current, point.power
                            ));
                        }
                    });
                }
                for junction in self.engine.project().junctions.values() {
                    painter.circle_filled(
                        self.screen(junction.position, rect),
                        4.5,
                        if self.selected == Some(ObjectId::Junction(junction.id)) {
                            theme.gold
                        } else {
                            theme.accent
                        },
                    );
                }
                let mut label_anchors = std::collections::BTreeMap::new();
                for c in self
                    .engine
                    .project()
                    .components
                    .values()
                    .filter(|c| c.imported.is_none())
                {
                    for pin in c.pins() {
                        let delta = (pin.position.x - c.position.x, pin.position.y - c.position.y);
                        let anchor = if (c.device.is_some() || c.kind.pin_count() > 3)
                            && delta.0 < 0
                        {
                            (egui::Align2::RIGHT_CENTER, Vec2::new(-6.0, 0.0))
                        } else if (c.device.is_some() || c.kind.pin_count() > 3) && delta.0 > 0 {
                            (egui::Align2::LEFT_CENTER, Vec2::new(6.0, 0.0))
                        } else if delta.0 < 0 {
                            (egui::Align2::RIGHT_BOTTOM, Vec2::new(-5.0, -5.0))
                        } else if delta.0 > 0 {
                            (egui::Align2::LEFT_BOTTOM, Vec2::new(5.0, -5.0))
                        } else if delta.1 > 0 {
                            (egui::Align2::CENTER_TOP, Vec2::new(0.0, 6.0))
                        } else if delta.1 < 0 {
                            (egui::Align2::CENTER_BOTTOM, Vec2::new(0.0, -6.0))
                        } else {
                            (egui::Align2::LEFT_TOP, Vec2::new(6.0, 5.0))
                        };
                        label_anchors.insert(pin.position, anchor);
                    }
                }
                let device_pins: std::collections::BTreeMap<_, _> = self
                    .engine
                    .project()
                    .components
                    .values()
                    .filter_map(|c| c.device.as_ref().map(|d| (c, d)))
                    .flat_map(|(c, d)| {
                        c.pins().into_iter().enumerate().map(move |(i, p)| {
                            (p.position, (c.id, d.pins[i].electrical_type == "power_in"))
                        })
                    })
                    .collect();
                let mut drawn_rails = std::collections::BTreeSet::new();
                for label in self.engine.project().labels.values() {
                    let pos = self.screen(label.position, rect);
                    let color = if self.selected == Some(ObjectId::Label(label.id)) {
                        theme.gold
                    } else {
                        theme.accent
                    };
                    painter.circle_filled(pos, 3.0, color);
                    if let Some((id, true)) = device_pins.get(&label.position)
                        && !drawn_rails.insert((*id, label.name.clone()))
                    {
                        continue;
                    }
                    let (anchor, offset) = label_anchors
                        .get(&label.position)
                        .copied()
                        .unwrap_or((egui::Align2::LEFT_BOTTOM, Vec2::new(4.0, -4.0)));
                    painter.text(
                        pos + offset,
                        anchor,
                        &label.name,
                        egui::FontId::monospace(if device_pins.contains_key(&label.position) {
                            (self.zoom * 0.9).clamp(5.0, 10.0)
                        } else {
                            13.0
                        }),
                        color,
                    );
                }
                if response.hovered()
                    && let Some(pos) = response.hover_pos()
                {
                    let point = self.placement_point(pos, rect);
                    let screen = self.screen(point, rect);
                    match self.tool {
                        Tool::Place(kind) => {
                            let component = Component {
                                id: eda_core::ComponentId(0),
                                reference: format!("{}?", kind.prefix()),
                                kind,
                                value: kind.default_value(),
                                position: point,
                                rotation: self.placement_rotation,
                                parameters: Default::default(),
                                device: None,
                                footprint: None,
                                imported: None,
                            };
                            let component = if kind == ComponentKind::Device {
                                self.library_component(point).unwrap_or(component)
                            } else {
                                component
                            };
                            draw_component(&painter, &component, screen, self.zoom, false, true);
                        }
                        Tool::Wire => {
                            if let Some(start) = self.wire_start {
                                let corner =
                                    geometry::orthogonal_corner(start, point, self.wire_vertical);
                                painter.line(
                                    vec![
                                        self.screen(start, rect),
                                        self.screen(corner, rect),
                                        screen,
                                    ],
                                    Stroke::new(2.0_f32, theme.gold),
                                );
                            }
                        }
                        Tool::Junction => {
                            painter.circle_stroke(screen, 6.0, Stroke::new(2.0_f32, theme.gold));
                        }
                        Tool::Label => {
                            painter.text(
                                screen + Vec2::new(4.0, -4.0),
                                egui::Align2::LEFT_BOTTOM,
                                &self.label_name,
                                egui::FontId::monospace(13.0),
                                theme.gold,
                            );
                        }
                        Tool::Select => {}
                    }
                    painter.line_segment(
                        [screen - Vec2::new(6.0, 0.0), screen + Vec2::new(6.0, 0.0)],
                        Stroke::new(1.0_f32, Color32::from_gray(90)),
                    );
                    painter.line_segment(
                        [screen - Vec2::new(0.0, 6.0), screen + Vec2::new(0.0, 6.0)],
                        Stroke::new(1.0_f32, Color32::from_gray(90)),
                    );
                    painter.text(
                        rect.right_bottom() - Vec2::new(14.0, 12.0),
                        egui::Align2::RIGHT_BOTTOM,
                        format!("x {}  y {}", point.x, point.y),
                        egui::FontId::monospace(12.0),
                        theme.muted,
                    );
                }
                let tool = match self.tool {
                    Tool::Select => "SELECIONAR",
                    Tool::Place(kind) => kind.label(),
                    Tool::Wire => {
                        if self.wire_start.is_some() {
                            "FIO · clique para continuar, Esc para terminar"
                        } else {
                            "FIO · clique no primeiro terminal"
                        }
                    }
                    Tool::Junction => "JUNÇÃO",
                    Tool::Label => "NOME DE REDE",
                };
                painter.text(
                    rect.left_top() + Vec2::new(18.0, 16.0),
                    egui::Align2::LEFT_TOP,
                    format!("{}   /   {tool}", self.engine.project().name),
                    egui::FontId::proportional(14.0),
                    theme.muted,
                );
            });
    }

    fn placement_point(&self, pos: Pos2, rect: Rect) -> Point {
        if self.snap {
            let nearest = self
                .engine
                .project()
                .components
                .values()
                .flat_map(Component::pins)
                .map(|pin| pin.position)
                .chain(
                    self.engine
                        .project()
                        .wires
                        .values()
                        .flat_map(|wire| [wire.start, wire.end]),
                )
                .map(|point| (point, self.screen(point, rect).distance(pos)))
                .filter(|(_, distance)| *distance < 11.0)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((point, _)) = nearest {
                return point;
            }
        }
        self.point(pos, rect)
    }
    fn draw_grid(&self, painter: &egui::Painter, rect: Rect) {
        let spacing = grid_spacing(self.zoom);
        let min = self.point(rect.left_top(), rect);
        let max = self.point(rect.right_bottom(), rect);
        let color = if self.dark {
            Color32::from_rgb(56, 59, 63)
        } else {
            Color32::from_rgb(220, 221, 216)
        };
        let start_x = min.x + (-min.x).rem_euclid(spacing);
        let start_y = min.y + (-min.y).rem_euclid(spacing);
        for x in (start_x..=max.x).step_by(spacing as usize) {
            for y in (start_y..=max.y).step_by(spacing as usize) {
                painter.circle_filled(self.screen(Point { x, y }, rect), 0.8, color);
            }
        }
    }
}

impl eframe::App for ProtoCraft {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.capture_path.is_some() {
            if !self.capture_requested
                && self.simulation.is_none()
                && self.capture_started.elapsed() > Duration::from_millis(600)
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
                self.capture_requested = true;
            }
            let image = ctx.input(|input| {
                input.events.iter().find_map(|event| {
                    if let egui::Event::Screenshot { image, .. } = event {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
            });
            if let Some(image) = image
                && let Some(path) = self.capture_path.take()
            {
                match save_screenshot(&path, &image) {
                    Ok(()) => self.message(format!("Captura salva em {}", path.display()), false),
                    Err(error) => self.message(format!("Falha ao salvar captura: {error}"), true),
                }
            }
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.close_allowed && self.dirty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Close);
        }
        if let Some(started) = self.smoke_started {
            if (started.elapsed() > Duration::from_secs(3)
                && self.simulation.is_none()
                && self.capture_path.is_none())
                || started.elapsed() > Duration::from_secs(15)
            {
                self.close_allowed = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        }
        self.poll_simulation(ctx);
        self.poll_live(ctx);
        self.shortcuts(ctx);
        self.refresh_derived();
        self.top(ctx);
        if !self.pcb.active {
            self.bottom(ctx);
        }
        if self.show_library && !self.pcb.active {
            self.mode_strip(ctx);
            self.library(ctx);
        }
        if self.pcb.active {
            self.pcb_tools(ctx);
            self.pcb_canvas(ctx);
        } else {
            self.properties(ctx);
            self.canvas(ctx);
        }
        self.confirmation(ctx);
        self.settings_window(ctx);
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "{}{} — ProtoCraft",
            self.engine.project().name,
            if self.dirty() { " *" } else { "" }
        )));
    }
}

impl Drop for ProtoCraft {
    fn drop(&mut self) {
        if let Some(control) = &self.simulation_control {
            control.cancel();
        }
    }
}

fn save_screenshot(
    path: &std::path::Path,
    image: &egui::ColorImage,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(file),
        u32::try_from(image.width())?,
        u32::try_from(image.height())?,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    let bytes: Vec<u8> = image
        .pixels
        .iter()
        .flat_map(|pixel| pixel.to_array())
        .collect();
    writer.write_image_data(&bytes)?;
    writer.finish()?;
    Ok(())
}

fn load_brand_texture(ctx: &egui::Context) -> Result<egui::TextureHandle, String> {
    let decoder = png::Decoder::new(std::io::Cursor::new(include_bytes!(
        "../../../assets/protocraft-logo.png"
    )));
    let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
    let output_size = reader
        .output_buffer_size()
        .ok_or_else(|| "A imagem da marca excede o limite de pixels.".to_string())?;
    let mut bytes = vec![0; output_size];
    let info = reader
        .next_frame(&mut bytes)
        .map_err(|error| error.to_string())?;
    let pixels = match info.color_type {
        png::ColorType::Rgba => bytes[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => bytes[..info.buffer_size()]
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
            .collect(),
        png::ColorType::Grayscale => bytes[..info.buffer_size()]
            .iter()
            .flat_map(|value| [*value, *value, *value, 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => bytes[..info.buffer_size()]
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|pixel| [pixel[0], pixel[0], pixel[0], pixel[1]])
            .collect(),
        png::ColorType::Indexed => return Err("A imagem da marca usa paleta não expandida.".into()),
    };
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [info.width as usize, info.height as usize],
        &pixels,
    );
    Ok(ctx.load_texture("protocraft-logo", image, egui::TextureOptions::LINEAR))
}

fn grid_spacing(zoom: f32) -> i32 {
    (12.0 / zoom.max(0.25)).ceil().max(1.0) as i32
}

fn stored_import_notes(project: &Project) -> Vec<String> {
    match &project.import_info {
        Some(info) => {
            let mut notes = vec![format!(
                "Origem: {} · conversão elétrica {}",
                info.source_format,
                if info.electrical_complete {
                    "completa no subconjunto suportado"
                } else {
                    "parcial"
                }
            )];
            notes.extend(info.notes.iter().cloned());
            notes
        }
        None => Vec::new(),
    }
}

fn import_report_notes(report: eda_import::ImportReport) -> Vec<String> {
    let mut notes = vec![
        format!(
            "{} · FILEVER {} · {} componentes · {} fios · {} junções · {} rótulos",
            report.format,
            report
                .file_version
                .map(|v| v.to_string())
                .unwrap_or_else(|| "não informado".into()),
            report.components,
            report.wires,
            report.junctions,
            report.labels
        ),
        if report.electrical_complete {
            "Conversão elétrica do subconjunto suportado concluída. Confira ERC e modelos antes de simular.".into()
        } else {
            "Importação parcial: há informações elétricas ou modelos não suportados. Consulte os avisos antes de simular.".into()
        },
    ];
    notes.extend(report.warnings);
    if !report.unsupported_components.is_empty() {
        notes.push(format!(
            "Sem modelo: {}",
            report.unsupported_components.join(", ")
        ));
    }
    notes
}

fn is_proteus_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdsprj") || e.eq_ignore_ascii_case("dsn"))
}

fn component_bounds(component: &Component) -> Rect {
    let origin = Pos2::new(component.position.x as f32, component.position.y as f32);
    let mut bounds = Rect::from_center_size(origin, Vec2::new(4.0, 2.8));
    for pin in component.pins() {
        bounds.extend_with(Pos2::new(pin.position.x as f32, pin.position.y as f32));
    }
    bounds
}

fn draw_component(
    painter: &egui::Painter,
    component: &Component,
    center: Pos2,
    zoom: f32,
    selected: bool,
    ghost: bool,
) {
    let theme = Palette::new(painter.ctx());
    let color = if ghost {
        theme.accent.gamma_multiply(0.55)
    } else if selected {
        theme.gold
    } else {
        painter.ctx().style().visuals.text_color()
    };
    let stroke = Stroke::new(if selected { 2.5_f32 } else { 1.8_f32 }, color);
    if component.device.is_some() {
        device_ui::draw_device(painter, component, center, zoom, color);
        return;
    }
    if let Some(imported) = &component.imported {
        let bounds = component_bounds(component);
        let origin = Vec2::new(component.position.x as f32, component.position.y as f32);
        let to_screen = |point: Pos2| center + (point.to_vec2() - origin) * zoom;
        let body = Rect::from_center_size(center, Vec2::new(3.4, 2.2) * zoom);
        let model_color = if component.kind == ComponentKind::Imported {
            theme.gold
        } else {
            theme.accent
        };
        painter.rect_stroke(
            body,
            2.0,
            Stroke::new(stroke.width, if selected { color } else { model_color }),
            egui::StrokeKind::Inside,
        );
        painter.text(
            center - Vec2::new(0.0, 7.0),
            egui::Align2::CENTER_CENTER,
            &component.reference,
            egui::FontId::proportional(12.0),
            color,
        );
        painter.text(
            center + Vec2::new(0.0, 8.0),
            egui::Align2::CENTER_CENTER,
            &imported.device,
            egui::FontId::monospace(10.0),
            color,
        );
        if !imported.value_text.is_empty() {
            painter.text(
                Pos2::new(center.x, body.bottom() + 5.0),
                egui::Align2::CENTER_TOP,
                &imported.value_text,
                egui::FontId::monospace(10.0),
                theme.muted,
            );
        }
        for (index, pin) in component.pins().iter().enumerate() {
            let position = to_screen(Pos2::new(pin.position.x as f32, pin.position.y as f32));
            painter.circle_stroke(position, 3.0, Stroke::new(1.5_f32, model_color));
            if let Some(original) = imported.pins.get(index) {
                painter.text(
                    position + Vec2::new(4.0, -5.0),
                    egui::Align2::LEFT_BOTTOM,
                    &original.number,
                    egui::FontId::monospace(9.0),
                    theme.muted,
                );
            }
        }
        if selected {
            painter.rect_stroke(
                Rect::from_min_max(to_screen(bounds.min), to_screen(bounds.max)).expand(5.0),
                0.0,
                Stroke::new(1.0_f32, theme.gold),
                egui::StrokeKind::Outside,
            );
        }
        return;
    }
    let transform = |x: f32, y: f32| {
        let (x, y) = match component.rotation % 4 {
            0 => (x, y),
            1 => (-y, x),
            2 => (-x, -y),
            _ => (y, -x),
        };
        center + Vec2::new(x, y) * zoom
    };
    let line = |a: (f32, f32), b: (f32, f32)| {
        painter.line_segment([transform(a.0, a.1), transform(b.0, b.1)], stroke);
    };
    let poly = |points: &[(f32, f32)]| {
        painter.line(
            points.iter().map(|&(x, y)| transform(x, y)).collect(),
            stroke,
        );
    };
    match component.kind {
        ComponentKind::Nmos | ComponentKind::Pmos => {
            line((-3.0, 0.0), (-1.3, 0.0));
            line((-1.3, -1.5), (-1.3, 1.5));
            line((-0.8, -1.5), (-0.8, 1.5));
            poly(&[(-0.8, -1.1), (1.0, -2.0), (3.0, -2.0)]);
            poly(&[(-0.8, 1.1), (1.0, 2.0), (3.0, 2.0)]);
            if component.kind == ComponentKind::Nmos {
                poly(&[(-0.3, -0.35), (-0.8, 0.0), (-0.3, 0.35)]);
            } else {
                poly(&[(-0.8, -0.35), (-0.3, 0.0), (-0.8, 0.35)]);
            }
        }
        ComponentKind::Comparator
        | ComponentKind::Schmitt
        | ComponentKind::Timer555
        | ComponentKind::DFlipFlop
        | ComponentKind::Counter4
        | ComponentKind::Counter10
        | ComponentKind::Vcvs
        | ComponentKind::Vccs => {
            let offsets: Vec<_> = (0..component.kind.pin_count())
                .map(|pin| component.kind.pin_offset(pin))
                .collect();
            let left = offsets.iter().map(|p| p.0).min().unwrap_or(-4) as f32 + 1.4;
            let right = offsets.iter().map(|p| p.0).max().unwrap_or(4) as f32 - 1.4;
            let top = offsets
                .iter()
                .filter(|p| p.0 != 0)
                .map(|p| p.1)
                .min()
                .unwrap_or(-3) as f32
                - 1.0;
            let bottom = offsets
                .iter()
                .filter(|p| p.0 != 0)
                .map(|p| p.1)
                .max()
                .unwrap_or(3) as f32
                + 1.0;
            poly(&[
                (left, top),
                (right, top),
                (right, bottom),
                (left, bottom),
                (left, top),
            ]);
            for (pin, (x, y)) in offsets.iter().enumerate() {
                let (x, y) = (*x as f32, *y as f32);
                let (end, anchor) = if x < 0.0 {
                    ((left, y), egui::Align2::LEFT_CENTER)
                } else if x > 0.0 {
                    ((right, y), egui::Align2::RIGHT_CENTER)
                } else if y < 0.0 {
                    ((x, top), egui::Align2::CENTER_TOP)
                } else {
                    ((x, bottom), egui::Align2::CENTER_BOTTOM)
                };
                line((x, y), end);
                let text_pos = if x < 0.0 {
                    (end.0 + 0.2, end.1)
                } else if x > 0.0 {
                    (end.0 - 0.2, end.1)
                } else {
                    end
                };
                painter.text(
                    transform(text_pos.0, text_pos.1),
                    anchor,
                    component.kind.pin_name(pin as u8),
                    egui::FontId::monospace(8.0),
                    color,
                );
            }
            let label = match component.kind {
                ComponentKind::Comparator => "CMP",
                ComponentKind::Schmitt => "SCH",
                ComponentKind::Timer555 => "555",
                ComponentKind::DFlipFlop => "D",
                ComponentKind::Counter4 => "BIN",
                ComponentKind::Counter10 => "DEC",
                ComponentKind::Vcvs => "E",
                _ => "G",
            };
            painter.text(
                center,
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::monospace(10.0),
                theme.muted,
            );
        }
        ComponentKind::Resistor | ComponentKind::Potentiometer => {
            line((-3.0, 0.0), (-1.8, 0.0));
            line((1.8, 0.0), (3.0, 0.0));
            poly(&[
                (-1.8, -0.65),
                (1.8, -0.65),
                (1.8, 0.65),
                (-1.8, 0.65),
                (-1.8, -0.65),
            ]);
        }
        ComponentKind::Switch => {
            line((-3.0, 0.0), (-1.4, 0.0));
            line((1.4, 0.0), (3.0, 0.0));
            line(
                (-1.4, 0.0),
                if component.parameters.closed {
                    (1.4, 0.0)
                } else {
                    (1.0, -1.2)
                },
            );
            painter.circle_stroke(transform(-1.4, 0.0), zoom * 0.15, stroke);
            painter.circle_stroke(transform(1.4, 0.0), zoom * 0.15, stroke);
        }
        ComponentKind::Npn | ComponentKind::Pnp => {
            line((-3.0, 0.0), (-1.0, 0.0));
            line((-1.0, -1.5), (-1.0, 1.5));
            poly(&[(-1.0, -0.7), (1.0, -2.0), (3.0, -2.0)]);
            poly(&[(-1.0, 0.7), (1.0, 2.0), (3.0, 2.0)]);
            if component.kind == ComponentKind::Npn {
                poly(&[(0.3, 2.0), (1.0, 2.0), (0.7, 1.3)]);
            } else {
                poly(&[(-0.6, 1.3), (-1.0, 0.7), (-0.2, 0.7)]);
            }
        }
        ComponentKind::OpAmp => {
            poly(&[(-1.6, -2.0), (1.8, 0.0), (-1.6, 2.0), (-1.6, -2.0)]);
            line((-3.0, -2.0), (-1.6, -1.0));
            line((-3.0, 2.0), (-1.6, 1.0));
            line((1.8, 0.0), (3.0, 0.0));
            painter.text(
                transform(-1.0, -0.9),
                egui::Align2::CENTER_CENTER,
                "+",
                egui::FontId::monospace(12.0),
                color,
            );
            painter.text(
                transform(-1.0, 0.9),
                egui::Align2::CENTER_CENTER,
                "−",
                egui::FontId::monospace(12.0),
                color,
            );
        }
        ComponentKind::LogicNot
        | ComponentKind::LogicAnd
        | ComponentKind::LogicOr
        | ComponentKind::LogicXor
        | ComponentKind::LogicNand
        | ComponentKind::LogicNor => {
            poly(&[
                (-1.8, -2.5),
                (1.8, -2.5),
                (1.8, 2.5),
                (-1.8, 2.5),
                (-1.8, -2.5),
            ]);
            if component.kind == ComponentKind::LogicNot {
                line((-3.0, 0.0), (-1.8, 0.0));
            } else {
                line((-3.0, -2.0), (-1.8, -2.0));
                line((-3.0, 2.0), (-1.8, 2.0));
            }
            line((1.8, 0.0), (3.0, 0.0));
            let text = match component.kind {
                ComponentKind::LogicAnd | ComponentKind::LogicNand => "&",
                ComponentKind::LogicOr | ComponentKind::LogicNor => "≥1",
                ComponentKind::LogicXor => "=1",
                _ => "1",
            };
            painter.text(
                center,
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::monospace(16.0),
                color,
            );
            if matches!(
                component.kind,
                ComponentKind::LogicNot | ComponentKind::LogicNand | ComponentKind::LogicNor
            ) {
                painter.circle_stroke(transform(2.1, 0.0), zoom * 0.25, stroke);
            }
        }
        ComponentKind::Capacitor => {
            line((-3.0, 0.0), (-0.35, 0.0));
            line((0.35, 0.0), (3.0, 0.0));
            line((-0.35, -1.0), (-0.35, 1.0));
            line((0.35, -1.0), (0.35, 1.0));
        }
        ComponentKind::Inductor => {
            line((-3.0, 0.0), (-1.6, 0.0));
            line((1.6, 0.0), (3.0, 0.0));
            let points: Vec<_> = (0..=64)
                .map(|i| {
                    let x = -1.6 + i as f32 * 3.2 / 64.0;
                    transform(
                        x,
                        -(i as f32 / 64.0 * std::f32::consts::PI * 4.0).sin().abs() * 0.9,
                    )
                })
                .collect();
            painter.line(points, stroke);
        }
        ComponentKind::Diode | ComponentKind::Led | ComponentKind::Zener => {
            line((-3.0, 0.0), (-1.0, 0.0));
            line((1.0, 0.0), (3.0, 0.0));
            poly(&[(-1.0, -1.0), (1.0, 0.0), (-1.0, 1.0), (-1.0, -1.0)]);
            line((1.0, -1.0), (1.0, 1.0));
        }
        ComponentKind::VoltageSource
        | ComponentKind::CurrentSource
        | ComponentKind::SineVoltage
        | ComponentKind::SquareVoltage
        | ComponentKind::TriangleVoltage
        | ComponentKind::SawVoltage
        | ComponentKind::LogicInput => {
            line((-3.0, 0.0), (-1.2, 0.0));
            line((1.2, 0.0), (3.0, 0.0));
            painter.circle_stroke(center, zoom * 1.2, stroke);
            if component.kind.is_waveform() {
                match component.kind {
                    ComponentKind::SineVoltage => {
                        painter.line(
                            (0..=32)
                                .map(|i| {
                                    let x = -0.8 + i as f32 * 1.6 / 32.0;
                                    transform(
                                        x,
                                        -0.45 * (i as f32 / 32.0 * std::f32::consts::TAU).sin(),
                                    )
                                })
                                .collect(),
                            stroke,
                        );
                    }
                    ComponentKind::SquareVoltage => poly(&[
                        (-0.8, 0.4),
                        (-0.4, 0.4),
                        (-0.4, -0.4),
                        (0.4, -0.4),
                        (0.4, 0.4),
                        (0.8, 0.4),
                    ]),
                    ComponentKind::TriangleVoltage => poly(&[(-0.8, 0.4), (0.0, -0.4), (0.8, 0.4)]),
                    _ => poly(&[(-0.8, 0.4), (0.6, -0.4), (0.6, 0.4), (0.8, 0.3)]),
                }
            } else if component.kind == ComponentKind::LogicInput {
                painter.text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    if component.parameters.closed {
                        "H"
                    } else {
                        "L"
                    },
                    egui::FontId::monospace(15.0),
                    color,
                );
            } else if matches!(component.kind, ComponentKind::VoltageSource) {
                line((-0.85, 0.0), (-0.25, 0.0));
                line((-0.55, -0.3), (-0.55, 0.3));
                line((0.25, 0.0), (0.85, 0.0));
            } else {
                line((-0.65, 0.0), (0.65, 0.0));
                line((0.65, 0.0), (0.15, -0.35));
                line((0.65, 0.0), (0.15, 0.35));
            }
        }
        ComponentKind::Ground => {
            line((0.0, 0.0), (0.0, 0.7));
            line((-1.0, 0.7), (1.0, 0.7));
            line((-0.65, 1.0), (0.65, 1.0));
            line((-0.3, 1.3), (0.3, 1.3));
        }
        ComponentKind::Connector => {
            line((0.0, 0.0), (1.0, 0.0));
            painter.circle_stroke(transform(1.4, 0.0), zoom * 0.4, stroke);
        }
        ComponentKind::Imported | ComponentKind::Device => {
            poly(&[
                (-2.0, -1.0),
                (2.0, -1.0),
                (2.0, 1.0),
                (-2.0, 1.0),
                (-2.0, -1.0),
            ]);
        }
    }
    if component.kind == ComponentKind::Potentiometer {
        line((0.0, 3.0), (0.0, 0.7));
        poly(&[(-0.3, 1.1), (0.0, 0.7), (0.3, 1.1)]);
    }
    if component.kind == ComponentKind::Led {
        poly(&[(0.0, -1.3), (0.8, -2.1), (0.8, -1.7)]);
        poly(&[(0.8, -1.3), (1.6, -2.1), (1.6, -1.7)]);
    }
    if component.kind == ComponentKind::Zener {
        line((1.0, -1.0), (0.6, -1.3));
        line((1.0, 1.0), (1.4, 1.3));
    }
    // O modelo fornece as posições; o renderer não redefine a geometria dos pinos.
    for pin in component.pins() {
        let delta = Vec2::new(
            (pin.position.x - component.position.x) as f32,
            (pin.position.y - component.position.y) as f32,
        );
        painter.circle_stroke(
            center + delta * zoom,
            3.0,
            Stroke::new(1.0_f32, if ghost { color } else { theme.accent }),
        );
    }
    if selected {
        painter.circle_stroke(
            center,
            zoom * 3.6,
            Stroke::new(0.7_f32, theme.gold.gamma_multiply(0.3)),
        );
    }
    let label_offset = if component.kind.pin_count() > 3 {
        (0..component.kind.pin_count())
            .map(|pin| component.kind.pin_offset(pin).1.abs())
            .max()
            .unwrap_or(3) as f32
            + 2.2
    } else if component.kind.pin_count() > 2 {
        3.3
    } else if component.kind == ComponentKind::Led {
        2.6
    } else {
        1.4
    };
    let (reference_pos, value_pos) = if component.rotation.is_multiple_of(2) {
        (
            center + Vec2::new(0.0, -zoom * label_offset),
            center + Vec2::new(0.0, zoom * label_offset),
        )
    } else {
        (
            center + Vec2::new(zoom * 1.6, -8.0),
            center + Vec2::new(zoom * 1.6, 10.0),
        )
    };
    let anchor = if component.rotation.is_multiple_of(2) {
        egui::Align2::CENTER_CENTER
    } else {
        egui::Align2::LEFT_CENTER
    };
    painter.text(
        reference_pos,
        anchor,
        &component.reference,
        egui::FontId::proportional(13.0),
        color,
    );
    if !component.kind.requires_mixed()
        && !matches!(
            component.kind,
            ComponentKind::Ground
                | ComponentKind::Connector
                | ComponentKind::Switch
                | ComponentKind::OpAmp
        )
    {
        painter.text(
            value_pos,
            anchor,
            format!("{} {}", engineering(component.value), component.kind.unit()),
            egui::FontId::monospace(11.0),
            if ghost { color } else { theme.muted },
        );
    }
}

fn preferred_trace(result: &SimulationResult) -> usize {
    ["V(VOUT)", "V(Q)", "V(Q0)", "V(OUT)"]
        .iter()
        .find_map(|name| result.traces.iter().position(|trace| trace.name == *name))
        .or_else(|| {
            result
                .traces
                .iter()
                .position(|trace| trace.name.starts_with("V(") && trace.name != "V(0)")
        })
        .unwrap_or(0)
}

/// Resolve também um destino ainda não criado, preservando a identidade do diretório real.
fn canonical_target(path: &std::path::Path) -> Result<PathBuf, String> {
    match std::fs::canonicalize(path) {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let absolute = std::path::absolute(path)
                .map_err(|error| format!("Não foi possível resolver o destino: {error}"))?;
            let parent = absolute
                .parent()
                .ok_or("O destino precisa ter um diretório válido.")?;
            let name = absolute
                .file_name()
                .ok_or("O destino precisa ter um nome de arquivo.")?;
            std::fs::canonicalize(parent)
                .map(|parent| parent.join(name))
                .map_err(|error| {
                    format!("Não foi possível verificar o diretório do destino: {error}")
                })
        }
        Err(error) => Err(format!("Não foi possível verificar o destino: {error}")),
    }
}

fn ensure_export_destination(
    path: &std::path::Path,
    project_path: Option<&std::path::Path>,
) -> Result<(), String> {
    let Some(project_path) = project_path else {
        return Ok(());
    };
    let project_path = canonical_target(project_path)?;
    let destination = canonical_target(path)?;
    #[cfg(windows)]
    let same_file = project_path
        .to_string_lossy()
        .eq_ignore_ascii_case(&destination.to_string_lossy());
    #[cfg(not(windows))]
    let same_file = project_path == destination;
    if same_file {
        return Err(
            "Escolha outro arquivo: a exportação não pode sobrescrever o projeto aberto.".into(),
        );
    }
    Ok(())
}

fn engineering(value: f64) -> String {
    for (factor, suffix) in [
        (1e9, "G"),
        (1e6, "M"),
        (1e3, "k"),
        (1.0, ""),
        (1e-3, "m"),
        (1e-6, "µ"),
        (1e-9, "n"),
        (1e-12, "p"),
    ] {
        if value.abs() >= factor {
            return format!("{:.3}{}", value / factor, suffix).replace(".000", "");
        }
    }
    format!("{value:.3e}")
}

fn draw_plot(ui: &mut egui::Ui, trace: &eda_simulation::Trace) {
    let theme = Palette::new(ui.ctx());
    let (response, painter) = ui.allocate_painter(
        Vec2::new(ui.available_width(), ui.available_height().max(90.0)),
        Sense::hover(),
    );
    let rect = response.rect.shrink2(Vec2::new(65.0, 24.0));
    if trace.samples.is_empty() || rect.width() < 30.0 || rect.height() < 20.0 {
        return;
    }
    let min_x = trace.samples.first().map(|s| s[0]).unwrap_or(0.0);
    let max_x = trace
        .samples
        .last()
        .map(|s| s[0])
        .unwrap_or(1.0)
        .max(min_x + 1e-15);
    let min_y = trace.samples.iter().map(|s| s[1]).fold(0.0, f64::min);
    let max_y = trace.samples.iter().map(|s| s[1]).fold(0.0, f64::max);
    let span_y = (max_y - min_y).max(1e-9);
    let min_y = min_y - span_y * 0.05;
    let span_y = span_y * 1.1;
    let position = |sample: [f64; 2]| {
        Pos2::new(
            rect.left() + ((sample[0] - min_x) / (max_x - min_x)) as f32 * rect.width(),
            rect.bottom() - ((sample[1] - min_y) / span_y) as f32 * rect.height(),
        )
    };
    for i in 0..=4 {
        let frac = i as f32 / 4.0;
        let x = egui::lerp(rect.x_range(), frac);
        let y = egui::lerp(rect.y_range(), frac);
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(0.6_f32, Color32::from_gray(65)),
        );
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            Stroke::new(0.6_f32, Color32::from_gray(65)),
        );
        painter.text(
            Pos2::new(x, rect.bottom() + 6.0),
            egui::Align2::CENTER_TOP,
            format!(
                "{}s",
                engineering(min_x + (max_x - min_x) * f64::from(frac))
            ),
            egui::FontId::monospace(10.0),
            theme.muted,
        );
        painter.text(
            Pos2::new(rect.left() - 8.0, y),
            egui::Align2::RIGHT_CENTER,
            engineering(min_y + span_y * (1.0 - f64::from(frac))),
            egui::FontId::monospace(10.0),
            theme.muted,
        );
    }
    let stride = (trace.samples.len() / 4000).max(1);
    painter.line(
        trace
            .samples
            .iter()
            .step_by(stride)
            .copied()
            .map(position)
            .collect(),
        Stroke::new(2.0_f32, theme.accent),
    );
    painter.text(
        rect.left_top() - Vec2::new(0.0, 8.0),
        egui::Align2::LEFT_BOTTOM,
        format!("{} [{}]", trace.name, trace.unit),
        egui::FontId::proportional(12.0),
        theme.accent,
    );
    if let Some(pointer) = response.hover_pos()
        && rect.contains(pointer)
    {
        let time = min_x + f64::from((pointer.x - rect.left()) / rect.width()) * (max_x - min_x);
        let index = trace
            .samples
            .partition_point(|sample| sample[0] < time)
            .min(trace.samples.len() - 1);
        let sample = trace.samples[index];
        let pos = position(sample);
        painter.line_segment(
            [
                Pos2::new(pos.x, rect.top()),
                Pos2::new(pos.x, rect.bottom()),
            ],
            Stroke::new(1.0_f32, theme.gold),
        );
        painter.circle_filled(pos, 4.0, theme.gold);
        response.on_hover_text(format!(
            "t = {:.7} s\n{} = {:.7} {}",
            sample[0], trace.name, sample[1], trace.unit
        ));
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let mut smoke = false;
    let mut capture_path = None;
    let mut path = None;
    let mut light = false;
    let mut startup_analysis = None;
    let mut startup_engine = None;
    let mut startup_pcb = false;
    let mut startup_live = false;
    let mut startup_settings = false;
    let mut window_position = None;
    let mut window_size = [1440_u32, 900_u32];
    while let Some(arg) = args.next() {
        if arg == "--smoke-exit" {
            smoke = true;
        } else if arg == "--continuous" {
            startup_live = true;
        } else if arg == "--settings" {
            startup_settings = true;
        } else if arg == "--pcb" {
            startup_pcb = true;
        } else if arg == "--light" {
            light = true;
        } else if arg == "--engine" {
            startup_engine = Some(eda_simulation::SimulationEngine::parse(
                &args
                    .next()
                    .ok_or("Informe stable, dynamic ou mixed após --engine.")?
                    .to_string_lossy(),
            )?);
        } else if arg == "--analysis" {
            let analysis = args
                .next()
                .ok_or("Informe dc ou transient após --analysis.")?;
            startup_analysis = Some(if analysis == "dc" {
                Analysis::Dc
            } else if analysis == "transient" {
                Analysis::Transient {
                    step: 10e-6,
                    stop: 5e-3,
                }
            } else {
                return Err("Análise desconhecida. Use dc ou transient.".into());
            });
        } else if arg == "--capture" {
            capture_path = Some(PathBuf::from(
                args.next()
                    .ok_or("Informe um caminho PNG após --capture.")?,
            ));
        } else if arg == "--window-position" {
            let x = args.next().ok_or("Informe X e Y após --window-position.")?;
            let y = args.next().ok_or("Informe X e Y após --window-position.")?;
            window_position = Some((
                x.to_string_lossy().parse::<i32>()?,
                y.to_string_lossy().parse::<i32>()?,
            ));
        } else if arg == "--window-size" {
            let width = args
                .next()
                .ok_or("Informe largura e altura após --window-size.")?;
            let height = args
                .next()
                .ok_or("Informe largura e altura após --window-size.")?;
            window_size = [
                width.to_string_lossy().parse::<u32>()?,
                height.to_string_lossy().parse::<u32>()?,
            ];
            if window_size[0] < 1000 || window_size[1] < 650 {
                return Err("A janela precisa ter ao menos 1000 × 650 pixels.".into());
            }
        } else if arg == "--help" || arg == "-h" {
            println!(
                "ProtoCraft [projeto.protocraft|projeto.pdsprj|esquema.dsn] [--analysis dc|transient] [--engine stable|dynamic|mixed] [--continuous] [--settings] [--pcb] [--light] [--capture captura.png] [--window-position X Y] [--window-size W H] [--smoke-exit]\n\n--analysis        Executa a análise ao abrir; transiente usa passo de 10 us e fim de 5 ms.\n--light           Abre com tema claro.\n--capture         Salva uma captura PNG após concluir a análise inicial.\n--window-position Define a posição inicial da janela em coordenadas da área de trabalho.\n--window-size     Define o tamanho inicial; mínimo de 1000 × 650 pixels.\n--smoke-exit      Fecha automaticamente após 3 s e conclusão da captura/análise (limite: 15 s)."
            );
            return Ok(());
        } else if arg.to_string_lossy().starts_with('-') || path.is_some() {
            return Err(format!("Argumento desconhecido: {}", arg.to_string_lossy()).into());
        } else {
            path = Some(PathBuf::from(arg));
        }
    }
    let mut startup_import = None;
    let mut startup_source = None;
    let (project, path, message) = if let Some(path) = path {
        let proteus = is_proteus_path(&path);
        let loaded = if proteus {
            eda_import::import_proteus(&path)
                .map(|result| {
                    startup_import = Some(result.report);
                    startup_source = Some(path.clone());
                    result.project
                })
                .map_err(|error| error.to_string())
        } else {
            eda_storage::load(&path).map_err(|error| error.to_string())
        };
        match loaded {
            Ok(project) => (project, if proteus { None } else { Some(path) }, None),
            Err(error) => (
                examples::voltage_divider(),
                None,
                Some(format!(
                    "Não foi possível abrir {}: {error}",
                    path.display()
                )),
            ),
        }
    } else {
        (examples::voltage_divider(), None, None)
    };
    let engine = Engine::new(project)?;
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([window_size[0] as f32, window_size[1] as f32])
        .with_min_inner_size([1000.0, 650.0]);
    if let Some((x, y)) = window_position {
        viewport = viewport.with_position([x as f32, y as f32]);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "ProtoCraft",
        options,
        Box::new(move |cc| {
            apply_theme(&cc.egui_ctx, !light);
            let logo = match load_brand_texture(&cc.egui_ctx) {
                Ok(texture) => Some(texture),
                Err(error) => {
                    eprintln!("Não foi possível carregar a marca do ProtoCraft: {error}");
                    None
                }
            };
            let mut app = ProtoCraft::new(engine, logo, path, message, smoke, capture_path);
            if let Some(report) = startup_import {
                app.unsaved_import = true;
                app.import_source = startup_source;
                app.import_notes = import_report_notes(report);
                app.tab = BottomTab::Import;
                app.message(
                    "Proteus importado. Salve como .protocraft para preservar o resultado.",
                    false,
                );
            }
            app.dark = !light;
            app.focus_device_library();
            if startup_pcb {
                app.pcb.selected = app.selected.and_then(|o| {
                    if let ObjectId::Component(id) = o {
                        Some(id)
                    } else {
                        None
                    }
                });
                app.pcb.active = true;
                app.pcb.report = eda_pcb::check(app.engine.project()).ok();
            }
            match settings::Settings::load() {
                Ok(settings) => app.settings = settings,
                Err(e) => app.message(format!("Falha ao ler configurações: {e}"), true),
            }
            if startup_engine.is_none()
                && app
                    .engine
                    .project()
                    .components
                    .values()
                    .any(|c| c.kind.requires_mixed())
            {
                app.settings.engine = eda_simulation::SimulationEngine::Mixed;
            }
            if let Some(engine) = startup_engine {
                app.settings.engine = engine;
            }
            app.show_settings = startup_settings;
            if startup_live {
                app.start_live();
            }
            if let Some(analysis) = startup_analysis {
                app.simulation(analysis);
            }
            Ok(Box::new(app))
        }),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(project: Project) -> ProtoCraft {
        ProtoCraft::new(Engine::new(project).unwrap(), None, None, None, false, None)
    }

    #[test]
    fn fit_encloses_large_imported_pin_geometry_and_grid_stays_sparse() {
        let mut project = Project::empty("Esquema grande");
        let id = eda_core::ComponentId(1);
        project.components.insert(
            id,
            Component {
                id,
                reference: "U1".into(),
                kind: ComponentKind::Imported,
                value: 0.0,
                position: Point::new(0, 0),
                rotation: 0,
                parameters: Default::default(),
                device: None,
                footprint: None,
                imported: Some(eda_core::ImportedComponent {
                    source_format: "Proteus".into(),
                    device: "GENERIC".into(),
                    value_text: String::new(),
                    properties: Default::default(),
                    pins: [-500, 500]
                        .into_iter()
                        .enumerate()
                        .map(|(index, coordinate)| eda_core::ImportedPin {
                            number: (index + 1).to_string(),
                            name: String::new(),
                            offset: Point::new(coordinate, coordinate),
                        })
                        .collect(),
                }),
            },
        );
        project.next_id = 2;
        let mut app = app(project);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        app.fit(rect);
        assert!(app.zoom < 5.0 && app.zoom >= 0.25);
        for component in app.engine.project().components.values() {
            for pin in component.pins() {
                assert!(rect.contains(app.screen(pin.position, rect)));
            }
        }
        for zoom in [0.25, 0.5, 1.0, 4.0, 18.0] {
            let spacing = grid_spacing(zoom);
            assert!(spacing >= 1);
            assert!(spacing as f32 * zoom >= 12.0);
        }
        let spacing = grid_spacing(0.25) as f32 * 0.25;
        assert!((rect.width() / spacing) * (rect.height() / spacing) < 4000.0);
    }

    #[test]
    fn restored_imported_project_has_specific_startup_status() {
        let mut project = Project::empty("Importado");
        project.import_info = Some(eda_core::ImportedProjectInfo {
            source_format: "Proteus".into(),
            electrical_complete: false,
            notes: Vec::new(),
        });
        let app = ProtoCraft::new(
            Engine::new(project).unwrap(),
            None,
            Some(PathBuf::from("importado.protocraft")),
            None,
            false,
            None,
        );
        assert_eq!(
            app.status,
            "Projeto importado aberto. Consulte a aba Importação."
        );
    }

    #[test]
    fn embedded_brand_image_decodes_to_a_texture() {
        let texture = load_brand_texture(&egui::Context::default()).unwrap();
        let [width, height] = texture.size();
        assert!(width > 1200 && height > 1200);
        assert_eq!(width, height);
    }

    #[test]
    fn imported_pin_bounds_follow_model_and_can_be_selected() {
        let mut project = Project::empty("Importado");
        let id = eda_core::ComponentId(1);
        project.components.insert(
            id,
            Component {
                id,
                reference: "U1".into(),
                kind: ComponentKind::Imported,
                value: 0.0,
                position: Point::new(10, 20),
                rotation: 1,
                parameters: Default::default(),
                device: None,
                footprint: None,
                imported: Some(eda_core::ImportedComponent {
                    source_format: "Proteus".into(),
                    device: "GENERIC".into(),
                    value_text: "original".into(),
                    properties: Default::default(),
                    pins: vec![eda_core::ImportedPin {
                        number: "8".into(),
                        name: "VCC".into(),
                        offset: Point::new(20, 4),
                    }],
                }),
            },
        );
        project.next_id = 2;
        let app = app(project);
        let component = app.engine.project().components.get(&id).unwrap();
        let pin = component.pins()[0].position;
        assert!(component_bounds(component).contains(Pos2::new(pin.x as f32, pin.y as f32)));
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        assert_eq!(
            app.hit(app.screen(pin, rect), rect),
            Some(ObjectId::Component(id))
        );
    }

    #[test]
    fn failed_import_preserves_current_project() {
        let mut app = app(examples::voltage_divider());
        let before = app.engine.project().clone();
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("invalido.pdsprj");
        std::fs::write(&path, b"arquivo invalido").unwrap();
        app.import_path(&path);
        assert_eq!(app.engine.project(), &before);
        assert!(app.status_error);
        assert!(!app.unsaved_import);
    }

    #[test]
    fn imported_document_requires_save_and_restores_stored_warnings() {
        let mut project = Project::empty("Importado");
        project.import_info = Some(eda_core::ImportedProjectInfo {
            source_format: "Proteus".into(),
            electrical_complete: false,
            notes: vec!["Sem modelo MCU".into()],
        });
        let mut app = app(project);
        assert!(app.import_notes.iter().any(|note| note == "Sem modelo MCU"));
        assert!(matches!(app.tab, BottomTab::Import));
        app.unsaved_import = true;
        assert!(app.dirty());
        let ctx = egui::Context::default();
        app.request(Pending::Import, &ctx);
        assert!(matches!(app.pending, Some(Pending::Import)));
    }

    #[test]
    fn dirty_close_requires_review_and_new_cancels_previous_worker() {
        let mut app = app(examples::voltage_divider());
        let ctx = egui::Context::default();
        app.command(Command::RenameProject {
            name: "Editado".into(),
        });
        app.request(Pending::Close, &ctx);
        assert!(matches!(app.pending, Some(Pending::Close)));
        assert!(!app.close_allowed);
        let control = eda_simulation::SimulationControl::default();
        app.simulation_control = Some(control);
        let (_sender, receiver) = mpsc::channel();
        app.simulation = Some(receiver);
        app.replace(examples::rc_filter(), None);
        assert!(app.simulation.is_none());
        assert!(app.simulation_control.is_none());
        assert!(!app.dirty());
    }

    #[test]
    fn copy_paste_component_snapshot_and_select_pasted_copy() {
        let mut app = app(Project::empty("Área de transferência"));
        app.command(Command::PlaceComponent {
            kind: ComponentKind::Resistor,
            position: Point::new(-10, 5),
        });
        app.command(Command::SetComponent {
            id: eda_core::ComponentId(1),
            reference: "Rcustom".into(),
            value: 3_300.0,
        });
        app.command(Command::RotateComponent {
            id: eda_core::ComponentId(1),
        });
        app.choose(Some(ObjectId::Component(eda_core::ComponentId(1))));
        let ctx = egui::Context::default();
        app.copy_component(&ctx);
        app.command(Command::Delete {
            object: ObjectId::Component(eda_core::ComponentId(1)),
        });
        app.choose(None);
        app.paste_component(&ctx);

        assert_eq!(app.engine.project().components.len(), 1);
        let pasted = &app.engine.project().components[&eda_core::ComponentId(2)];
        assert_eq!(pasted.reference, "R1");
        assert_eq!(pasted.value, 3_300.0);
        assert_eq!(pasted.rotation, 1);
        assert_eq!(pasted.position, Point::new(-6, 9));
        assert_eq!(app.selected, Some(ObjectId::Component(pasted.id)));
        assert!(app.engine.undo());
        assert!(app.engine.project().components.is_empty());
        assert!(app.engine.redo());
        assert_eq!(app.engine.project().components.len(), 1);
    }

    #[test]
    fn canvas_click_places_rotated_component_as_one_undoable_command() {
        let mut app = app(Project::empty("Interação"));
        app.tool = Tool::Place(ComponentKind::Resistor);
        app.rotate();
        let ctx = egui::Context::default();
        let pos = Pos2::new(400.0, 300.0);
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
                ..Default::default()
            },
            |ctx| app.canvas(ctx),
        );
        assert!(!output.shapes.is_empty());
        for pressed in [true, false] {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
                events: vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            };
            let output = ctx.run(input, |ctx| app.canvas(ctx));
            assert!(!output.shapes.is_empty());
        }
        assert_eq!(app.engine.project().components.len(), 1);
        let component = app.engine.project().components.values().next().unwrap();
        assert_eq!(component.rotation, 1);
        assert_eq!(component.position, Point::new(0, 0));
        assert!(app.engine.undo());
        assert!(app.engine.project().components.is_empty());
        assert!(app.engine.redo());
        assert_eq!(app.engine.project().components.len(), 1);
    }

    #[test]
    fn coordinates_and_hit_detection_preserve_pin_positions() {
        let app = app(examples::voltage_divider());
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        for component in app.engine.project().components.values() {
            assert_eq!(
                app.hit(app.screen(component.position, rect), rect),
                Some(ObjectId::Component(component.id))
            );
            for pin in component.pins() {
                assert_eq!(
                    app.point(app.screen(pin.position, rect), rect),
                    pin.position
                );
            }
        }
    }

    #[test]
    fn background_result_keeps_original_revision_after_edit() {
        let mut app = app(examples::voltage_divider());
        app.simulation(Analysis::Dc);
        app.command(Command::RenameProject {
            name: "Revisão posterior".into(),
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        let ctx = egui::Context::default();
        while app.simulation.is_some() && Instant::now() < deadline {
            app.poll_simulation(&ctx);
            std::thread::yield_now();
        }
        let result = app
            .result
            .as_ref()
            .expect("A análise DC do exemplo precisa concluir.");
        assert_eq!(result.revision, 0);
        assert_eq!(result.traces[app.selected_trace].name, "V(VOUT)");
        assert_ne!(result.revision, app.engine.revision());
        assert!(app.status.contains("revisão anterior"));
        assert!(result.traces.iter().any(|trace| {
            trace.name.contains("VOUT")
                && trace
                    .samples
                    .iter()
                    .any(|sample| (sample[1] - 5.0).abs() < 1e-8)
        }));
    }

    #[test]
    fn trace_selection_prefers_output_then_non_ground_voltage() {
        let mut result = eda_simulation::simulate(&SimulationRequest {
            project: examples::voltage_divider(),
            revision: 0,
            analysis: Analysis::Dc,
        })
        .unwrap();
        assert_eq!(result.traces[preferred_trace(&result)].name, "V(VOUT)");
        result.traces.retain(|trace| trace.name != "V(VOUT)");
        assert!(
            result.traces[preferred_trace(&result)]
                .name
                .starts_with("V(")
        );
        assert_ne!(result.traces[preferred_trace(&result)].name, "V(0)");
        result.traces.retain(|trace| !trace.name.starts_with("V("));
        assert_eq!(preferred_trace(&result), 0);
    }

    #[test]
    fn mixed_chips_select_engine_fit_all_pins_and_prefer_counter_output() {
        let mut app = app(Project::empty("Bancada"));
        app.settings.engine = eda_simulation::SimulationEngine::Dynamic;
        app.replace(examples::decade_counter(), None);
        assert_eq!(app.settings.engine, eda_simulation::SimulationEngine::Mixed);
        let result = eda_simulation::simulate_with_engine(
            &SimulationRequest {
                project: app.engine.project().clone(),
                revision: 0,
                analysis: Analysis::Dc,
            },
            eda_simulation::SimulationEngine::Mixed,
            &Default::default(),
        )
        .unwrap();
        assert_eq!(result.traces[preferred_trace(&result)].name, "V(Q0)");
        let mut engine = Engine::new(Project::empty("Pinos")).unwrap();
        engine
            .execute(Command::PlaceComponent {
                kind: ComponentKind::Counter10,
                position: Point::new(0, 0),
            })
            .unwrap();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0));
        for _ in 0..4 {
            app.replace(engine.project().clone(), None);
            app.fit(rect);
            let component = &app.engine.project().components[&eda_core::ComponentId(1)];
            for pin in component.pins() {
                let pos = app.screen(pin.position, rect);
                assert!(rect.contains(pos));
                assert_eq!(app.hit(pos, rect), Some(ObjectId::Component(component.id)));
            }
            engine
                .execute(Command::RotateComponent {
                    id: eda_core::ComponentId(1),
                })
                .unwrap();
        }
    }

    #[test]
    fn exports_reject_project_path_alias_without_touching_file() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("original.protocraft");
        let child = directory.path().join("subpasta");
        std::fs::create_dir(&child).unwrap();
        std::fs::write(&project, b"projeto preservado").unwrap();
        let alias = child.join("..").join("original.protocraft");
        assert!(ensure_export_destination(&alias, Some(&project)).is_err());
        assert_eq!(std::fs::read(&project).unwrap(), b"projeto preservado");
        assert!(
            ensure_export_destination(&directory.path().join("resultado.csv"), Some(&project))
                .is_ok()
        );
    }
    #[test]
    fn canvas_double_click_changes_switch_and_undo_restores_state() {
        let mut app = app(examples::switched_led());
        let id = app
            .engine
            .project()
            .components
            .values()
            .find(|c| c.kind == ComponentKind::Switch)
            .unwrap()
            .id;
        let ctx = egui::Context::default();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0));
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(rect),
                time: Some(0.0),
                ..Default::default()
            },
            |ctx| app.canvas(ctx),
        );
        assert!(!output.shapes.is_empty());
        let pos = app.screen(
            app.engine.project().components[&id].position,
            app.canvas_rect.unwrap(),
        );
        for (index, pressed) in [true, false, true, false].into_iter().enumerate() {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(rect),
                    time: Some(0.1 + index as f64 * 0.05),
                    events: vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ctx| app.canvas(ctx),
            );
            assert!(!output.shapes.is_empty());
        }
        assert!(app.engine.project().components[&id].parameters.closed);
        assert!(app.engine.undo());
        assert!(!app.engine.project().components[&id].parameters.closed);
    }

    #[test]
    fn desktop_uses_selected_engine_for_background_analysis() {
        let mut app = app(examples::rc_filter());
        app.settings.engine = eda_simulation::SimulationEngine::Dynamic;
        app.simulation(Analysis::Transient {
            step: 1e-5,
            stop: 0.001,
        });
        let ctx = egui::Context::default();
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.simulation.is_some() && Instant::now() < deadline {
            app.poll_simulation(&ctx);
            std::thread::yield_now();
        }
        assert!(app.result.as_ref().unwrap().backend.contains("Dinâmico"));
    }
}
