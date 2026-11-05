//! Importador original de registros CircuitJS; nenhum modelo ou runtime externo é executado.
//! O perfil conserva terminais e metadados, mas não presume equivalência de semicondutores.
use crate::{ImportReport, ImportResult};
use eda_core::{
    Component, ComponentId, ComponentKind, ImportedComponent, ImportedPin, ImportedProjectInfo,
    LabelId, ModelParameters, NetLabel, Point, Project, Wire, WireId,
};
use quick_xml::{
    Reader,
    encoding::Decoder,
    events::{BytesStart, Event},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    path::Path,
};

const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_RECORDS: usize = 4096;
const MAX_REPORTED_WARNINGS: usize = 240;
const OPAQUE_XML_RECORDS: &[&str] = &["ccm", "clm", "rlm", "scopedata"];
#[derive(Debug, Clone)]
struct Record {
    tag: String,
    fields: BTreeMap<String, String>,
    args: Vec<String>,
}
fn error(key: &str) -> String {
    eda_i18n::text(key)
}
fn record_warning(key: &str, index: usize, tag: &str) -> String {
    eda_i18n::format(
        key,
        &[("record", index.to_string()), ("type", tag.to_owned())],
    )
}
fn warning(warnings: &mut Vec<String>, suppressed: &mut usize, message: String) {
    if warnings.len() < MAX_REPORTED_WARNINGS {
        warnings.push(message);
    } else {
        *suppressed = suppressed.saturating_add(1);
    }
}
fn is_metadata_record(tag: &str) -> bool {
    matches!(
        tag,
        "dm" | "mm"
            | "tm"
            | "rlm"
            | "clm"
            | "ccm"
            | "!"
            | "\""
            | "&"
            | "34"
            | "32"
            | "38"
            | "."
            | "o"
            | "h"
            | "adj"
            | "test"
            | "switchevent"
            | "scopedata"
            | "%"
            | "?"
            | "B"
    )
}

fn xml_open(
    e: &BytesStart<'_>,
    empty: bool,
    decoder: Decoder,
    stack: &mut Vec<String>,
    root_seen: &mut bool,
    result: &mut Vec<Record>,
) -> Result<(), String> {
    let tag = std::str::from_utf8(e.name().as_ref())
        .map_err(|_| error("falstad.syntax"))?
        .to_owned();
    if stack.is_empty() {
        if tag != "cir" || *root_seen {
            return Err(error("falstad.syntax"));
        }
        *root_seen = true;
    }
    let mut fields = BTreeMap::new();
    for attr in e.attributes() {
        let attr = attr.map_err(|_| error("falstad.syntax"))?;
        let key = std::str::from_utf8(attr.key.as_ref())
            .map_err(|_| error("falstad.syntax"))?
            .to_owned();
        let value = attr
            .decoded_and_normalized_value(quick_xml::XmlVersion::Implicit1_0, decoder)
            .map_err(|_| error("falstad.syntax"))?
            .into_owned();
        if key.len() > 64 || value.len() > 4096 || fields.len() >= 32 {
            return Err(error("falstad.limit"));
        }
        if fields.insert(key, value).is_some() {
            return Err(error("falstad.syntax"));
        }
    }
    if tag == "cir" && fields.get("sublib").is_some_and(|value| value == "1") {
        return Err(error("falstad.library"));
    }
    if tag != "cir" {
        if stack.len() == 1 {
            result.push(Record {
                tag: tag.clone(),
                fields,
                args: vec![],
            });
        } else {
            let record_type = stack.get(1).map(String::as_str);
            let valid_scope_pin = record_type == Some("o") && stack.len() == 2 && tag == "p";
            let valid_opaque_child =
                record_type.is_some_and(|value| OPAQUE_XML_RECORDS.contains(&value));
            if !valid_scope_pin && !valid_opaque_child {
                return Err(error("falstad.syntax"));
            }
        }
    }
    if !empty {
        stack.push(tag);
    }
    if stack.len() > 16 {
        return Err(error("falstad.limit"));
    }
    Ok(())
}

