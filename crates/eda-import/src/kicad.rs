//! Importação limitada do formato aberto `.kicad_pcb`.
//!
//! O conversor preserva geometria retangular/linear, footprints, pads,
//! roteamento F.Cu/B.Cu, vias passantes e nomes de redes. O modelo do ProtoCraft
//! não contém esquemático, camadas internas nem contornos arbitrários; esses
//! limites são registrados no relatório em vez de serem ocultados.

use crate::{
    interchange::*,
    sexpr::{self, SExpr},
};
use eda_core::{
    Board, BoardSide, Component, ComponentId, ComponentKind, DataSource, Footprint, FootprintLine,
    ImportedComponent, ImportedPin, LabelId, MountingHole, NM_PER_MM, NmBounds, NmPoint, Pad,
    PadShape, PadTechnology, PinRef, Placement, Point, Project, Track, Via,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    path::Path,
};

const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;
const MAX_BOARD_OBJECTS: usize = 20_000;
const MAX_TOTAL_PADS: usize = 100_000;

#[derive(Default)]
struct ReportBuilder {
    losses: Vec<InterchangeIssue>,
    warnings: Vec<String>,
}

impl ReportBuilder {
    fn loss(&mut self, code: &str, message: &'static str) {
        self.losses.push(InterchangeIssue {
            code: code.to_owned(),
            message: eda_i18n::text(message),
        });
    }

    fn loss_fmt(&mut self, code: &str, template: &str, args: &[(&str, String)]) {
        self.losses.push(InterchangeIssue {
            code: code.to_owned(),
            message: eda_i18n::format(template, args),
        });
    }

    fn warn(&mut self, message: &'static str) {
        self.warnings.push(eda_i18n::text(message));
    }

    fn warn_fmt(&mut self, template: &str, args: &[(&str, String)]) {
        self.warnings.push(eda_i18n::format(template, args));
    }
}

#[derive(Clone, Copy)]
struct BoardOutline {
    min: NmPoint,
    max: NmPoint,
    exact_rectangle: bool,
}

#[derive(Clone)]
struct NetInfo {
    name: String,
}

/// Importa um arquivo KiCad PCB com limite de tamanho fixo.
pub fn import_kicad_pcb(path: impl AsRef<Path>) -> Result<InterchangeResult, String> {
    let path = path.as_ref();
    let file = File::open(path).map_err(|error| {
        eda_i18n::format(
            "Não foi possível abrir o arquivo KiCad: {error}",
            &[("error", error.to_string())],
        )
    })?;
    let metadata = file.metadata().map_err(|error| {
        eda_i18n::format(
            "Não foi possível consultar o arquivo KiCad: {error}",
            &[("error", error.to_string())],
        )
    })?;
    if metadata.len() > MAX_FILE_BYTES as u64 {
        return Err(eda_i18n::text("Arquivo KiCad excede o limite de 8 MiB."));
    }
    let mut bytes = Vec::with_capacity(metadata.len().min(MAX_FILE_BYTES as u64) as usize);
    file.take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            eda_i18n::format(
                "Não foi possível ler o arquivo KiCad: {error}",
                &[("error", error.to_string())],
            )
        })?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(eda_i18n::text("Arquivo KiCad excede o limite de 8 MiB."));
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| eda_i18n::text("O arquivo KiCad não está codificado em UTF-8."))?;
    parse_kicad_pcb(
        path.file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("KiCad importado"),
        text,
    )
}

