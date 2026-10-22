use super::*;

impl ProtoCraft {
    pub(super) fn focus_device_library(&mut self) {
        if let Some(id) = self
            .engine
            .project()
            .components
            .values()
            .find(|c| c.device.is_some())
            .map(|c| c.id)
        {
            self.library_mcu = true;
            self.choose(Some(ObjectId::Component(id)));
        }
    }

    pub(super) fn device_library(&mut self, ui: &mut egui::Ui) {
        let catalog = match eda_library::catalog() {
            Ok(c) => c,
            Err(e) => {
                ui.colored_label(egui::Color32::RED, e);
                return;
            }
        };
        ui.label("Pinagem e footprint; sem executar firmware.");
        let rows = catalog.search(&self.component_filter);
        ui.small(format!(
            "{} encontrados · mostrando até 80. Refine a pesquisa.",
            rows.len()
        ));
        for row in rows.iter().take(80) {
            let selected = self.library_device.as_ref() == Some(&row.id)
                && matches!(self.tool, Tool::Place(ComponentKind::Device));
            if ui
                .selectable_label(selected, &row.id)
                .on_hover_text(format!(
                    "{} · {} · {} pinos\n{}\n{}",
                    row.family,
                    row.form,
                    catalog.symbols[&row.symbol].len(),
                    row.footprint,
                    row.description
                ))
                .clicked()
            {
                self.library_device = Some(
                    if catalog
                        .find(self.component_filter.trim())
                        .is_ok_and(|d| d.id == row.id)
                    {
                        self.component_filter.trim().to_owned()
                    } else {
                        row.id.clone()
                    },
                );
                self.tool = Tool::Place(ComponentKind::Device);
                self.wire_start = None;
            }
        }
    }
    pub(super) fn library_component(&self, position: Point) -> Option<Component> {
        let (device, footprint) = eda_library::catalog()
            .ok()?
            .definition(self.library_device.as_deref()?)
            .ok()?;
        Some(Component {
            id: eda_core::ComponentId(0),
            reference: "U?".into(),
            kind: ComponentKind::Device,
            value: 0.0,
            position,
            rotation: self.placement_rotation,
            device: Some(Box::new(device)),
            footprint: Some(Box::new(footprint)),
            imported: None,
            parameters: Default::default(),
        })
    }
    pub(super) fn physical_properties(&mut self, ui: &mut egui::Ui, c: &Component) {
        if let Some(d) = &c.device {
            ui.strong(if d.requested_part.is_empty() {
                &d.id
            } else {
                &d.requested_part
            });
            if d.requested_part != d.id {
                ui.small(format!("Pinagem do catálogo: {}", d.id));
            }
            ui.label(format!("{} · {} · {}", d.manufacturer, d.family, d.form));
            ui.label(&d.description);
            ui.colored_label(
                egui::Color32::from_rgb(193, 131, 38),
                "Suporte físico. Firmware e periféricos não são simulados.",
            );
            if d.datasheet.starts_with("https://") {
                ui.hyperlink_to("Datasheet", &d.datasheet);
            }
            ui.small(format!(
                "Origem: {} · {}",
                d.source.project,
                &d.source.revision[..d.source.revision.len().min(12)]
            ));
        }
        if c.device.is_none()
            && !matches!(c.kind, ComponentKind::Ground | ComponentKind::Connector)
            && let Ok(catalog) = eda_library::catalog()
        {
            let numbers: std::collections::BTreeSet<_> = c
                .pins()
                .iter()
                .map(|p| c.pin_number(p.reference.pin))
                .collect();
            let current = c.footprint.as_ref().map_or("", |f| f.id.as_str());
            let mut choice = current.to_owned();
            egui::ComboBox::from_id_salt("physical_footprint")
                .selected_text(if current.is_empty() {
                    "Associar footprint…"
                } else {
                    current
                })
                .show_ui(ui, |ui| {
                    for f in catalog.footprints.values() {
                        let pads: std::collections::BTreeSet<_> = f
                            .pads
                            .iter()
                            .filter(|p| {
                                p.technology != eda_core::PadTechnology::NonPlated
                                    && !p.number.is_empty()
                            })
                            .map(|p| p.number.clone())
                            .collect();
                        if pads == numbers {
                            ui.selectable_value(&mut choice, f.id.clone(), &f.id);
                        }
                    }
                });
            if choice != current
                && let Some(f) = catalog.footprints.get(&choice)
            {
                self.command(Command::AssignFootprint {
                    id: c.id,
                    footprint: Box::new(f.clone()),
                });
            }
        }
        if let Some(f) = &c.footprint {
            ui.separator();
            ui.strong("Encapsulamento / PCB");
            ui.label(&f.id);
            ui.label(format!(
                "Corpo: {:.3} × {:.3} mm",
                f.body_bounds.width() as f64 / 1e6,
                f.body_bounds.height() as f64 / 1e6
            ));
            ui.small(format!(
                "Contorno: {} · {} pads · {} áreas de exclusão",
                f.body_layer,
                f.pads.len(),
                f.keepouts.len()
            ));
            if ui.button("Abrir PCB e sincronizar").clicked() {
                self.pcb.active = true;
                self.sync_board();
            }
            ui.collapsing("Mapeamento pino / pad", |ui| {
                for pin in c.pins() {
                    ui.monospace(format!(
                        "{} {} → {}",
                        c.pin_number(pin.reference.pin),
                        c.pin_name(pin.reference.pin),
                        c.pads_for_pin(pin.reference.pin).join(", ")
                    ));
                }
            });
        }
    }
}

