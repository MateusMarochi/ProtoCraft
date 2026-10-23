use super::*;

#[derive(Debug, Default)]
pub(super) struct DeviceLibraryFilters {
    manufacturer: Option<String>,
    family: Option<String>,
    form: Option<String>,
    feature: Option<String>,
    pin_range: usize,
    size_range: usize,
}

impl DeviceLibraryFilters {
    fn to_search_filters(&self) -> eda_library::DeviceSearchFilters {
        let (min_pins, max_pins) = match self.pin_range {
            1 => (Some(1), Some(16)),
            2 => (Some(17), Some(32)),
            3 => (Some(33), Some(64)),
            4 => (Some(65), Some(100)),
            5 => (Some(101), None),
            _ => (None, None),
        };
        let max_body_dimension_nm = match self.size_range {
            1 => Some(3_000_000),
            2 => Some(5_000_000),
            3 => Some(10_000_000),
            4 => Some(20_000_000),
            _ => None,
        };
        eda_library::DeviceSearchFilters {
            manufacturer: self.manufacturer.clone(),
            family: self.family.clone(),
            form: self.form.clone(),
            feature: self.feature.clone(),
            min_pins,
            max_pins,
            max_body_dimension_nm,
        }
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
}

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
                ui.colored_label(egui::Color32::RED, eda_i18n::text(&e));
                return;
            }
        };
        ui.label(tr!("Filtre por código, função ou características físicas."));
        ui.horizontal_wrapped(|ui| {
            facet_combo(
                ui,
                "device_manufacturer",
                "Fabricante",
                &mut self.device_library_filters.manufacturer,
                catalog.manufacturers(),
                |value| value.to_owned(),
            );
            facet_combo(
                ui,
                "device_family",
                "Família",
                &mut self.device_library_filters.family,
                catalog.families(),
                |value| value.to_owned(),
            );
            facet_combo(
                ui,
                "device_form",
                "Formato",
                &mut self.device_library_filters.form,
                catalog.forms(),
                form_label,
            );
        });
        ui.horizontal_wrapped(|ui| {
            pin_range_combo(ui, &mut self.device_library_filters.pin_range);
            size_range_combo(ui, &mut self.device_library_filters.size_range);
        });
        ui.horizontal_wrapped(|ui| {
            feature_combo(ui, &mut self.device_library_filters.feature);
            if !self.device_library_filters.to_search_filters().is_empty()
                && ui.small_button(tr!("Limpar filtros")).clicked()
            {
                self.device_library_filters.clear();
            }
        });

        let search_filters = self.device_library_filters.to_search_filters();
        let search_key = format!("{}\u{1f}{:?}", self.library_search, search_filters);
        if self.device_search_key.as_deref() != Some(&search_key) {
            self.device_search_results =
                catalog.search_faceted(&self.library_search, &search_filters);
            self.device_search_key = Some(search_key);
            self.device_results_limit = 80;
        }
        let count = self.device_search_results.len();
        if count == 0 {
            ui.add_space(8.0);
            ui.weak(tr!(
                "Nenhum dispositivo encontrado. Remova algum filtro ou tente outro termo."
            ));
        } else {
            ui.small(trf!(
                "{count} resultados · prioridade para código e família exatos",
                count = count
            ));
        }
        let visible = count.min(self.device_results_limit);
        let mut chosen = None;
        for hit in self.device_search_results.iter().take(visible) {
            let row = hit.record;
            let footprint_description = catalog
                .footprints
                .get(&row.footprint)
                .map_or("", |footprint| footprint.description.as_str());
            let details = [row.description.as_str(), footprint_description]
                .into_iter()
                .filter(|value| !value.trim().is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            let short_details: String = details.chars().take(125).collect();
            let selected = self.library_device.as_deref() == Some(row.id.as_str())
                && matches!(self.tool, Tool::Place(ComponentKind::Device));
            let response = ui
                .selectable_label(selected, egui::RichText::new(&row.id).monospace().strong())
                .on_hover_text(trf!(
                    "Família: {family}\nDetalhes: {details}",
                    family = eda_i18n::text(&row.family),
                    details = eda_i18n::text(&details)
                ));
            ui.small(trf!(
                "{manufacturer} · {form} · {pins} pinos",
                manufacturer = row.manufacturer,
                form = form_label(&row.form),
                pins = hit.pin_count
            ));
            ui.small(trf!(
                "{footprint} · {width} × {height} mm",
                footprint = row.footprint,
                width = format!("{:.2}", hit.width_nm as f64 / 1e6),
                height = format!("{:.2}", hit.height_nm as f64 / 1e6)
            ));
            if !short_details.is_empty() {
                ui.small(eda_i18n::text(&short_details));
            }
            if response.clicked() {
                chosen = Some(row.id.clone());
            }
            ui.add_space(3.0);
        }
        if count > visible
            && ui
                .button(trf!(
                    "Carregar mais · {count} restantes",
                    count = count - visible
                ))
                .clicked()
        {
            self.device_results_limit = self.device_results_limit.saturating_add(80);
        }
        if let Some(id) = chosen {
            self.library_device = Some(id);
            self.tool = Tool::Place(ComponentKind::Device);
            self.wire_start = None;
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
                ui.small(trf!("Pinagem do catálogo: {id}", id = d.id));
            }
            ui.label(trf!(
                "{manufacturer} · {family} · {form}",
                manufacturer = &d.manufacturer,
                family = &d.family,
                form = form_label(&d.form)
            ));
            ui.label(eda_i18n::text(&d.description));
            ui.colored_label(
                egui::Color32::from_rgb(193, 131, 38),
                tr!("Suporte físico. Firmware e periféricos não são simulados."),
            );
            if d.datasheet.starts_with("https://") {
                ui.hyperlink_to(tr!("Datasheet"), &d.datasheet);
            }
            ui.small(trf!(
                "Origem: {project} · {revision}",
                project = &d.source.project,
                revision = &d.source.revision[..d.source.revision.len().min(12)]
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
                    tr!("Associar footprint…")
                } else {
                    current.to_owned()
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
            ui.strong(tr!("Encapsulamento / PCB"));
            ui.label(&f.id);
            ui.label(trf!(
                "Corpo: {width} × {height} mm",
                width = format!("{:.3}", f.body_bounds.width() as f64 / 1e6),
                height = format!("{:.3}", f.body_bounds.height() as f64 / 1e6)
            ));
            ui.small(trf!(
                "Contorno: {layer} · {pads} pads · {keepouts} áreas de exclusão",
                layer = f.body_layer,
                pads = f.pads.len(),
                keepouts = f.keepouts.len()
            ));
            if ui.button(tr!("Abrir PCB e sincronizar")).clicked() {
                self.pcb.active = true;
                self.sync_board();
            }
            ui.collapsing(tr!("Mapeamento pino / pad"), |ui| {
                for pin in c.pins() {
                    ui.monospace(trf!(
                        "{number} {name} → {pads}",
                        number = c.pin_number(pin.reference.pin),
                        name = c.pin_name(pin.reference.pin),
                        pads = c.pads_for_pin(pin.reference.pin).join(", ")
                    ));
                }
            });
        }
    }
}