/// Converte um texto `.kicad_pcb` para um projeto com PCB e esquema sintético.
pub fn parse_kicad_pcb(name: &str, text: &str) -> Result<InterchangeResult, String> {
    if text.len() > MAX_FILE_BYTES {
        return Err(eda_i18n::text("Arquivo KiCad excede o limite de 8 MiB."));
    }
    let root = sexpr::parse(text)?;
    if root.head() != Some("kicad_pcb") {
        return Err(eda_i18n::text("O arquivo não possui cabeçalho kicad_pcb."));
    }
    let mut report = ReportBuilder::default();
    let version = child(&root, "version")
        .and_then(|node| atom(node, 1))
        .unwrap_or("desconhecida");
    let mut project = Project::empty(safe_project_name(name));
    project.import_info = Some(eda_core::ImportedProjectInfo {
        source_format: "KiCad PCB".into(),
        electrical_complete: false,
        notes: vec![eda_i18n::format(
            "Importação restrita à placa KiCad (versão {version}); o esquema original e os modelos elétricos não existem no arquivo PCB.",
            &[("version", version.to_owned())],
        )],
    });

    let nets = parse_nets(&root)?;
    let outline = parse_outline(&root, &mut report)?;
    let mut board = Board::default();
    if let Some(outline) = outline {
        let width = outline.max.x.checked_sub(outline.min.x).ok_or_else(|| {
            eda_i18n::text("Dimensão horizontal do contorno KiCad excede o limite.")
        })?;
        let height = outline.max.y.checked_sub(outline.min.y).ok_or_else(|| {
            eda_i18n::text("Dimensão vertical do contorno KiCad excede o limite.")
        })?;
        if !(NM_PER_MM..=eda_core::PHYSICAL_LIMIT).contains(&width)
            || !(NM_PER_MM..=eda_core::PHYSICAL_LIMIT).contains(&height)
        {
            return Err(eda_i18n::text(
                "Contorno KiCad vazio ou fora dos limites físicos suportados.",
            ));
        }
        board.width_nm = width;
        board.height_nm = height;
        if !outline.exact_rectangle {
            report.loss(
                "outline_approximated",
                "O contorno Edge.Cuts não é um retângulo representável; a PCB usa somente sua caixa delimitadora.",
            );
        }
    } else {
        report.loss(
            "outline_missing",
            "O arquivo não contém um contorno Edge.Cuts reconhecível; foi usada uma placa padrão de 100 × 80 mm.",
        );
    }
    if let Some(thickness) = child(&root, "general")
        .and_then(|general| child(general, "thickness"))
        .and_then(|node| atom(node, 1))
        .and_then(parse_mm)
    {
        board.thickness_nm = thickness;
    }
    if let Some(clearance) = child(&root, "setup")
        .and_then(|setup| child(setup, "design_settings"))
        .and_then(|settings| child(settings, "copper_clearance"))
        .and_then(|node| atom(node, 1))
        .and_then(parse_mm)
    {
        board.clearance_nm = clearance.clamp(0, 10 * NM_PER_MM);
    }
    if child(&root, "net_class").is_some() {
        report.loss(
            "net_classes_omitted",
            "As classes e regras de rede do KiCad não são convertidas para as classes nativas da PCB.",
        );
    }
    if child(&root, "layers").is_some_and(|layers| {
        layers
            .children("layer")
            .filter_map(|layer| atom(layer, 1))
            .any(|name| name.starts_with("In") && name.ends_with(".Cu"))
    }) {
        report.loss(
            "inner_copper_stackup_omitted",
            "A pilha de cobre contém camadas internas, ausentes do modelo de placa do ProtoCraft.",
        );
    }
    if child(&root, "setup").is_some() {
        report.loss(
            "board_setup_partially_imported",
            "Somente espessura e folga global de cobre são lidas; as demais regras de fabricação e DRC do setup não são importadas.",
        );
    }

    let mut reference_names = BTreeSet::new();
    let mut pad_anchor_by_net: BTreeMap<i64, PinRef> = BTreeMap::new();
    let net_grid: BTreeMap<_, _> = nets
        .keys()
        .copied()
        .enumerate()
        .map(|(index, id)| schematic_grid(index).map(|point| (id, point)))
        .collect::<Result<_, _>>()?;
    let mut free_pin_index = net_grid.len();
    let mut board_object_count = 0usize;
    let mut total_pads = 0usize;

    let footprints: Vec<_> = root.children("footprint").collect();
    if footprints.len() > MAX_BOARD_OBJECTS {
        return Err(eda_i18n::text(
            "O arquivo excede o limite de footprints suportados.",
        ));
    }
    for (footprint_index, footprint_expr) in footprints.iter().enumerate() {
        total_pads = total_pads.saturating_add(footprint_expr.children("pad").count());
        if total_pads > MAX_TOTAL_PADS {
            return Err(eda_i18n::text(
                "O arquivo excede o limite de 100.000 pads suportados.",
            ));
        }
        let parsed = parse_footprint(
            footprint_expr,
            footprint_index,
            &mut reference_names,
            &mut report,
        )?;
        let Some(mut parsed) = parsed else {
            continue;
        };
        if parsed.imported.pins.is_empty() {
            let mechanical_pads: Vec<_> = parsed
                .footprint
                .pads
                .iter()
                .filter(|pad| pad.technology == PadTechnology::NonPlated && pad.number.is_empty())
                .collect();
            if mechanical_pads.len() == parsed.footprint.pads.len() {
                for pad in mechanical_pads {
                    board_object_count += 1;
                    if board_object_count > MAX_BOARD_OBJECTS {
                        return Err(eda_i18n::text(
                            "O arquivo excede o limite de 20.000 trilhas, vias e furos.",
                        ));
                    }
                    let Some(drill) = pad.drill else { continue };
                    if drill.x != drill.y {
                        report.loss(
                            "oval_mounting_hole_approximated",
                            "Furo mecânico oval foi aproximado pelo maior diâmetro circular.",
                        );
                    }
                    let id = board_object_count as u64;
                    board.holes.insert(
                        id,
                        MountingHole {
                            id,
                            position: subtract_origin(
                                Placement {
                                    position: parsed.position,
                                    rotation: 0,
                                    side: parsed.side,
                                }
                                .transform(pad.position),
                                outline,
                            ),
                            drill_nm: drill.x.max(drill.y),
                        },
                    );
                }
                continue;
            }
            report.loss_fmt(
                "footprint_without_electrical_pads",
                "{reference} não possui pads elétricos numerados e foi omitido do modelo de componentes.",
                &[("reference", parsed.reference.clone())],
            );
            continue;
        }
        if parsed.imported.pins.len() > 64 {
            report.loss_fmt(
                "footprint_pin_limit",
                "{reference} possui mais de 64 números de pino; o componente foi omitido.",
                &[("reference", parsed.reference.clone())],
            );
            continue;
        }

        let id = ComponentId(project.next_id);
        project.next_id = project.next_id.checked_add(1).ok_or_else(|| {
            eda_i18n::text("Os identificadores internos do projeto foram esgotados.")
        })?;
        let pin_index_by_number: BTreeMap<_, _> = parsed
            .imported
            .pins
            .iter()
            .enumerate()
            .map(|(index, pin)| (pin.number.clone(), index as u16))
            .collect();

        for (number, net_id) in &parsed.pad_nets {
            let Some(&pin_index) = pin_index_by_number.get(number) else {
                continue;
            };
            let pin_ref = PinRef {
                component: id,
                pin: pin_index,
            };
            if let Some(net_id) = net_id {
                if nets.contains_key(net_id) {
                    if let Some(previous) = pad_anchor_by_net.get(net_id)
                        && previous.component == id
                        && previous.pin == pin_index
                    {
                        continue;
                    }
                    pad_anchor_by_net.entry(*net_id).or_insert(pin_ref);
                    parsed.imported.pins[pin_index as usize].offset = *net_grid
                        .get(net_id)
                        .ok_or_else(|| eda_i18n::text("Rede sem coordenada sintética."))?;
                } else {
                    report.loss_fmt(
                        "pad_net_missing",
                        "{reference} pad {number} referencia à rede KiCad {net_id} ausente na tabela net.",
                        &[
                            ("reference", parsed.reference.clone()),
                            ("number", number.clone()),
                            ("net_id", net_id.to_string()),
                        ],
                    );
                    let grid = schematic_grid(free_pin_index)?;
                    free_pin_index += 1;
                    parsed.imported.pins[pin_index as usize].offset = grid;
                }
            } else {
                let grid = schematic_grid(free_pin_index)?;
                free_pin_index += 1;
                parsed.imported.pins[pin_index as usize].offset = grid;
            }
        }

        for (number, net_id) in &parsed.pad_nets {
            if let Some(net_name) = net_id
                .and_then(|net_id| nets.get(&net_id))
                .map(|net| net.name.as_str())
            {
                if net_name.len() <= 4096 {
                    parsed
                        .imported
                        .properties
                        .insert(format!("KICAD_NET_PIN_{number}"), net_name.to_owned());
                } else {
                    report.loss(
                        "net_name_too_long_for_metadata",
                        "Nome de rede excede o limite de metadados e foi mantido somente como alias quando possível.",
                    );
                }
            }
        }

        let component = Component {
            id,
            reference: parsed.reference.clone(),
            kind: ComponentKind::Imported,
            value: 0.0,
            position: Point::default(),
            rotation: 0,
            imported: Some(parsed.imported),
            device: None,
            footprint: Some(Box::new(parsed.footprint)),
            parameters: Default::default(),
        };
        project.components.insert(id, component);
        board.placements.insert(
            id,
            Placement {
                position: subtract_origin(parsed.position, outline),
                rotation: 0,
                side: parsed.side,
            },
        );
    }

    let label_capacity = eda_core::MAX_OBJECTS.saturating_sub(project.components.len());
    let mut labels_written = 0usize;
    let mut net_names: BTreeMap<&str, usize> = BTreeMap::new();
    for net in nets.values().filter(|net| !net.name.is_empty()) {
        *net_names.entry(&net.name).or_default() += 1;
    }
    for (net_id, net) in &nets {
        if !net.name.is_empty()
            && pad_anchor_by_net.contains_key(net_id)
            && labels_written < label_capacity
        {
            let position = *net_grid
                .get(net_id)
                .ok_or_else(|| eda_i18n::text("Rede sem coordenada sintética."))?;
            let name_is_valid = net.name.len() <= 64
                && net.name.trim() == net.name
                && !net.name.chars().any(char::is_control)
                && net_names.get(net.name.as_str()) == Some(&1);
            let label_name = if name_is_valid {
                net.name.clone()
            } else {
                report.loss(
                    "net_name_aliased",
                    "Nome de rede KiCad inválido ou duplicado foi convertido para um alias interno; o nome original é mantido nos metadados dos pinos.",
                );
                format!("KICAD_NET_{net_id}")
            };
            let label_id = LabelId(project.next_id);
            project.next_id = project.next_id.checked_add(1).ok_or_else(|| {
                eda_i18n::text("Os identificadores internos do projeto foram esgotados.")
            })?;
            project.labels.insert(
                label_id,
                eda_core::NetLabel {
                    id: label_id,
                    position,
                    name: label_name,
                },
            );
            labels_written += 1;
        }
    }
    let named_nets_used = nets
        .iter()
        .filter(|(id, net)| !net.name.is_empty() && pad_anchor_by_net.contains_key(id))
        .count();
    if labels_written < named_nets_used {
        report.loss(
            "net_labels_omitted_limit",
            "Alguns nomes de rede foram omitidos para respeitar o limite de objetos do projeto.",
        );
    }

    for segment in root.children("segment") {
        board_object_count += 1;
        if board_object_count > MAX_BOARD_OBJECTS {
            return Err(eda_i18n::text(
                "O arquivo excede o limite de 20.000 trilhas, vias e furos.",
            ));
        }
        let layer = child(segment, "layer")
            .and_then(|node| atom(node, 1))
            .unwrap_or("");
        let side = match layer {
            "F.Cu" => BoardSide::Top,
            "B.Cu" => BoardSide::Bottom,
            _ => {
                report.loss_fmt(
                    "inner_layer_track",
                    "Trilha na camada {layer} foi omitida; o modelo suporta F.Cu e B.Cu.",
                    &[("layer", layer.to_owned())],
                );
                continue;
            }
        };
        let Some(net_id) = child(segment, "net")
            .and_then(|node| atom(node, 1))
            .and_then(|value| value.parse::<i64>().ok())
        else {
            report.loss(
                "track_without_net",
                "Trilha sem identificador de rede foi omitida.",
            );
            continue;
        };
        let Some(&net) = pad_anchor_by_net.get(&net_id) else {
            report.loss_fmt(
                "track_net_without_pad",
                "Trilha da rede {net_id} sem pad de referência foi omitida.",
                &[("net_id", net_id.to_string())],
            );
            continue;
        };
        let start = child(segment, "start")
            .and_then(parse_xy)
            .ok_or_else(|| eda_i18n::text("Trilha KiCad sem coordenada inicial válida."))?;
        let end = child(segment, "end")
            .and_then(parse_xy)
            .ok_or_else(|| eda_i18n::text("Trilha KiCad sem coordenada final válida."))?;
        let width = child(segment, "width")
            .and_then(|node| atom(node, 1))
            .and_then(parse_mm)
            .filter(|width| *width > 0)
            .ok_or_else(|| eda_i18n::text("Largura de trilha KiCad inválida."))?;
        let track_id = board_object_count as u64;
        let start = subtract_origin(start, outline);
        let end = subtract_origin(end, outline);
        if start == end {
            report.loss(
                "zero_length_track",
                "Trilha de comprimento zero foi omitida.",
            );
            continue;
        }
        board.tracks.insert(
            track_id,
            Track {
                id: track_id,
                start,
                end,
                width_nm: width,
                layer: side,
                net,
            },
        );
    }
    for via_expr in root.children("via") {
        board_object_count += 1;
        if board_object_count > MAX_BOARD_OBJECTS {
            return Err(eda_i18n::text(
                "O arquivo excede o limite de 20.000 trilhas, vias e furos.",
            ));
        }
        let layers = child(via_expr, "layers")
            .and_then(SExpr::list)
            .unwrap_or(&[])
            .iter()
            .filter_map(SExpr::atom)
            .skip(1)
            .collect::<Vec<_>>();
        if layers.len() != 2 || !layers.contains(&"F.Cu") || !layers.contains(&"B.Cu") {
            report.loss(
                "blind_or_buried_via",
                "Via cega, enterrada ou com camadas não suportadas foi omitida.",
            );
            continue;
        }
        let Some(net_id) = child(via_expr, "net")
            .and_then(|node| atom(node, 1))
            .and_then(|value| value.parse::<i64>().ok())
        else {
            report.loss(
                "via_without_net",
                "Via sem identificador de rede foi omitida.",
            );
            continue;
        };
        let Some(&net) = pad_anchor_by_net.get(&net_id) else {
            report.loss_fmt(
                "via_net_without_pad",
                "Via da rede {net_id} sem pad de referência foi omitida.",
                &[("net_id", net_id.to_string())],
            );
            continue;
        };
        let position = child(via_expr, "at")
            .and_then(parse_xy)
            .ok_or_else(|| eda_i18n::text("Via KiCad sem posição válida."))?;
        let diameter = child(via_expr, "size")
            .and_then(|node| atom(node, 1))
            .and_then(parse_mm)
            .filter(|value| *value > 0)
            .ok_or_else(|| eda_i18n::text("Diâmetro de via KiCad inválido."))?;
        let drill = child(via_expr, "drill")
            .and_then(|node| atom(node, 1))
            .and_then(parse_mm)
            .filter(|value| *value > 0 && *value < diameter)
            .ok_or_else(|| eda_i18n::text("Furo de via KiCad inválido."))?;
        let id = board_object_count as u64;
        board.vias.insert(
            id,
            Via {
                id,
                position: subtract_origin(position, outline),
                diameter_nm: diameter,
                drill_nm: drill,
                net,
            },
        );
    }

    if root.children("zone").next().is_some() {
        report.loss(
            "copper_zone_omitted",
            "Zonas de cobre, ilhas, preenchimento térmico e áreas de exclusão não são representáveis e foram omitidas.",
        );
    }
    report_unsupported_root_items(&root, &mut report);
    if root.children("gr_arc").next().is_some()
        || root.children("gr_poly").next().is_some()
        || root.children("gr_circle").next().is_some()
    {
        report.loss(
            "board_graphics_omitted",
            "Arcos, polígonos ou círculos gráficos do arquivo não são representáveis no modelo de placa.",
        );
    }

    board.corner_radius_nm = 0;
    project.board = Some(Box::new(board));
    project.validate().map_err(|error| {
        eda_i18n::format(
            "A geometria importada não passou pela validação do projeto: {error}",
            &[("error", error.to_string())],
        )
    })?;
    report.warn(
        "O esquemático sintético contém somente pinos importados e nomes de redes da PCB; ele não substitui o esquemático KiCad e não habilita simulação elétrica.",
    );

    Ok(InterchangeResult {
        project,
        report: InterchangeReport {
            format: "KiCad PCB".into(),
            electrical_complete: false,
            geometry_complete: report.losses.is_empty(),
            losses: report.losses,
            warnings: report.warnings,
        },
    })
}

