use super::*;
use eda_simulation::SimulationEngine;

impl ProtoCraft {
    pub(super) fn stop_live(&mut self) {
        self.live = None;
    }
    pub(super) fn start_live(&mut self) {
        if !self.settings.validate() {
            self.message(
                tr!("Confira o passo e a velocidade nas configurações."),
                true,
            );
            return;
        }
        if let Some(control) = self.simulation_control.take() {
            control.cancel();
        }
        self.simulation = None;
        self.stop_live();
        match live::start(
            self.engine.project().clone(),
            self.engine.revision(),
            self.settings.engine,
            self.settings.live_step,
            self.settings.steps_per_frame,
        ) {
            Ok(worker) => {
                self.live = Some(worker);
                self.result = None;
                self.tab = BottomTab::Results;
                self.message(tr!("Simulação contínua iniciada. Dê dois cliques em chaves ou entradas lógicas para alternar."),false);
            }
            Err(e) => self.message(trf!("Não foi possível iniciar: {error}", error = e), true),
        }
    }
    pub(super) fn poll_live(&mut self, ctx: &egui::Context) {
        let mut error = None;
        let mut received = None;
        if let Some(worker) = &mut self.live {
            if worker.revision != self.engine.revision() {
                if worker
                    .sender
                    .send(live::Message::Update(
                        Box::new(self.engine.project().clone()),
                        self.engine.revision(),
                    ))
                    .is_err()
                {
                    error = Some(tr!("O motor contínuo encerrou."));
                } else {
                    worker.revision = self.engine.revision();
                }
            }
            match worker.receiver.try_recv() {
                Ok(result) => received = Some(result),
                Err(mpsc::TryRecvError::Disconnected) => {
                    error = Some(tr!("O motor contínuo encerrou."))
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        if let Some(result) = received {
            match result {
                Ok(result) => {
                    if self.result.is_none() || self.selected_trace >= result.traces.len() {
                        self.selected_trace = preferred_trace(&result);
                    }
                    self.result = Some(result);
                }
                Err(e) => error = Some(e),
            }
        }
        if let Some(error) = error {
            self.stop_live();
            self.message(
                trf!("Simulação contínua interrompida: {error}", error = error),
                true,
            );
        }
    }
    pub(super) fn live_controls(&mut self, ui: &mut egui::Ui) {
        if self
            .engine
            .project()
            .components
            .values()
            .any(|c| c.device.is_some())
        {
            ui.label(tr!("Projeto físico STM32 / ESP32 · pinagem e PCB"));
            return;
        }

        let engine_label = eda_i18n::text(self.settings.engine.label());
        ui.label(trf!("Motor: {engine}", engine = engine_label));
        if self.live.is_none() {
            if ui.button(tr!("▶ Contínua")).clicked() {
                self.start_live();
            }
        } else {
            let paused = self.live.as_ref().is_some_and(|w| w.paused);
            if ui
                .button(if paused {
                    tr!("▶ Continuar")
                } else {
                    tr!("Pausar")
                })
                .clicked()
                && let Some(worker) = &mut self.live
            {
                match worker.sender.send(live::Message::Pause(!paused)) {
                    Ok(()) => worker.paused = !paused,
                    Err(_) => self.message(tr!("O motor contínuo encerrou."), true),
                }
            }
            if paused
                && ui.button(tr!("Um passo")).clicked()
                && let Some(worker) = &self.live
                && worker.sender.send(live::Message::Step).is_err()
            {
                self.message(tr!("O motor contínuo encerrou."), true);
            }
            if ui.button(tr!("■ Parar")).clicked() {
                self.stop_live();
                self.message(tr!("Simulação contínua parada. A próxima execução usa os estados iniciais do circuito."),false);
            }
            if let Some(time) = self
                .result
                .as_ref()
                .and_then(|r| r.traces.first())
                .and_then(|t| t.samples.last())
            {
                ui.monospace(trf!("t = {time} s", time = format!("{:.6}", time[0])));
            }
        }
        if ui.button(tr!("Configurações…")).clicked() {
            self.show_settings = true;
        }
    }
    pub(super) fn settings_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_settings;
        let previous = self.settings.engine;
        let previous_language = self.settings.language;
        egui::Window::new(tr!("Configurações")).open(&mut open).default_width(460.0).show(ctx,|ui| {
            ui.heading(tr!("Idioma da interface"));
            egui::ComboBox::from_id_salt("ui_language")
                .selected_text(eda_i18n::text(eda_i18n::preference_name(self.settings.language)))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.settings.language, eda_i18n::LanguagePreference::System, eda_i18n::text("language.system"));
                    ui.selectable_value(&mut self.settings.language, eda_i18n::LanguagePreference::English, eda_i18n::text("language.english"));
                    ui.selectable_value(&mut self.settings.language, eda_i18n::LanguagePreference::PortugueseBr, eda_i18n::text("language.portuguese_br"));
                });
            ui.label(tr!("A seleção é aplicada imediatamente e fica salva ao escolher Salvar configurações."));
            ui.separator();
            ui.heading(tr!("Motor de simulação"));
            egui::ComboBox::from_id_salt("simulation_engine").selected_text(eda_i18n::text(self.settings.engine.label())).show_ui(ui,|ui| {
                for engine in SimulationEngine::all() {ui.selectable_value(&mut self.settings.engine,*engine,eda_i18n::text(engine.label()));}
            });
            ui.label(eda_i18n::text(self.settings.engine.description()));
            ui.label(tr!("Os motores usam modelos genéricos em Rust. Misto inclui eventos digitais; Estável e Dinâmico diferem na integração analógica."));
            ui.separator();ui.strong(tr!("Execução contínua"));
            ui.horizontal(|ui|{ui.label(tr!("Passo (s)"));ui.add(egui::DragValue::new(&mut self.settings.live_step).speed(1e-6).range(1e-12..=1.0).max_decimals(12));});
            ui.horizontal(|ui|{ui.label(tr!("Passos por atualização"));ui.add(egui::DragValue::new(&mut self.settings.steps_per_frame).range(1..=256));});
            ui.label(tr!("A velocidade visual depende da complexidade do circuito. Passo e velocidade passam a valer ao iniciar uma nova execução."));
            ui.separator();ui.strong(tr!("Visualização elétrica"));
            ui.checkbox(&mut self.settings.voltage_colors,tr!("Colorir fios pela tensão"));
            ui.checkbox(&mut self.settings.current_dots,tr!("Animar corrente nos componentes de dois pinos"));
            ui.horizontal(|ui|{ui.label(tr!("Escala de cor (±V)"));ui.add(egui::DragValue::new(&mut self.settings.voltage_scale).range(0.001..=1e6));});
            ui.label(tr!("Verde: tensão positiva · vermelho: negativa · cinza: zero. Pontos mostram o sentido da corrente convencional no componente."));
            if ui.button(tr!("Salvar configurações")).clicked() {match self.settings.save() {Ok(())=>self.message(tr!("Configurações salvas para a próxima abertura."),false),Err(e)=>self.message(trf!("Falha ao salvar configurações: {error}", error = e),true)}}
        });
        self.show_settings = open;
        if previous_language != self.settings.language {
            eda_i18n::set_preference(self.settings.language);
        }
        if previous != self.settings.engine {
            self.stop_live();
            if let Some(control) = self.simulation_control.take() {
                control.cancel();
            }
            self.simulation = None;
            self.result = None;
            self.message(
                trf!(
                    "Motor {engine} selecionado. Inicie uma nova análise.",
                    engine = eda_i18n::text(self.settings.engine.label())
                ),
                false,
            );
        }
    }
    pub(super) fn toggle_interactive(&mut self, id: eda_core::ComponentId) {
        if let Some(c) = self.engine.project().components.get(&id)
            && matches!(c.kind, ComponentKind::Switch | ComponentKind::LogicInput)
        {
            let mut parameters = c.parameters.clone();
            parameters.closed = !parameters.closed;
            self.command(Command::SetModel { id, parameters });
            self.choose(Some(ObjectId::Component(id)));
        }
    }
    pub(super) fn model_properties(&mut self, ui: &mut egui::Ui, c: &Component) {
        let p = &mut self.property_parameters;
        if c.kind.is_waveform() {
            ui.strong(tr!("Forma de onda"));
            field(ui, "Frequência (Hz)", &mut p.frequency_hz, 10.0, 1e-6..=1e9);
            field(ui, "Offset (V)", &mut p.offset_v, 0.1, -1e6..=1e6);
            field(ui, "Fase (°)", &mut p.phase_deg, 1.0, -360000.0..=360000.0);
            if c.kind == ComponentKind::SquareVoltage {
                field(
                    ui,
                    "Ciclo ativo (0–1)",
                    &mut p.duty_cycle,
                    0.01,
                    0.0001..=0.9999,
                );
            }
        }
        if matches!(c.kind, ComponentKind::Switch | ComponentKind::LogicInput) {
            ui.checkbox(
                &mut p.closed,
                if c.kind == ComponentKind::Switch {
                    tr!("Chave fechada")
                } else {
                    tr!("Nível alto")
                },
            );
        }
        if c.kind == ComponentKind::Potentiometer {
            ui.add(egui::Slider::new(&mut p.wiper, 0.0..=1.0).text(tr!("Cursor A → B")));
        }
        if c.kind == ComponentKind::Switch {
            field(
                ui,
                "Resistência fechada (Ω)",
                &mut p.on_resistance,
                0.001,
                1e-6..=1e6,
            );
            field(
                ui,
                "Resistência aberta (Ω)",
                &mut p.off_resistance,
                1e6,
                1e6..=1e12,
            );
        }
        if c.kind == ComponentKind::OpAmp {
            field(ui, "Ganho aberto", &mut p.gain, 100.0, 1.0..=1e6);
            field(ui, "Limite inferior (V)", &mut p.rail_low, 0.1, -1e6..=1e6);
            field(ui, "Limite superior (V)", &mut p.rail_high, 0.1, -1e6..=1e6);
        }
        if c.kind.is_gate() {
            field(ui, "Limiar lógico (V)", &mut p.threshold_v, 0.1, -1e6..=1e6);
        }
        if c.kind.is_gate() || c.kind == ComponentKind::OpAmp {
            field(
                ui,
                "Resistência de saída (Ω)",
                &mut p.output_resistance,
                1.0,
                0.001..=1e6,
            );
        }
        if matches!(
            c.kind,
            ComponentKind::Diode
                | ComponentKind::Led
                | ComponentKind::Zener
                | ComponentKind::Npn
                | ComponentKind::Pnp
        ) {
            field(ui, "Fator de idealidade", &mut p.ideality, 0.01, 0.5..=4.0);
            if matches!(
                c.kind,
                ComponentKind::Zener | ComponentKind::Npn | ComponentKind::Pnp
            ) {
                field(ui, "Is (A)", &mut p.saturation_current, 1e-15, 1e-30..=1e-6);
            }
        }
        if c.kind.is_mosfet() {
            field(
                ui,
                "Tensão de limiar |Vt| (V)",
                &mut p.mos_threshold_v,
                0.1,
                0.0..=1000.0,
            );
            field(
                ui,
                "Modulação de canal (1/V)",
                &mut p.channel_modulation,
                0.001,
                0.0..=10.0,
            );
        }
        if c.kind.requires_mixed() || c.kind.is_gate() {
            ui.label(tr!("Motor Misto: atraso genérico de propagação"));
            field(
                ui,
                "Atraso (s)",
                &mut p.propagation_delay_s,
                1e-8,
                1e-12..=1.0,
            );
            let max = if c.kind == ComponentKind::Counter4 {
                15
            } else if c.kind == ComponentKind::Counter10 {
                9
            } else {
                1
            };
            ui.horizontal(|ui| {
                ui.label(tr!("Estado na inicialização"));
                ui.add(egui::DragValue::new(&mut p.initial_state).range(0..=max));
            });
        }
        if c.kind == ComponentKind::Comparator {
            field(ui, "Histerese (V)", &mut p.hysteresis_v, 0.01, 0.0..=1000.0);
        }
        if c.kind.requires_mixed() {
            field(
                ui,
                "Resistência de saída (Ω)",
                &mut p.output_resistance,
                1.0,
                0.001..=1e6,
            );
        }
        if let Some(result) = &self.result
            && result.revision == self.engine.revision()
            && let Some(point) = result.operating_points.iter().find(|p| p.component == c.id)
        {
            ui.separator();
            if let Some(state) = point.digital_state {
                ui.strong(trf!("Estado digital: {state}", state = state));
            }
            for (index, current) in point.pin_currents.iter().enumerate() {
                ui.monospace(trf!(
                    "{pin}: {voltage} V / {current} A",
                    pin = c.kind.pin_name(index as u8),
                    voltage = format!("{:.6}", point.pin_voltages[index]),
                    current = format!("{:.6}", current)
                ));
            }
            ui.strong(tr!("Último estado elétrico"));
            ui.monospace(trf!(
                "U = {voltage} V\nI = {current} A\nP = {power} W",
                voltage = format!("{:.6}", point.voltage),
                current = format!("{:.6}", point.current),
                power = format!("{:.6}", point.power)
            ));
        }
    }
}
fn field(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    speed: f64,
    range: std::ops::RangeInclusive<f64>,
) {
    ui.horizontal(|ui| {
        ui.label(eda_i18n::text(label));
        ui.add(
            egui::DragValue::new(value)
                .speed(speed)
                .range(range)
                .max_decimals(30),
        );
    });
}
pub(super) fn voltage_color(value: f64, scale: f64) -> Color32 {
    let level = (value / scale).abs().min(1.0) as f32;
    let zero = [135.0, 142.0, 149.0];
    let target = if value >= 0.0 {
        [52.0, 218.0, 128.0]
    } else {
        [246.0, 82.0, 98.0]
    };
    Color32::from_rgb(
        (zero[0] + (target[0] - zero[0]) * level) as u8,
        (zero[1] + (target[1] - zero[1]) * level) as u8,
        (zero[2] + (target[2] - zero[2]) * level) as u8,
    )
}