pub(super) fn draw_device(
    painter: &egui::Painter,
    c: &Component,
    center: Pos2,
    zoom: f32,
    color: Color32,
) {
    let pins = c.pins();
    if pins.is_empty() {
        return;
    }
    let world = |p: Point| {
        center + Vec2::new((p.x - c.position.x) as f32, (p.y - c.position.y) as f32) * zoom
    };
    let min = Pos2::new(
        pins.iter().map(|p| p.position.x).min().unwrap_or(0) as f32,
        pins.iter().map(|p| p.position.y).min().unwrap_or(0) as f32,
    );
    let max = Pos2::new(
        pins.iter().map(|p| p.position.x).max().unwrap_or(0) as f32,
        pins.iter().map(|p| p.position.y).max().unwrap_or(0) as f32,
    );
    let mut bounds = Rect::from_min_max(min, max).shrink(2.0);
    if bounds.width() < 4.0 || bounds.height() < 4.0 {
        bounds = Rect::from_center_size(
            Pos2::new(c.position.x as f32, c.position.y as f32),
            Vec2::splat(6.0),
        );
    }
    let screen =
        |p: Pos2| center + (p - Pos2::new(c.position.x as f32, c.position.y as f32)) * zoom;
    let body = Rect::from_min_max(screen(bounds.min), screen(bounds.max));
    painter.rect_stroke(
        body,
        0.0,
        Stroke::new(1.6_f32, color),
        egui::StrokeKind::Inside,
    );
    let font = (zoom * 0.7).clamp(5.0, 11.0);
    for pin in &pins {
        let pos = world(pin.position);
        let point = Pos2::new(pin.position.x as f32, pin.position.y as f32);
        let end = screen(Pos2::new(
            point.x.clamp(bounds.left(), bounds.right()),
            point.y.clamp(bounds.top(), bounds.bottom()),
        ));
        painter.line_segment([pos, end], Stroke::new(1.0_f32, color));
        painter.circle_filled(pos, 2.0, color);
        let (align, offset) = if point.x < bounds.left() {
            (egui::Align2::LEFT_CENTER, Vec2::new(3.0, 0.0))
        } else if point.x > bounds.right() {
            (egui::Align2::RIGHT_CENTER, Vec2::new(-3.0, 0.0))
        } else if point.y < bounds.top() {
            (egui::Align2::CENTER_TOP, Vec2::new(0.0, 3.0))
        } else {
            (egui::Align2::CENTER_BOTTOM, Vec2::new(0.0, -3.0))
        };
        let name = c
            .pin_name(pin.reference.pin)
            .split('/')
            .next()
            .unwrap_or("?");
        painter.text(
            end + offset,
            align,
            name,
            egui::FontId::monospace(font),
            color,
        );
        painter.text(
            pos + Vec2::new(1.0, -2.0),
            egui::Align2::LEFT_BOTTOM,
            c.pin_number(pin.reference.pin),
            egui::FontId::monospace(font * 0.8),
            Palette::new(painter.ctx()).muted,
        );
    }
    if let Some(d) = &c.device {
        painter.text(
            body.center_top() - Vec2::new(0.0, 30.0),
            egui::Align2::CENTER_BOTTOM,
            format!("{} · {}", c.reference, d.id),
            egui::FontId::monospace(11.0),
            color,
        );
    }
}