struct ParsedFootprint {
    reference: String,
    position: NmPoint,
    side: BoardSide,
    footprint: Footprint,
    imported: ImportedComponent,
    pad_nets: Vec<(String, Option<i64>)>,
}

fn parse_footprint(
    expr: &SExpr,
    index: usize,
    references: &mut BTreeSet<String>,
    report: &mut ReportBuilder,
) -> Result<Option<ParsedFootprint>, String> {
    let id_text = expr.value(1).unwrap_or("Imported:unknown");
    let footprint_at = child(expr, "at")
        .and_then(parse_xy_angle)
        .ok_or_else(|| eda_i18n::text("Footprint KiCad sem posição válida."))?;
    if footprint_at.angle_mdeg.rem_euclid(90_000) != 0 {
        report.loss_fmt(
            "arbitrary_footprint_rotation_rounded",
            "Rotação não ortogonal de {footprint} foi incorporada à geometria com arredondamento nanométrico.",
            &[("footprint", id_text.to_owned())],
        );
    }
    let layer = child(expr, "layer")
        .and_then(|node| atom(node, 1))
        .ok_or_else(|| eda_i18n::text("Footprint KiCad sem camada de posicionamento."))?;
    let side = match layer {
        "F.Cu" => BoardSide::Top,
        "B.Cu" => BoardSide::Bottom,
        _ => {
            report.loss_fmt(
                "unsupported_footprint_side",
                "Footprint na camada {layer} foi omitido.",
                &[("layer", layer.to_owned())],
            );
            return Ok(None);
        }
    };
    let original_reference = property_value(expr, "Reference")
        .or_else(|| fp_text_value(expr, "reference"))
        .unwrap_or("");
    let reference = sanitize_reference(original_reference, index, references);
    if reference != original_reference {
        report.warn_fmt(
            "Referência KiCad {reference} foi normalizada para {safe_reference}; o nome original foi mantido nos metadados.",
            &[
                ("reference", original_reference.to_owned()),
                ("safe_reference", reference.clone()),
            ],
        );
    }
    let value = property_value(expr, "Value")
        .or_else(|| fp_text_value(expr, "value"))
        .unwrap_or("");
    for property in expr.children("property") {
        if property.children("at").next().is_some()
            || property.children("layer").next().is_some()
            || property.children("effects").next().is_some()
        {
            report.loss_fmt(
                "footprint_property_placement_omitted",
                "Posição, camada ou estilo de texto das propriedades de {reference} não é preservado.",
                &[("reference", reference.clone())],
            );
            break;
        }
    }
    let mut pads = Vec::new();
    let mut pad_nets = Vec::new();
    let mut pins_by_number: BTreeMap<String, ImportedPin> = BTreeMap::new();
    let mut bounds_points = Vec::new();
    let mut lines = Vec::new();
    let mut courtyard_points = Vec::new();
    let mut saw_courtyard = false;
    for pad_expr in expr.children("pad") {
        let number = pad_expr.value(1).unwrap_or("").to_owned();
        let technology_text = pad_expr.value(2).unwrap_or("");
        let shape_text = pad_expr.value(3).unwrap_or("");
        let at = child(pad_expr, "at")
            .and_then(parse_xy_angle)
            .ok_or_else(|| eda_i18n::text("Pad KiCad sem posição válida."))?;
        let size = child(pad_expr, "size")
            .and_then(parse_xy)
            .ok_or_else(|| eda_i18n::text("Pad KiCad sem tamanho válido."))?;
        if size.x <= 0 || size.y <= 0 {
            return Err(eda_i18n::text("Tamanho de pad KiCad inválido."));
        }
        let layers_expr = child(pad_expr, "layers");
        let mut layers = layers_expr
            .and_then(SExpr::list)
            .unwrap_or(&[])
            .iter()
            .skip(1)
            .filter_map(SExpr::atom)
            .map(|layer| {
                if side == BoardSide::Bottom {
                    swap_layer(layer)
                } else {
                    layer.to_owned()
                }
            })
            .collect::<Vec<_>>();
        if layers.is_empty() {
            return Err(eda_i18n::text("Pad KiCad sem camadas."));
        }
        layers.sort();
        layers.dedup();
        let technology = match technology_text {
            "smd" => PadTechnology::Smd,
            "thru_hole" => PadTechnology::ThroughHole,
            "np_thru_hole" => PadTechnology::NonPlated,
            _ => {
                report.loss_fmt(
                    "unsupported_pad_technology",
                    "Tecnologia de pad {technology} convertida para SMD.",
                    &[("technology", technology_text.to_owned())],
                );
                PadTechnology::Smd
            }
        };
        let drill = parse_drill(pad_expr)?;
        if technology != PadTechnology::Smd && drill.is_none() {
            return Err(eda_i18n::format(
                "Pad KiCad furado sem dimensão de drill válida: {number}.",
                &[("number", number.clone())],
            ));
        }
        let mut shape = match shape_text {
            "rect" => PadShape::Rect,
            "roundrect" => PadShape::RoundRect,
            "circle" => PadShape::Circle,
            "oval" => PadShape::Oval,
            "custom" => {
                report.loss_fmt(
                    "custom_pad_shape_approximated",
                    "Pad customizado {number} foi aproximado pelo retângulo delimitador.",
                    &[("number", number.clone())],
                );
                PadShape::Rect
            }
            _ => {
                report.loss_fmt(
                    "unsupported_pad_shape",
                    "Forma de pad {shape} convertida para retângulo.",
                    &[("shape", shape_text.to_owned())],
                );
                PadShape::Rect
            }
        };
        let roundrect_ratio_ppm = child(pad_expr, "roundrect_rratio")
            .and_then(|node| atom(node, 1))
            .and_then(parse_ratio_ppm)
            .unwrap_or(250_000)
            .clamp(0, 500_000) as u32;
        if shape == PadShape::RoundRect && roundrect_ratio_ppm == 0 {
            shape = PadShape::Rect;
        }
        let mask_margin_nm = child(pad_expr, "solder_mask_margin")
            .and_then(|node| atom(node, 1))
            .and_then(parse_mm)
            .unwrap_or(0);
        let paste_margin_nm = child(pad_expr, "solder_paste_margin")
            .and_then(|node| atom(node, 1))
            .and_then(parse_mm)
            .unwrap_or(0);
        let paste_ratio_ppm = child(pad_expr, "solder_paste_margin_ratio")
            .and_then(|node| atom(node, 1))
            .and_then(parse_ratio_ppm)
            .unwrap_or(0);
        let native_pos =
            native_local_position(at.position.x, at.position.y, footprint_at.angle_mdeg, side);
        let native_angle = native_pad_angle(at.angle_mdeg, footprint_at.angle_mdeg, side);
        bounds_for_pad(&mut bounds_points, native_pos, size, native_angle);
        let net_id = child(pad_expr, "net")
            .and_then(|node| atom(node, 1))
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|id| *id > 0);
        if technology != PadTechnology::NonPlated && !number.is_empty() {
            let pin = pins_by_number
                .entry(number.clone())
                .or_insert_with(|| ImportedPin {
                    number: number.clone(),
                    name: number.clone(),
                    offset: Point::default(),
                });
            if let Some(net_id) = net_id {
                if let Some((_, previous)) = pad_nets.iter().find(|(n, _)| n == &number) {
                    if *previous != Some(net_id) {
                        report.loss_fmt(
                            "duplicate_pad_number_nets",
                            "Pads com número {number} em {reference} pertencem a redes diferentes; foi mantida a primeira associação.",
                            &[
                                ("number", number.clone()),
                                ("reference", reference.clone()),
                            ],
                        );
                    }
                } else {
                    pad_nets.push((number.clone(), Some(net_id)));
                }
            } else if !pad_nets.iter().any(|(n, _)| n == &number) {
                pad_nets.push((number.clone(), None));
            }
            let _ = pin;
        } else if technology != PadTechnology::NonPlated && number.is_empty() {
            report.loss(
                "unnumbered_electrical_pad",
                "Pad elétrico sem número não pode ser associado a um pino e sua conectividade foi omitida.",
            );
        }
        pads.push(Pad {
            number,
            position: native_pos,
            size,
            rotation_mdeg: native_angle,
            shape,
            technology,
            drill,
            layers,
            roundrect_ratio_ppm,
            mask_margin_nm,
            paste_margin_nm,
            paste_ratio_ppm: paste_ratio_ppm.clamp(-1_000_000, 1_000_000) as i32,
            polygons: Vec::new(),
        });
    }
    if pads.is_empty() {
        report.loss_fmt(
            "footprint_without_pads",
            "Footprint {footprint} sem pads foi omitido.",
            &[("footprint", id_text.to_owned())],
        );
        return Ok(None);
    }

    for line_expr in expr.children("fp_line") {
        let start = child(line_expr, "start").and_then(parse_xy);
        let end = child(line_expr, "end").and_then(parse_xy);
        let (Some(start), Some(end)) = (start, end) else {
            report.loss(
                "invalid_footprint_line",
                "Linha de footprint inválida foi omitida.",
            );
            continue;
        };
        let native_start = native_local_position(start.x, start.y, footprint_at.angle_mdeg, side);
        let native_end = native_local_position(end.x, end.y, footprint_at.angle_mdeg, side);
        let layer = child(line_expr, "layer")
            .and_then(|node| atom(node, 1))
            .unwrap_or("F.SilkS");
        let layer = if side == BoardSide::Bottom {
            swap_layer(layer)
        } else {
            layer.to_owned()
        };
        let width = child(line_expr, "stroke")
            .and_then(|stroke| child(stroke, "width"))
            .and_then(|node| atom(node, 1))
            .and_then(parse_mm)
            .unwrap_or(150_000);
        bounds_points.push(native_start);
        bounds_points.push(native_end);
        if layer.contains(".CrtYd") {
            saw_courtyard = true;
            courtyard_points.push(native_start);
            courtyard_points.push(native_end);
        }
        lines.push(FootprintLine {
            start: native_start,
            end: native_end,
            width_nm: width,
            layer,
        });
    }
    for child_expr in expr.list().unwrap_or(&[]).iter().skip(1) {
        if matches!(
            child_expr.head(),
            Some("fp_arc" | "fp_circle" | "fp_rect" | "fp_poly" | "fp_curve" | "fp_text")
        ) {
            // Referência e valor são guardados nos metadados, mas seus estados de
            // visibilidade, posição, fonte e desenhos não cabem em FootprintLine.
            if child_expr.head() == Some("fp_text")
                && matches!(child_expr.value(1), Some("reference" | "value"))
            {
                report.loss_fmt(
                    "footprint_text_geometry_omitted",
                    "Texto de referência/valor de {reference} teve estilo e posição omitidos.",
                    &[("reference", reference.clone())],
                );
            } else {
                report.loss_fmt(
                    "footprint_graphic_omitted",
                    "Geometria {shape} de {reference} não é representável.",
                    &[
                        (
                            "shape",
                            child_expr.head().unwrap_or("desconhecida").to_owned(),
                        ),
                        ("reference", reference.clone()),
                    ],
                );
            }
        }
    }
    let known_pad_items = [
        "at",
        "size",
        "layers",
        "drill",
        "net",
        "roundrect_rratio",
        "solder_mask_margin",
        "solder_paste_margin",
        "solder_paste_margin_ratio",
        "options",
        "primitives",
        "remove_unused_layers",
        "keep_end_layers",
        "pinfunction",
        "pintype",
        "die_length",
        "zone_connect",
        "thermal_width",
        "thermal_gap",
    ];
    for pad_expr in expr.children("pad") {
        for child_expr in pad_expr.list().unwrap_or(&[]).iter().skip(1) {
            if let Some(kind) = child_expr.head()
                && !known_pad_items.contains(&kind)
            {
                report.loss_fmt(
                    "unsupported_pad_property",
                    "A propriedade {property} de um pad de {reference} não é preservada.",
                    &[
                        ("property", kind.to_owned()),
                        ("reference", reference.clone()),
                    ],
                );
            }
        }
    }
    let known_footprint_items = [
        "layer",
        "at",
        "property",
        "fp_text",
        "pad",
        "fp_line",
        "fp_arc",
        "fp_circle",
        "fp_rect",
        "fp_poly",
        "fp_curve",
        "attr",
        "uuid",
        "path",
        "tstamp",
        "descr",
        "tags",
        "autoplace_cost90",
        "autoplace_cost180",
        "clearance",
        "solder_mask_margin",
        "solder_paste_margin",
        "solder_paste_margin_ratio",
        "zone_connect",
        "thermal_width",
        "thermal_gap",
    ];
    for child_expr in expr.list().unwrap_or(&[]).iter().skip(1) {
        if let Some(kind) = child_expr.head()
            && !known_footprint_items.contains(&kind)
        {
            report.loss_fmt(
                "unsupported_footprint_property",
                "Propriedade {property} de {reference} não é preservada.",
                &[
                    ("property", kind.to_owned()),
                    ("reference", reference.clone()),
                ],
            );
        }
    }
    if !saw_courtyard {
        report.loss_fmt(
            "courtyard_derived_bounds",
            "{reference}: limite mecânico foi aproximado pela caixa de pads e linhas; não havia courtyard reconhecível.",
            &[("reference", reference.clone())],
        );
    }
    let bounds_source = if saw_courtyard {
        courtyard_points
    } else {
        bounds_points
    };
    let body_bounds = bounding_box(&bounds_source)
        .ok_or_else(|| eda_i18n::text("Não foi possível derivar os limites do footprint KiCad."))?;
    let source = DataSource {
        project: "KiCad PCB import".into(),
        revision: "KiCad 6+ S-expression".into(),
        url: String::new(),
        license: "arquivo fornecido pelo usuário".into(),
    };
    let footprint = Footprint {
        id: id_text.to_owned(),
        description: value.to_owned(),
        body_layer: if side == BoardSide::Bottom {
            "B.Fab".into()
        } else {
            "F.Fab".into()
        },
        body_bounds,
        pads,
        lines,
        keepouts: Vec::new(),
        source,
    };
    footprint.validate().map_err(|error| {
        eda_i18n::format(
            "Footprint KiCad inválido: {error}",
            &[("error", error.to_string())],
        )
    })?;
    let imported = ImportedComponent {
        source_format: "KiCad PCB".into(),
        device: id_text.to_owned(),
        value_text: value.to_owned(),
        properties: BTreeMap::from([
            ("KICAD_REFERENCE".into(), original_reference.to_owned()),
            ("KICAD_VALUE".into(), value.to_owned()),
            ("KICAD_FOOTPRINT".into(), id_text.to_owned()),
        ]),
        pins: pins_by_number.into_values().collect(),
    };
    Ok(Some(ParsedFootprint {
        reference,
        position: footprint_at.position,
        side,
        footprint,
        imported,
        pad_nets,
    }))
}