pub fn recognizes(text: &str) -> bool {
    let text = text.trim_start_matches('\u{feff}').trim_start();
    text.starts_with("$ ")
        || text.starts_with("$\t")
        || text.starts_with("<cir ")
        || text.starts_with("<cir>")
        || (text.starts_with("<?xml") && text.contains("<cir"))
}

pub fn import_file(path: impl AsRef<Path>) -> Result<ImportResult, String> {
    let path = path.as_ref();
    let file = File::open(path).map_err(|_| error("falstad.io"))?;
    if file.metadata().map_err(|_| error("falstad.io"))?.len() > MAX_BYTES as u64 {
        return Err(error("falstad.limit"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| error("falstad.io"))?;
    let text = std::str::from_utf8(&bytes).map_err(|_| error("falstad.syntax"))?;
    import_text(
        text,
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("CircuitJS"),
    )
}

fn records(text: &str) -> Result<Vec<Record>, String> {
    let mut result = Vec::new();
    if text.starts_with('<') {
        // Entidades XML predefinidas são decodificadas; DTD, CDATA e inclusões são recusadas.
        let mut reader = Reader::from_str(text);
        let mut stack = Vec::new();
        let mut root_seen = false;
        loop {
            match reader.read_event().map_err(|_| error("falstad.syntax"))? {
                Event::Start(e) => xml_open(
                    &e,
                    false,
                    reader.decoder(),
                    &mut stack,
                    &mut root_seen,
                    &mut result,
                )?,
                Event::Empty(e) => xml_open(
                    &e,
                    true,
                    reader.decoder(),
                    &mut stack,
                    &mut root_seen,
                    &mut result,
                )?,
                Event::End(e) => {
                    let tag = std::str::from_utf8(e.name().as_ref())
                        .map_err(|_| error("falstad.syntax"))?
                        .to_owned();
                    if stack.pop().as_ref() != Some(&tag) {
                        return Err(error("falstad.syntax"));
                    }
                }
                Event::Text(e) => {
                    let bytes: &[u8] = e.as_ref();
                    let inside_opaque_record = stack
                        .get(1)
                        .is_some_and(|tag| OPAQUE_XML_RECORDS.contains(&tag.as_str()));
                    if !inside_opaque_record && !bytes.iter().all(u8::is_ascii_whitespace) {
                        return Err(error("falstad.xml_unsafe"));
                    }
                }
                Event::Decl(_) | Event::Comment(_) => {}
                Event::Eof => {
                    if !root_seen || !stack.is_empty() {
                        return Err(error("falstad.syntax"));
                    }
                    break;
                }
                _ => return Err(error("falstad.xml_unsafe")),
            }
            if result.len() > MAX_RECORDS {
                return Err(error("falstad.limit"));
            }
        }
    } else {
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            if line.len() > 8192 {
                return Err(error("falstad.limit"));
            }
            let tokens: Vec<_> = line.split_whitespace().collect();
            if tokens.len() > 128 {
                return Err(error("falstad.limit"));
            }
            let tag = tokens[0];
            if tag == "$" {
                if tokens.len() < 3 {
                    return Err(error("falstad.syntax"));
                }
                continue;
            }
            result.push(Record {
                tag: tag.into(),
                fields: BTreeMap::new(),
                args: tokens[1..].iter().map(|s| s.to_string()).collect(),
            });
            if result.len() > MAX_RECORDS {
                return Err(error("falstad.limit"));
            }
        }
    }
    if result.is_empty() {
        return Err(error("falstad.syntax"));
    }
    Ok(result)
}

impl Record {
    fn number(&self, key: &str, index: usize, default: Option<f64>) -> Result<f64, String> {
        match self.fields.get(key).or_else(|| self.args.get(index)) {
            Some(s) => {
                if matches!(key, "p" | "mm") && (s == "true" || s == "false") {
                    return Ok(if s == "true" { 1.0 } else { 0.0 });
                }
                let value: f64 = s.parse().map_err(|_| error("falstad.number"))?;
                if !value.is_finite() || value.abs() > 1e12 {
                    return Err(error("falstad.number"));
                }
                Ok(value)
            }
            None => default.ok_or_else(|| error("falstad.syntax")),
        }
    }
    fn coords(&self) -> Result<[Point; 2], String> {
        let tokens: Vec<_> = if let Some(coords) = self.fields.get("x") {
            coords.split_whitespace().collect()
        } else {
            self.args.iter().take(4).map(String::as_str).collect()
        };
        if tokens.len() != 4 {
            return Err(error("falstad.syntax"));
        }
        let mut values = [0; 4];
        for (slot, text) in values.iter_mut().zip(tokens) {
            *slot = text.parse::<i32>().map_err(|_| error("falstad.number"))?;
            if slot.abs_diff(0) > 10_000 {
                return Err(error("falstad.limit"));
            }
        }
        Ok([
            Point::new(values[0], values[1]),
            Point::new(values[2], values[3]),
        ])
    }
    fn flags(&self) -> Result<i32, String> {
        let s = self
            .fields
            .get("f")
            .or_else(|| self.args.get(4))
            .ok_or_else(|| error("falstad.syntax"))?;
        s.parse::<i32>().map_err(|_| error("falstad.number"))
    }
}

fn add_component(
    project: &mut Project,
    kind: ComponentKind,
    value: f64,
    pins: &[Point],
    record: &Record,
    parameters: ModelParameters,
) -> ComponentId {
    let id = ComponentId(project.next_id);
    project.next_id += 1;
    let position = pins[0];
    let mut properties = record.fields.clone();
    if !record.args.is_empty() {
        properties.insert("tokens".into(), record.args.join(" "));
    }
    project.components.insert(
        id,
        Component {
            id,
            reference: format!("{}{}", kind.prefix(), id.0),
            kind,
            value,
            position,
            rotation: 0,
            parameters,
            device: None,
            footprint: None,
            imported: Some(ImportedComponent {
                source_format: "Falstad/CircuitJS".into(),
                device: record.tag.clone(),
                value_text: value.to_string(),
                properties,
                pins: pins
                    .iter()
                    .enumerate()
                    .map(|(index, p)| ImportedPin {
                        number: (index + 1).to_string(),
                        name: format!("P{}", index + 1),
                        offset: Point::new(p.x - position.x, p.y - position.y),
                    })
                    .collect(),
            }),
        },
    );
    id
}

fn ground(project: &mut Project, p: Point) {
    let record = Record {
        tag: "implicit_ground".into(),
        fields: BTreeMap::new(),
        args: vec![],
    };
    add_component(
        project,
        ComponentKind::Ground,
        0.0,
        &[p],
        &record,
        ModelParameters::default(),
    );
}

fn find_free_point(
    project: &Project,
    anchor: Point,
    geometry_work: &mut usize,
) -> Result<Point, String> {
    for step in 1..=256 {
        let distance = step * 16;
        for (dx, dy) in [(distance, 0), (-distance, 0), (0, distance), (0, -distance)] {
            *geometry_work = geometry_work.saturating_add(
                project.components.len() + project.wires.len() + project.labels.len(),
            );
            if *geometry_work > 2_000_000 {
                return Err(error("falstad.limit"));
            }
            let point = Point::new(anchor.x + dx, anchor.y + dy);
            if point.x.abs() > 10_000 || point.y.abs() > 10_000 {
                continue;
            }
            if project.labels.values().any(|label| label.position == point)
                || project
                    .components
                    .values()
                    .flat_map(Component::pins)
                    .any(|pin| pin.position == point)
            {
                continue;
            }
            let on_wire = project.wires.values().any(|wire| {
                (wire.start.x == wire.end.x
                    && point.x == wire.start.x
                    && (wire.start.y.min(wire.end.y)..=wire.start.y.max(wire.end.y))
                        .contains(&point.y))
                    || (wire.start.y == wire.end.y
                        && point.y == wire.start.y
                        && (wire.start.x.min(wire.end.x)..=wire.start.x.max(wire.end.x))
                            .contains(&point.x))
            });
            if !on_wire {
                return Ok(point);
            }
        }
    }
    Err(error("falstad.limit"))
}

pub fn import_text(text: &str, name: &str) -> Result<ImportResult, String> {
    if text.len() > MAX_BYTES || name.len() > 256 {
        return Err(error("falstad.limit"));
    }
    let text = text.trim_start_matches('\u{feff}').trim();
    if !recognizes(text) {
        return Err(error("falstad.syntax"));
    }
    let records = records(text)?;
    let mut project = Project::empty(name);
    let mut warnings = vec![];
    let mut suppressed_warnings = 0usize;
    let mut unsupported = BTreeSet::new();
    let mut rails = vec![];
    let mut geometry_work = 0usize;
    let models: BTreeMap<_, _> = records
        .iter()
        .filter(|r| matches!(r.tag.as_str(), "mm" | "dm" | "tm"))
        .filter_map(|r| {
            r.fields
                .get("nm")
                .map(|name| (name.clone(), r.fields.clone()))
        })
        .collect();
    for (index, record) in records.iter().enumerate() {
        let mut record = record.clone();
        if let Some(model) = record.fields.get("mo").and_then(|name| models.get(name)) {
            for (key, value) in model {
                if key != "nm" {
                    record.fields.insert(format!("model.{key}"), value.clone());
                }
            }
            for key in ["vt", "be"] {
                if let Some(value) = model.get(key) {
                    record.fields.insert(key.into(), value.clone());
                }
            }
        }
        let record = &record;
        let tag = record.tag.as_str();
        // Marcadores de medição e texto não alteram a topologia elétrica.
        // Não os materializamos como dispositivos desconhecidos no solver.
        if matches!(tag, "p" | "O") {
            warning(
                &mut warnings,
                &mut suppressed_warnings,
                record_warning("falstad.instrument", index + 1, tag),
            );
            continue;
        }
        if tag == "x" {
            warning(
                &mut warnings,
                &mut suppressed_warnings,
                record_warning("falstad.annotation", index + 1, tag),
            );
            continue;
        }
        if tag == "ln" {
            let name = record
                .fields
                .get("text")
                .or_else(|| record.fields.get("name"))
                .or_else(|| record.fields.get("lab"))
                .cloned()
                .unwrap_or_else(|| record.args.get(5..).unwrap_or_default().join(" "));
            let name = name.trim();
            if name.is_empty() {
                warning(
                    &mut warnings,
                    &mut suppressed_warnings,
                    record_warning("falstad.annotation", index + 1, tag),
                );
                continue;
            }
            if name.len() > 256 {
                return Err(error("falstad.limit"));
            }
            let position = record.coords()?[0];
            let id = LabelId(project.next_id);
            project.next_id += 1;
            project.labels.insert(
                id,
                NetLabel {
                    id,
                    position,
                    name: name.to_owned(),
                },
            );
            continue;
        }
        if is_metadata_record(tag) {
            if !matches!(tag, "dm" | "mm" | "tm") {
                warning(
                    &mut warnings,
                    &mut suppressed_warnings,
                    eda_i18n::format("falstad.metadata", &[("type", tag.to_owned())]),
                );
            }
            continue;
        }
        let coords = record.coords()?;
        let [a, b] = coords;
        let flags = record.flags()?;
        if tag == "w" {
            if a == b {
                return Err(error("falstad.syntax"));
            }
            if a.x != b.x && a.y != b.y {
                // O domínio só desenha fios ortogonais. Rótulos conservam os dois terminais.
                for p in [a, b] {
                    let id = LabelId(project.next_id);
                    project.next_id += 1;
                    project.labels.insert(
                        id,
                        NetLabel {
                            id,
                            position: p,
                            name: format!("FALSTAD_W{index}"),
                        },
                    );
                }
                warning(
                    &mut warnings,
                    &mut suppressed_warnings,
                    error("falstad.diagonal"),
                );
            } else {
                let id = WireId(project.next_id);
                project.next_id += 1;
                project.wires.insert(
                    id,
                    Wire {
                        id,
                        start: a,
                        end: b,
                    },
                );
            }
            continue;
        }
        if tag == "g" {
            ground(&mut project, a);
            continue;
        }
        let mut pins = vec![a, b];
        let mut params = ModelParameters::default();
        let mut incomplete = false;
        let mut approximated = false;
        let mut unknown_pin_map = false;
        let mut approximation_warning = "falstad.generic_model";
        let mut internal_resistor = None;
        let (kind, value) = match tag {
            "r" => (ComponentKind::Resistor, record.number("r", 5, None)?),
            "c" => {
                let initial_voltage = record.number("iv", 7, Some(0.0))?;
                incomplete = initial_voltage != 0.0;
                (ComponentKind::Capacitor, record.number("c", 5, None)?)
            }
            "l" => {
                let initial_current = record.number("ic", 7, Some(0.0))?;
                incomplete = initial_current != 0.0;
                (ComponentKind::Inductor, record.number("l", 5, None)?)
            }
            "s" => {
                let state = record.number("p", 5, Some(0.0))?;
                if state != 0.0 && state != 1.0 {
                    return Err(error("falstad.number"));
                }
                params.closed = state == 0.0;
                let resistance = record.number("r", 7, Some(0.001))?;
                if resistance < 0.0 {
                    return Err(error("falstad.number"));
                }
                params.on_resistance = resistance.max(1e-6);
                approximated = true;
                (ComponentKind::Switch, 1.0)
            }
            "v" | "R" => {
                let wave = record.number("wf", 5, Some(0.0))?;
                let max = record.number("maxv", 7, Some(5.0))?;
                let bias = record.number("bias", 8, Some(0.0))?;
                let resistance = record.number("ir", usize::MAX, Some(0.0))?;
                if resistance < 0.0 {
                    return Err(error("falstad.number"));
                }
                if tag == "v" {
                    // CircuitJS VoltageElm usa o primeiro ponto como terminal positivo.
                    pins = vec![a, b];
                } else {
                    rails.push((ComponentId(project.next_id), a));
                }
                if resistance > 0.0 {
                    let internal = find_free_point(&project, a, &mut geometry_work)?;
                    pins[0] = internal;
                    internal_resistor = Some((a, internal, resistance));
                }
                match wave as i32 {
                    0 if wave == 0.0 => (ComponentKind::VoltageSource, max + bias),
                    1 | 2 if wave == wave.trunc() => {
                        params.frequency_hz = record.number("fr", 6, Some(40.0))?;
                        params.offset_v = bias;
                        params.phase_deg = record.number("phaseShift", 9, Some(0.0))?.to_degrees();
                        params.duty_cycle = record.number("dutyCycle", 10, Some(0.5))?;
                        if flags & 2 != 0 {
                            params.phase_deg += 90.0;
                        }
                        (
                            if wave == 1.0 {
                                ComponentKind::SineVoltage
                            } else {
                                ComponentKind::SquareVoltage
                            },
                            max,
                        )
                    }
                    _ => {
                        incomplete = true;
                        (ComponentKind::Imported, 1.0)
                    }
                }
            }
            "t" | "f" => {
                let polarity = if tag == "t" {
                    record.number("pn", 5, None)?
                } else if flags & 1 != 0 {
                    -1.0
                } else {
                    1.0
                };
                if a == b || (a.x != b.x && a.y != b.y) {
                    return Err(error("falstad.orientation"));
                }
                let flip_flag = if tag == "t" { 1 } else { 8 };
                let flip = if flags & flip_flag != 0 { -1 } else { 1 };
                let sign = if tag == "t" { polarity as i32 } else { 1 };
                let delta = 16 * flip * sign;
                let upper = if a.y == b.y {
                    Point::new(b.x, b.y - delta)
                } else {
                    Point::new(b.x + delta, b.y)
                };
                let lower = if a.y == b.y {
                    Point::new(b.x, b.y + delta)
                } else {
                    Point::new(b.x - delta, b.y)
                };
                pins = vec![a, upper, lower];
                if tag == "t" {
                    if polarity == 0.0 {
                        // No CircuitJS, pn=0 gera pinos coletor/emissor coincidentes
                        // e somente a fuga mínima de base; preservamos isso como 1 TΩ.
                        pins = vec![a, b];
                        approximated = true;
                        approximation_warning = "falstad.degenerate_transistor";
                        (ComponentKind::Resistor, 1e12)
                    } else if polarity != 1.0 && polarity != -1.0 {
                        incomplete = true;
                        (ComponentKind::Imported, 1.0)
                    } else {
                        params.saturation_current =
                            record.number("model.is", usize::MAX, Some(1e-13))?;
                        params.ideality = record.number("model.nf", usize::MAX, Some(1.0))?;
                        approximated = true;
                        (
                            if polarity == 1.0 {
                                ComponentKind::Npn
                            } else {
                                ComponentKind::Pnp
                            },
                            record.number("be", 8, Some(100.0))?,
                        )
                    }
                } else {
                    // O solver nativo espera gate, drain, source. No CircuitJS,
                    // post 1/2 são source/drain para NMOS e drain/source para PMOS.
                    if polarity > 0.0 {
                        pins.swap(1, 2);
                    }
                    if flags & 64 != 0 {
                        pins.push(b);
                        incomplete = true;
                        (ComponentKind::Imported, 1.0)
                    } else {
                        let threshold = record.number("vt", 5, Some(1.5))?.abs();
                        let beta = record.number("be", 6, Some(0.02))?;
                        if threshold == 0.0 || beta <= 0.0 {
                            incomplete = true;
                            (ComponentKind::Imported, 1.0)
                        } else {
                            params.mos_threshold_v = threshold;
                            params.channel_modulation = record
                                .number("model.la", usize::MAX, Some(0.0))?
                                .clamp(0.0, 10.0);
                            approximated = true;
                            (
                                if polarity > 0.0 {
                                    ComponentKind::Nmos
                                } else {
                                    ComponentKind::Pmos
                                },
                                beta,
                            )
                        }
                    }
                }
            }
            "d" | "z" | "LED" | "162" => {
                let model_name = record.fields.get("mo").map(String::as_str);
                let defaults: (f64, f64, f64, f64) = match model_name {
                    Some("default") => (1.714_352_819_280_888_3e-7, 0.0, 2.0, 0.0),
                    Some("default-zener") => (1.714_352_819_280_888_3e-7, 0.0, 2.0, 5.6),
                    Some("default-led") => (93.2e-12, 0.042, 3.73, 0.0),
                    Some("old-default-led") => (2.234_990_700_667_192_7e-18, 0.0, 2.0, 0.0),
                    Some(_) => (1e-14, 0.0, 1.0, 0.0),
                    None => (1.714_352_819_280_888_3e-7, 0.0, 2.0, 0.0),
                };
                let saturation = record.number("model.is", usize::MAX, Some(defaults.0))?;
                params.diode_series_resistance =
                    record.number("model.rs", usize::MAX, Some(defaults.1))?;
                params.ideality = record.number("model.n", usize::MAX, Some(defaults.2))?;
                let kind = match tag {
                    "z" => ComponentKind::Zener,
                    "LED" | "162" => ComponentKind::Led,
                    _ => ComponentKind::Diode,
                };
                let value = if kind == ComponentKind::Zener {
                    params.saturation_current = saturation;
                    record.number("model.bv", usize::MAX, Some(defaults.3.max(5.6)))?
                } else {
                    saturation
                };
                approximated = true;
                (kind, value)
            }
            "a" => {
                if a == b || (a.x != b.x && a.y != b.y) {
                    incomplete = true;
                    unknown_pin_map = true;
                    (ComponentKind::Imported, 1.0)
                } else {
                    // OpAmpElm tem duas entradas em lados opostos da ponta 1;
                    // a saída fica na ponta 2. Reordenamos para +, -, saída.
                    let size = if flags & 2 != 0 { 1 } else { 2 };
                    let offset = 8 * size;
                    let normal = if a.y == b.y {
                        Point::new(0, if b.x > a.x { -1 } else { 1 })
                    } else {
                        Point::new(if b.y > a.y { 1 } else { -1 }, 0)
                    };
                    let mut negative = Point::new(a.x + normal.x * offset, a.y + normal.y * offset);
                    if flags & 1 != 0 {
                        negative = Point::new(a.x - normal.x * offset, a.y - normal.y * offset);
                    }
                    let positive = Point::new(2 * a.x - negative.x, 2 * a.y - negative.y);
                    pins = vec![positive, negative, b];
                    params.gain = record.number("ga", 10, Some(100_000.0))?;
                    params.rail_high = record.number("ma", 5, Some(15.0))?;
                    params.rail_low = record.number("mi", 6, Some(-15.0))?;
                    // O CircuitJS trata a saída como fonte controlada ideal;
                    // o solver nativo usa uma resistência pequena para estabilizar o modelo.
                    params.output_resistance = 0.001;
                    approximated = true;
                    (ComponentKind::OpAmp, 1.0)
                }
            }
            _ => {
                incomplete = true;
                unknown_pin_map = true;
                (ComponentKind::Imported, 1.0)
            }
        };
        if incomplete {
            unsupported.insert(format!("{}:{}", index + 1, tag));
            let message = if unknown_pin_map {
                eda_i18n::format(
                    "falstad.unknown_pin_map",
                    &[
                        ("type", tag.to_owned()),
                        ("record", (index + 1).to_string()),
                    ],
                )
            } else {
                eda_i18n::format(
                    "falstad.approximation",
                    &[
                        ("type", tag.to_owned()),
                        ("record", (index + 1).to_string()),
                    ],
                )
            };
            warning(&mut warnings, &mut suppressed_warnings, message);
        }
        if approximated {
            warning(
                &mut warnings,
                &mut suppressed_warnings,
                record_warning(approximation_warning, index + 1, tag),
            );
        }
        if !value.is_finite() || (kind.positive_value() && value <= 0.0) || !params.is_valid() {
            return Err(error("falstad.number"));
        }
        add_component(&mut project, kind, value, &pins, record, params);
        if let Some((external, internal, resistance)) = internal_resistor {
            let mut resistor_record = record.clone();
            resistor_record.tag = "source_internal_resistance".into();
            add_component(
                &mut project,
                ComponentKind::Resistor,
                resistance,
                &[external, internal],
                &resistor_record,
                ModelParameters::default(),
            );
        }
    }
    if suppressed_warnings > 0 {
        warnings.push(eda_i18n::format(
            "falstad.warnings_suppressed",
            &[("count", suppressed_warnings.to_string())],
        ));
    }
    // Cada rail recebe referência implícita fora de qualquer fio/pino existente.
    for (id, a) in rails {
        let mut chosen = None;
        for step in 1..=256 {
            geometry_work = geometry_work.saturating_add(
                project.components.len() + project.wires.len() + project.labels.len(),
            );
            if geometry_work > 2_000_000 {
                return Err(error("falstad.limit"));
            }
            let p = Point::new(a.x + step, a.y + 3);
            if project.labels.values().any(|label| label.position == p) {
                continue;
            }
            if project
                .components
                .values()
                .flat_map(Component::pins)
                .any(|pin| pin.position == p)
            {
                continue;
            }
            if project.wires.values().any(|w| {
                (w.start.x == w.end.x
                    && p.x == w.start.x
                    && (w.start.y.min(w.end.y)..=w.start.y.max(w.end.y)).contains(&p.y))
                    || (w.start.y == w.end.y
                        && p.y == w.start.y
                        && (w.start.x.min(w.end.x)..=w.start.x.max(w.end.x)).contains(&p.x))
            }) {
                continue;
            }
            chosen = Some(p);
            break;
        }
        let p = chosen.ok_or_else(|| error("falstad.limit"))?;
        let component = project
            .components
            .get_mut(&id)
            .ok_or_else(|| error("falstad.syntax"))?;
        if let Some(imported) = component.imported.as_mut() {
            imported.pins[1].offset =
                Point::new(p.x - component.position.x, p.y - component.position.y);
        }
        ground(&mut project, p);
    }
    let complete = unsupported.is_empty();
    project.import_info = Some(ImportedProjectInfo {
        source_format: "Falstad/CircuitJS".into(),
        electrical_complete: complete,
        notes: warnings.clone(),
    });
    project.validate().map_err(|e| e.to_string())?;
    Ok(ImportResult {
        report: ImportReport {
            format: "falstad".into(),
            file_version: None,
            components: project.components.len(),
            wires: project.wires.len(),
            junctions: 0,
            labels: project.labels.len(),
            unsupported_components: unsupported.into_iter().collect(),
            warnings,
            electrical_complete: complete,
        },
        project,
    })
}