fn facet_combo(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    selected: &mut Option<String>,
    values: &[String],
    format_value: impl Fn(&str) -> String,
) {
    let label = eda_i18n::text(label);
    let selected_text = selected
        .as_deref()
        .map(|value| eda_i18n::text(&format_value(value)))
        .unwrap_or_else(|| trf!("{label}: todos", label = label));
    egui::ComboBox::from_id_salt(id)
        .selected_text(selected_text)
        .show_ui(ui, |ui| {
            let all_label = match id {
                "device_manufacturer" => "Todas as marcas",
                "device_family" => "Todas as famílias",
                "device_form" => "Todos os formatos",
                _ => "Todos",
            };
            ui.selectable_value(selected, None, eda_i18n::text(all_label));
            for value in values {
                ui.selectable_value(
                    selected,
                    Some(value.clone()),
                    eda_i18n::text(&format_value(value)),
                );
            }
        });
}

fn form_label(value: &str) -> String {
    match value {
        "Chip" => eda_i18n::text("Chip"),
        "Module" => eda_i18n::text("Módulo"),
        "Board" => eda_i18n::text("Placa"),
        _ => value.into(),
    }
}

fn pin_range_combo(ui: &mut egui::Ui, selected: &mut usize) {
    const LABELS: [&str; 6] = [
        "Pinos: todos",
        "1–16 pinos",
        "17–32 pinos",
        "33–64 pinos",
        "65–100 pinos",
        "101+ pinos",
    ];
    egui::ComboBox::from_id_salt("device_pin_range")
        .selected_text(eda_i18n::text(
            LABELS.get(*selected).copied().unwrap_or(LABELS[0]),
        ))
        .show_ui(ui, |ui| {
            for (index, label) in LABELS.iter().enumerate() {
                ui.selectable_value(selected, index, eda_i18n::text(label));
            }
        });
}

fn size_range_combo(ui: &mut egui::Ui, selected: &mut usize) {
    const LABELS: [&str; 5] = [
        "Corpo: qualquer tamanho",
        "Até 3 mm",
        "Até 5 mm",
        "Até 10 mm",
        "Até 20 mm",
    ];
    egui::ComboBox::from_id_salt("device_body_size")
        .selected_text(eda_i18n::text(
            LABELS.get(*selected).copied().unwrap_or(LABELS[0]),
        ))
        .show_ui(ui, |ui| {
            for (index, label) in LABELS.iter().enumerate() {
                ui.selectable_value(selected, index, eda_i18n::text(label));
            }
        });
}

fn feature_combo(ui: &mut egui::Ui, selected: &mut Option<String>) {
    const FEATURES: [(&str, &str); 4] = [
        ("wifi", "Wi-Fi"),
        ("bluetooth", "Bluetooth"),
        ("low power", "Baixo consumo"),
        ("ble", "Bluetooth Low Energy"),
    ];
    egui::ComboBox::from_id_salt("device_feature")
        .selected_text(
            selected
                .as_deref()
                .and_then(|value| FEATURES.iter().find(|(key, _)| *key == value))
                .map_or_else(
                    || eda_i18n::text("Função: qualquer"),
                    |(_, label)| eda_i18n::text(label),
                ),
        )
        .show_ui(ui, |ui| {
            ui.selectable_value(selected, None, tr!("Qualquer função"));
            for (key, label) in FEATURES {
                ui.selectable_value(selected, Some(key.to_owned()), eda_i18n::text(label));
            }
        });
    ui.small(tr!(
        "Funções consultam as descrições do dispositivo e do footprint"
    ));
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