fn parse_outline(root: &SExpr, report: &mut ReportBuilder) -> Result<Option<BoardOutline>, String> {
    let mut rects = Vec::new();
    let mut lines = Vec::new();
    let mut points = Vec::new();
    let mut unsupported = false;
    for item in root.list().unwrap_or(&[]).iter().skip(1) {
        if item.head() == Some("gr_rect")
            && child(item, "layer").and_then(|n| atom(n, 1)) == Some("Edge.Cuts")
        {
            let start = child(item, "start").and_then(parse_xy);
            let end = child(item, "end").and_then(parse_xy);
            if let (Some(start), Some(end)) = (start, end) {
                rects.push((start, end));
            }
        } else if item.head() == Some("gr_line")
            && child(item, "layer").and_then(|n| atom(n, 1)) == Some("Edge.Cuts")
        {
            let start = child(item, "start").and_then(parse_xy);
            let end = child(item, "end").and_then(parse_xy);
            if let (Some(start), Some(end)) = (start, end) {
                points.extend([start, end]);
                lines.push((start, end));
            }
        } else if matches!(item.head(), Some("gr_arc" | "gr_poly" | "gr_circle"))
            && child(item, "layer").and_then(|n| atom(n, 1)) == Some("Edge.Cuts")
        {
            unsupported = true;
            if let Some(start) = child(item, "start").and_then(parse_xy) {
                points.push(start);
            }
            if let Some(mid) = child(item, "mid").and_then(parse_xy) {
                points.push(mid);
            }
            if let Some(end) = child(item, "end").and_then(parse_xy) {
                points.push(end);
            }
            if let Some(pts) = child(item, "pts") {
                for xy in pts.children("xy") {
                    if let Some(point) = parse_xy(xy) {
                        points.push(point);
                    }
                }
            }
        }
    }
    if rects.len() == 1 && lines.is_empty() && !unsupported {
        let (a, b) = rects[0];
        return Ok(Some(BoardOutline {
            min: NmPoint::new(a.x.min(b.x), a.y.min(b.y)),
            max: NmPoint::new(a.x.max(b.x), a.y.max(b.y)),
            exact_rectangle: true,
        }));
    }
    if rects.len() > 1 {
        report.loss(
            "multiple_edge_rectangles",
            "Múltiplos retângulos Edge.Cuts foram reduzidos a uma caixa delimitadora.",
        );
    }
    for (a, b) in &rects {
        points.extend([*a, *b]);
    }
    if lines.len() == 4 && rects.is_empty() && !unsupported {
        let Some(bounds) = bounding_box(&points) else {
            return Ok(None);
        };
        let (min, max) = (bounds.min, bounds.max);
        let corners = BTreeSet::from([
            (min.x, min.y),
            (max.x, min.y),
            (max.x, max.y),
            (min.x, max.y),
        ]);
        let mut degree: BTreeMap<(i64, i64), usize> = BTreeMap::new();
        let mut all_axis = true;
        for (a, b) in lines {
            all_axis &= a.x == b.x || a.y == b.y;
            *degree.entry((a.x, a.y)).or_default() += 1;
            *degree.entry((b.x, b.y)).or_default() += 1;
        }
        let exact = all_axis
            && degree.len() == 4
            && degree.values().all(|degree| *degree == 2)
            && degree.keys().all(|point| corners.contains(point));
        return Ok(Some(BoardOutline {
            min,
            max,
            exact_rectangle: exact,
        }));
    }
    if points.is_empty() {
        return Ok(None);
    }
    let Some(bounds) = bounding_box(&points) else {
        return Ok(None);
    };
    Ok(Some(BoardOutline {
        min: bounds.min,
        max: bounds.max,
        exact_rectangle: false,
    }))
}

fn parse_nets(root: &SExpr) -> Result<BTreeMap<i64, NetInfo>, String> {
    let mut nets = BTreeMap::new();
    for net in root.children("net") {
        let Some(id) = atom(net, 1).and_then(|value| value.parse::<i64>().ok()) else {
            return Err(eda_i18n::text("Identificador de rede KiCad inválido."));
        };
        if id < 0 {
            return Err(eda_i18n::text("Identificador de rede KiCad negativo."));
        }
        let name = atom(net, 2).unwrap_or("").to_owned();
        if nets.insert(id, NetInfo { name }).is_some() {
            return Err(eda_i18n::text("Identificador de rede KiCad duplicado."));
        }
    }
    Ok(nets)
}

fn report_unsupported_root_items(root: &SExpr, report: &mut ReportBuilder) {
    let known = [
        "version",
        "generator",
        "general",
        "paper",
        "layers",
        "setup",
        "net",
        "footprint",
        "segment",
        "via",
        "zone",
        "gr_rect",
        "gr_line",
        "gr_arc",
        "gr_poly",
        "gr_circle",
        "gr_text",
        "dimension",
        "target",
        "arc",
        "group",
        "rule_area",
        "embedded_fonts",
        "title_block",
        "property",
        "uuid",
        "bitmap",
        "image",
        "net_class",
    ];
    let mut reported = BTreeSet::new();
    for item in root.list().unwrap_or(&[]).iter().skip(1) {
        let Some(kind) = item.head() else { continue };
        let non_edge_graphic = matches!(kind, "gr_line" | "gr_rect")
            && child(item, "layer").and_then(|node| atom(node, 1)) != Some("Edge.Cuts");
        let unsupported = non_edge_graphic
            || matches!(
                kind,
                "gr_text"
                    | "dimension"
                    | "target"
                    | "arc"
                    | "group"
                    | "rule_area"
                    | "bitmap"
                    | "image"
            );
        if unsupported {
            if reported.insert(kind.to_owned()) {
                report.loss_fmt(
                    "board_feature_omitted",
                    "Objetos de placa {kind} não são preservados pelo modelo atual.",
                    &[("kind", kind.to_owned())],
                );
            }
        } else if !known.contains(&kind) && reported.insert(kind.to_owned()) {
            report.loss_fmt(
                "unknown_board_feature",
                "Recurso KiCad desconhecido {kind} não foi importado.",
                &[("kind", kind.to_owned())],
            );
        }
    }
}

fn parse_drill(pad: &SExpr) -> Result<Option<NmPoint>, String> {
    let Some(drill) = child(pad, "drill") else {
        return Ok(None);
    };
    let values = drill.list().unwrap_or(&[]);
    let oval = values.get(1).and_then(SExpr::atom) == Some("oval");
    let x_index = if oval { 2 } else { 1 };
    let x = values
        .get(x_index)
        .and_then(SExpr::atom)
        .and_then(parse_mm)
        .ok_or_else(|| eda_i18n::text("Drill de pad KiCad inválido."))?;
    let y = if oval {
        values
            .get(x_index + 1)
            .and_then(SExpr::atom)
            .and_then(parse_mm)
            .ok_or_else(|| eda_i18n::text("Drill oval de pad KiCad inválido."))?
    } else {
        x
    };
    if values.iter().any(|node| node.head() == Some("offset")) {
        return Err(eda_i18n::text(
            "Drill deslocado não é suportado na importação KiCad.",
        ));
    }
    Ok(Some(NmPoint::new(x, y)))
}

fn parse_xy(expr: &SExpr) -> Option<NmPoint> {
    Some(NmPoint::new(
        parse_mm(atom(expr, 1)?)?,
        parse_mm(atom(expr, 2)?)?,
    ))
}

struct AtValue {
    position: NmPoint,
    angle_mdeg: i32,
}

fn parse_xy_angle(expr: &SExpr) -> Option<AtValue> {
    Some(AtValue {
        position: parse_xy(expr)?,
        angle_mdeg: parse_angle_mdeg(atom(expr, 3).unwrap_or("0"))?,
    })
}

fn parse_angle_mdeg(value: &str) -> Option<i32> {
    let microdegrees = parse_fixed(value, 1_000_000)?;
    let rounded_millidegrees = if microdegrees >= 0 {
        microdegrees.checked_add(500)? / 1_000
    } else {
        microdegrees.checked_sub(500)? / 1_000
    };
    i32::try_from(rounded_millidegrees).ok()
}

fn parse_mm(value: &str) -> Option<i64> {
    parse_fixed(value, NM_PER_MM)
        .filter(|value| value.abs_diff(0) <= eda_core::PHYSICAL_LIMIT as u64)
}

fn parse_fixed(value: &str, scale: i64) -> Option<i64> {
    let value = value.trim();
    if value.is_empty() || value.len() > 32 {
        return None;
    }
    let (negative, value) = if let Some(rest) = value.strip_prefix('-') {
        (true, rest)
    } else if let Some(rest) = value.strip_prefix('+') {
        (false, rest)
    } else {
        (false, value)
    };
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if (whole.is_empty() && fraction.is_empty())
        || (!whole.is_empty() && !whole.bytes().all(|byte| byte.is_ascii_digit()))
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let digits = scale.ilog10() as usize;
    if fraction.len() > digits {
        return None;
    }
    let whole = if whole.is_empty() {
        0
    } else {
        whole.parse::<i64>().ok()?.checked_mul(scale)?
    };
    let mut fraction_value = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<i64>().ok()?
    };
    for _ in fraction.len()..digits {
        fraction_value = fraction_value.checked_mul(10)?;
    }
    let result = whole.checked_add(fraction_value)?;
    if negative {
        result.checked_neg()
    } else {
        Some(result)
    }
}

fn parse_ratio_ppm(value: &str) -> Option<i64> {
    parse_fixed(value, 1_000_000)
}

fn child<'a>(expr: &'a SExpr, name: &str) -> Option<&'a SExpr> {
    expr.child(name)
}

fn atom(expr: &SExpr, index: usize) -> Option<&str> {
    expr.value(index)
}

fn property_value<'a>(expr: &'a SExpr, name: &str) -> Option<&'a str> {
    expr.children("property")
        .find(|property| property.value(1) == Some(name))
        .and_then(|property| property.value(2))
}

fn fp_text_value<'a>(expr: &'a SExpr, kind: &str) -> Option<&'a str> {
    expr.children("fp_text")
        .find(|node| node.value(1) == Some(kind))
        .and_then(|node| node.value(2))
}

fn safe_project_name(name: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        "KiCad importado".into()
    } else {
        name.chars().take(256).collect()
    }
}

fn sanitize_reference(raw: &str, index: usize, used: &mut BTreeSet<String>) -> String {
    let normalized = raw
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .take(48)
        .collect::<String>();
    let base = if normalized.is_empty() {
        format!("KICAD{}", index + 1)
    } else {
        normalized
    };
    let mut reference = base.clone();
    let mut suffix = 1usize;
    while !used.insert(reference.to_ascii_uppercase()) {
        reference = format!("{base}_{suffix}");
        suffix += 1;
    }
    reference
}

fn schematic_grid(index: usize) -> Result<Point, String> {
    let x = i32::try_from((index % 1000) * 1000)
        .map_err(|_| eda_i18n::text("Número de pinos excede o espaço do esquemático sintético."))?;
    let y = i32::try_from((index / 1000) * 1000)
        .map_err(|_| eda_i18n::text("Número de pinos excede o espaço do esquemático sintético."))?;
    let point = Point::new(x, y);
    if !point.is_valid() {
        return Err(eda_i18n::text(
            "Número de pinos excede o espaço do esquemático sintético.",
        ));
    }
    Ok(point)
}

fn subtract_origin(point: NmPoint, outline: Option<BoardOutline>) -> NmPoint {
    let origin = outline.map_or(NmPoint::default(), |outline| outline.min);
    NmPoint::new(point.x - origin.x, point.y - origin.y)
}

fn native_local_position(x: i64, y: i64, footprint_angle_mdeg: i32, side: BoardSide) -> NmPoint {
    let angle = (footprint_angle_mdeg as f64 / 1000.0).to_radians();
    let (sin, cos) = angle.sin_cos();
    match side {
        BoardSide::Top => NmPoint::new(
            round_nm(x as f64 * cos - y as f64 * sin),
            round_nm(x as f64 * sin + y as f64 * cos),
        ),
        // Placement espelha X no verso; desfazemos esse espelho após incorporar a rotação KiCad.
        BoardSide::Bottom => NmPoint::new(
            round_nm(-(x as f64 * cos + y as f64 * sin)),
            round_nm(-x as f64 * sin + y as f64 * cos),
        ),
    }
}

fn native_pad_angle(pad_angle_mdeg: i32, footprint_angle_mdeg: i32, side: BoardSide) -> i32 {
    match side {
        BoardSide::Top => pad_angle_mdeg.saturating_add(footprint_angle_mdeg),
        BoardSide::Bottom => pad_angle_mdeg.saturating_sub(footprint_angle_mdeg),
    }
}

fn round_nm(value: f64) -> i64 {
    if !value.is_finite() {
        return 0;
    }
    value.round().clamp(
        -(eda_core::PHYSICAL_LIMIT as f64),
        eda_core::PHYSICAL_LIMIT as f64,
    ) as i64
}

fn bounds_for_pad(bounds: &mut Vec<NmPoint>, center: NmPoint, size: NmPoint, angle_mdeg: i32) {
    let angle = (angle_mdeg as f64 / 1000.0).to_radians();
    let (sin, cos) = angle.sin_cos();
    let hx = (size.x as f64 * cos.abs() + size.y as f64 * sin.abs()) / 2.0;
    let hy = (size.x as f64 * sin.abs() + size.y as f64 * cos.abs()) / 2.0;
    bounds.extend([
        NmPoint::new(
            round_nm(center.x as f64 - hx),
            round_nm(center.y as f64 - hy),
        ),
        NmPoint::new(
            round_nm(center.x as f64 + hx),
            round_nm(center.y as f64 + hy),
        ),
    ]);
}

fn bounding_box(points: &[NmPoint]) -> Option<NmBounds> {
    let first = *points.first()?;
    let mut min = first;
    let mut max = first;
    for point in points.iter().skip(1) {
        min.x = min.x.min(point.x);
        min.y = min.y.min(point.y);
        max.x = max.x.max(point.x);
        max.y = max.y.max(point.y);
    }
    if min.x == max.x {
        max.x = max.x.saturating_add(1);
    }
    if min.y == max.y {
        max.y = max.y.saturating_add(1);
    }
    Some(NmBounds { min, max })
}

fn swap_layer(layer: &str) -> String {
    if let Some(suffix) = layer.strip_prefix("F.") {
        format!("B.{suffix}")
    } else if let Some(suffix) = layer.strip_prefix("B.") {
        format!("F.{suffix}")
    } else {
        layer.to_owned()
    }
}
