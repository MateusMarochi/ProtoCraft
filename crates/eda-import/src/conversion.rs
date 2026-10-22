use crate::{
    ImportError, ImportReport, ImportResult,
    cdb::{CdbDocument, CdbInstance},
    container,
    dsn::{self, DsnDocument, DsnPin, DsnPoint},
};
use eda_core::{
    Component, ComponentId, ComponentKind, ImportedComponent, ImportedPin, ImportedProjectInfo,
    Junction, JunctionId, LabelId, NetLabel, Point, Project, Wire, WireId,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::Path,
};

pub fn import_proteus(path: impl AsRef<Path>) -> Result<ImportResult, ImportError> {
    let path = path.as_ref();
    let extension = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mut name = path
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or("Projeto importado")
        .to_owned();
    if name == "projeto_original"
        && let Some(parent) = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|v| v.to_str())
    {
        name = parent.to_owned();
    }
    let (bytes, cdb_bytes, file_version, ignored) = match extension.as_str() {
        "pdsprj" | "pdsbak" => {
            let archive = container::read_container(path)?;
            (
                archive
                    .dsn
                    .ok_or_else(|| ImportError::Conversion("ROOT.DSN ausente".into()))?,
                archive.cdb,
                archive.metadata.file_version,
                archive.ignored_entries,
            )
        }
        "dsn" => {
            let bytes = read_limited(path)?;
            let companion = path.with_extension("CDB");
            let cdb = if companion.exists() {
                Some(read_limited(&companion)?)
            } else {
                None
            };
            (bytes, cdb, None, Vec::new())
        }
        _ => {
            return Err(ImportError::Unsupported(
                "selecione um arquivo .pdsprj, .pdsbak ou .dsn".into(),
            ));
        }
    };
    let document = dsn::parse_dsn(&bytes)?;
    let catalog = cdb_bytes
        .as_deref()
        .map(crate::cdb::parse_cdb)
        .transpose()?;
    let mut result = convert_document(&name, &document, catalog.as_ref(), file_version)?;
    if !ignored.is_empty() {
        result.report.warnings.push(format!(
            "Membros mantidos somente no original e não executados/importados: {}.",
            ignored.join(", ")
        ));
    }
    if catalog.is_none() {
        result.report.warnings.push(
            "Catálogo CDB ausente: referências e pinos foram recuperados somente do DSN.".into(),
        );
    }
    sync_notes(&mut result);
    Ok(result)
}

fn read_limited(path: &Path) -> Result<Vec<u8>, ImportError> {
    let file = fs::File::open(path)?;
    if file.metadata()?.len() > 4 * 1024 * 1024 {
        return Err(ImportError::Conversion("DSN/CDB maior que 4 MiB".into()));
    }
    let mut bytes = Vec::new();
    file.take(4 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err(ImportError::Conversion("DSN/CDB maior que 4 MiB".into()));
    }
    Ok(bytes)
}

fn gcd(mut a: i64, mut b: i64) -> i64 {
    while b != 0 {
        let next = a % b;
        a = b;
        b = next;
    }
    a.abs()
}
fn sanitized_reference(raw: &str, index: usize, used: &mut BTreeSet<String>) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(48)
        .collect();
    let base = if cleaned.is_empty() {
        format!("IMP{}", index + 1)
    } else {
        cleaned
    };
    let mut reference = base.clone();
    let mut suffix = 1;
    while !used.insert(reference.to_ascii_uppercase()) {
        reference = format!("{base}_{suffix}");
        suffix += 1;
    }
    reference
}
fn properties(raw: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in raw.lines() {
        let line = line.trim().trim_start_matches('{').trim_end_matches('}');
        if let Some((key, value)) = line.split_once('=')
            && key.len() <= 64
            && value.len() <= 4096
        {
            out.insert(key.to_ascii_uppercase(), value.to_owned());
        }
    }
    out
}

/// O perfil confirma apenas sufixos SI simples. Expressões e valores simbólicos não são avaliados.
fn numeric_value(raw: &str, kind: ComponentKind) -> Option<f64> {
    let mut value = raw.trim().replace('µ', "u");
    let unit = match kind {
        ComponentKind::Resistor => "Ω",
        ComponentKind::Capacitor => "F",
        ComponentKind::Inductor => "H",
        ComponentKind::VoltageSource => "V",
        ComponentKind::CurrentSource => "A",
        _ => "",
    };
    if !unit.is_empty()
        && let Some(number) = value.strip_suffix(unit)
    {
        value = number.trim().to_owned();
    }
    let (number, scale) = [
        ("meg", 1e6),
        ("G", 1e9),
        ("k", 1e3),
        ("m", 1e-3),
        ("u", 1e-6),
        ("n", 1e-9),
        ("p", 1e-12),
        ("f", 1e-15),
    ]
    .into_iter()
    .find_map(|(suffix, scale)| value.strip_suffix(suffix).map(|number| (number, scale)))
    .unwrap_or((&value, 1.0));
    let value = number.trim().parse::<f64>().ok()? * scale;
    if value.is_finite()
        && value.abs() <= 1e12
        && (!matches!(
            kind,
            ComponentKind::Resistor | ComponentKind::Capacitor | ComponentKind::Inductor
        ) || value > 0.0)
    {
        Some(value)
    } else {
        None
    }
}
fn match_instance<'a>(
    catalog: Option<&'a CdbDocument>,
    reference: &str,
) -> Option<&'a CdbInstance> {
    let catalog = catalog?;
    let reference = reference.trim();
    if reference.is_empty() {
        return None;
    }
    let mut matches = catalog
        .instances
        .iter()
        .filter(|instance| instance.reference.trim() == reference);
    let first = matches.next()?;
    if matches.next().is_none() {
        Some(first)
    } else {
        None
    }
}
fn ordered_pins(source: &[DsnPin], instance: Option<&CdbInstance>) -> (Vec<DsnPin>, bool) {
    let Some(instance) = instance else {
        return (source.to_vec(), true);
    };
    if instance.pins.is_empty() {
        return (source.to_vec(), source.is_empty());
    }
    let mut result = Vec::new();
    let mut taken = BTreeSet::new();
    for expected in &instance.pins {
        let candidates: Vec<_> = source
            .iter()
            .enumerate()
            .filter(|(index, pin)| {
                !taken.contains(index)
                    && ((!expected.name.is_empty() && pin.name == expected.name)
                        || (!expected.number.is_empty() && pin.number == expected.number)
                        || (!expected.name.is_empty() && pin.number == expected.name))
            })
            .collect();
        if candidates.len() != 1 {
            return (source.to_vec(), false);
        }
        let (index, pin) = candidates[0];
        taken.insert(index);
        let mut pin = pin.clone();
        if !expected.number.is_empty() {
            pin.number = expected.number.clone();
        }
        result.push(pin);
    }
    let complete = result.len() == source.len();
    (result, complete)
}

pub fn convert_document(
    name: &str,
    document: &DsnDocument,
    catalog: Option<&CdbDocument>,
    file_version: Option<u32>,
) -> Result<ImportResult, ImportError> {
    let mut project = Project::empty(name.chars().take(100).collect::<String>());
    let mut warnings = document.warnings.clone();
    let mut unsupported = Vec::new();
    // O leitor ainda não decodifica todos os tipos de objetos. Não libera simulação implicitamente.
    let mut complete = false;
    let mut unit = 0i64;
    for point in document
        .components
        .iter()
        .flat_map(|c| std::iter::once(c.position).chain(c.pins.iter().map(|p| p.position)))
        .chain(document.wires.iter().flat_map(|w| w.points.iter().copied()))
        .chain(document.junctions.iter().copied())
        .chain(document.labels.iter().map(|l| l.position))
    {
        unit = gcd(unit, i64::from(point.x));
        unit = gcd(unit, i64::from(point.y));
    }
    unit = unit.max(1);
    let normalize = |point: DsnPoint| -> Result<Point, ImportError> {
        // Coordenadas dobradas garantem centro inteiro sem arredondar pinos; Y CAD aponta para cima.
        let x = i64::from(point.x) * 2 / unit;
        let y = -i64::from(point.y) * 2 / unit;
        let point = Point::new(
            i32::try_from(x)
                .map_err(|_| ImportError::Conversion("coordenada X fora do limite".into()))?,
            i32::try_from(y)
                .map_err(|_| ImportError::Conversion("coordenada Y fora do limite".into()))?,
        );
        if !point.is_valid() {
            return Err(ImportError::Conversion(
                "geometria excede a extensão suportada pelo editor".into(),
            ));
        }
        Ok(point)
    };
    let mut references = BTreeSet::new();
    let mut used_catalog = BTreeSet::new();
    for (index, source) in document.components.iter().enumerate() {
        let instance = match_instance(catalog, &source.reference);
        if let Some(instance) = instance {
            used_catalog.insert(instance.id);
        }
        let part = instance
            .and_then(|i| i.part_id)
            .and_then(|id| catalog?.parts.iter().find(|part| part.id == id));
        let reference = sanitized_reference(&source.reference, index, &mut references);
        if reference != source.reference.trim() {
            warnings.push(format!(
                "Referência original {:?} normalizada para {reference}.",
                source.reference.trim()
            ));
        }
        let device = part
            .map_or(source.symbol_name.as_str(), |p| p.device.as_str())
            .trim()
            .to_owned();
        let value_text = part
            .map_or(source.value.as_str(), |p| p.value.as_str())
            .trim()
            .to_owned();
        let (pins, pins_match) = ordered_pins(&source.pins, instance);
        if !source.pins_complete || !pins_match {
            warnings.push(format!(
                "{reference}: pinos não reconciliados integralmente entre DSN e CDB."
            ));
        }
        let mut positions = Vec::new();
        for pin in &pins {
            positions.push(normalize(pin.position)?);
        }
        let position = if let Some(first) = positions.first() {
            let min_x = positions.iter().map(|p| p.x).min().unwrap_or(first.x);
            let max_x = positions.iter().map(|p| p.x).max().unwrap_or(first.x);
            let min_y = positions.iter().map(|p| p.y).min().unwrap_or(first.y);
            let max_y = positions.iter().map(|p| p.y).max().unwrap_or(first.y);
            Point::new((min_x + max_x) / 2, (min_y + max_y) / 2)
        } else {
            normalize(source.position)?
        };
        let mut numbers = BTreeSet::new();
        let mut imported_pins = Vec::new();
        for (pin_index, (pin, position_pin)) in pins.iter().zip(positions).enumerate() {
            let number = if !pin.number.is_empty() {
                pin.number.clone()
            } else if !pin.name.is_empty() {
                pin.name.clone()
            } else {
                (pin_index + 1).to_string()
            };
            if !numbers.insert(number.clone()) {
                return Err(ImportError::Conversion(format!(
                    "{reference}: número de pino repetido {number}"
                )));
            }
            imported_pins.push(ImportedPin {
                number,
                name: pin.name.clone(),
                offset: Point::new(position_pin.x - position.x, position_pin.y - position.y),
            });
        }
        let candidate = match device.to_ascii_uppercase().as_str() {
            "RESISTOR" => Some(ComponentKind::Resistor),
            "CAPACITOR" => Some(ComponentKind::Capacitor),
            "INDUCTOR" => Some(ComponentKind::Inductor),
            "VDC" | "BATTERY" => Some(ComponentKind::VoltageSource),
            "IDC" => Some(ComponentKind::CurrentSource),
            "GND" | "GROUND" => Some(ComponentKind::Ground),
            _ => None,
        };
        let mapped = candidate.and_then(|kind| {
            if imported_pins.len() != kind.pin_count() as usize
                || !source.pins_complete
                || !pins_match
            {
                return None;
            }
            if kind == ComponentKind::Ground {
                Some((kind, 0.0))
            } else {
                numeric_value(&value_text, kind).map(|value| (kind, value))
            }
        });
        let (kind, value) = mapped.unwrap_or((ComponentKind::Imported, 0.0));
        if kind == ComponentKind::Imported {
            unsupported.push(format!(
                "{reference}: {device} (valor original: {value_text})"
            ));
        }
        let mut props = properties(&source.properties_raw);
        props.insert("PROTEUS_SYMBOL".into(), source.symbol_name.clone());
        props.insert(
            "PROTEUS_REFERENCE".into(),
            source.reference.trim().to_owned(),
        );
        props.insert("PROTEUS_SOURCE_ID".into(), source.source_id.to_string());
        props.insert(
            "PROTEUS_ANGLE_TENTHS".into(),
            source.angle_tenths.to_string(),
        );
        props.insert(
            "PROTEUS_MIRROR_FLAGS".into(),
            source.mirror_flags.to_string(),
        );
        props.insert("PROTEUS_COORDINATE_UNIT".into(), unit.to_string());
        if let Some(part) = part {
            props.insert("PROTEUS_PACKAGE".into(), part.package.clone());
            for (key, value) in properties(&part.properties) {
                props.entry(key).or_insert(value);
            }
        }
        let id = ComponentId(project.next_id);
        project.next_id += 1;
        project.components.insert(
            id,
            Component {
                id,
                reference,
                kind,
                value,
                position,
                rotation: 0,
                parameters: Default::default(),
                device: None,
                footprint: None,
                imported: Some(ImportedComponent {
                    source_format: "Proteus ISIS 8".into(),
                    device,
                    value_text,
                    properties: props,
                    pins: imported_pins,
                }),
            },
        );
    }
    for source in &document.wires {
        for segment in source.points.windows(2) {
            let start = normalize(segment[0])?;
            let end = normalize(segment[1])?;
            if start == end {
                continue;
            }
            if start.x != end.x && start.y != end.y {
                return Err(ImportError::Unsupported(format!(
                    "fio diagonal em DSN 0x{:X}; não foi alterada sua geometria",
                    source.offset
                )));
            }
            let id = WireId(project.next_id);
            project.next_id += 1;
            project.wires.insert(id, Wire { id, start, end });
        }
    }
    for source in &document.junctions {
        let position = normalize(*source)?;
        let id = JunctionId(project.next_id);
        project.next_id += 1;
        project.junctions.insert(id, Junction { id, position });
    }
    for source in &document.labels {
        let position = normalize(source.position)?;
        if source.is_ground {
            let id = ComponentId(project.next_id);
            project.next_id += 1;
            let reference =
                sanitized_reference("GND_IMPORT", project.components.len(), &mut references);
            project.components.insert(
                id,
                Component {
                    id,
                    reference,
                    kind: ComponentKind::Ground,
                    value: 0.0,
                    position,
                    rotation: 0,
                    parameters: Default::default(),
                    device: None,
                    footprint: None,
                    imported: Some(ImportedComponent {
                        source_format: "Proteus ISIS 8".into(),
                        device: "TERGROUND".into(),
                        value_text: "0".into(),
                        properties: BTreeMap::new(),
                        pins: vec![ImportedPin {
                            number: "1".into(),
                            name: "GND".into(),
                            offset: Point::default(),
                        }],
                    }),
                },
            );
        }
        let id = LabelId(project.next_id);
        project.next_id += 1;
        project.labels.insert(
            id,
            NetLabel {
                id,
                position,
                name: source.name.trim().to_owned(),
            },
        );
    }
    if let Some(catalog) = catalog {
        let missing: Vec<_> = catalog
            .instances
            .iter()
            .filter(|instance| {
                !used_catalog.contains(&instance.id) && !instance.reference.trim().is_empty()
            })
            .map(|instance| instance.reference.clone())
            .collect();
        if !missing.is_empty() {
            warnings.push(format!(
                "Entradas CDB sem instância reconciliada no DSN: {}.",
                missing.join(", ")
            ));
            complete = false;
        }
    }
    warnings.push("Importação experimental para edição e inspeção: fontes especiais, instrumentos, barramentos e todos os tipos de terminais ainda não têm equivalência completa. A simulação deste projeto fica bloqueada para evitar resultados com ligações omitidas.".into());
    let report = ImportReport {
        format: "Proteus ISIS 8 (perfil 800)".into(),
        file_version,
        components: project.components.len(),
        wires: project.wires.len(),
        junctions: project.junctions.len(),
        labels: project.labels.len(),
        unsupported_components: unsupported,
        warnings,
        electrical_complete: complete,
    };
    let mut result = ImportResult { project, report };
    sync_notes(&mut result);
    result
        .project
        .validate()
        .map_err(|error| ImportError::Conversion(error.to_string()))?;
    Ok(result)
}

fn sync_notes(result: &mut ImportResult) {
    let mut notes = result.report.warnings.clone();
    notes.extend(
        result
            .report
            .unsupported_components
            .iter()
            .map(|item| format!("Modelo não suportado: {item}")),
    );
    if notes.len() > 255 {
        let omitted = notes.len() - 254;
        notes.truncate(254);
        notes.push(format!(
            "Mais {omitted} avisos; consulte o relatório completo da importação."
        ));
    }
    for note in &mut notes {
        if note.len() > 4096 {
            *note = note.chars().take(1000).collect::<String>() + " [aviso abreviado]";
        }
    }
    result.project.import_info = Some(ImportedProjectInfo {
        source_format: result.report.format.clone(),
        electrical_complete: result.report.electrical_complete,
        notes,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn units_are_si_and_not_guessed() {
        assert_eq!(numeric_value("2k ", ComponentKind::Resistor), Some(2000.0));
        assert_eq!(numeric_value("100f", ComponentKind::Capacitor), Some(1e-13));
        assert_eq!(numeric_value("X", ComponentKind::VoltageSource), None);
        assert_eq!(numeric_value("1M", ComponentKind::Resistor), None);
        assert_eq!(numeric_value("NaN", ComponentKind::Resistor), None);
    }
    #[test]
    fn original_pin_positions_survive_rotation_and_round_trip() {
        let doc = DsnDocument {
            components: vec![dsn::DsnComponent {
                offset: 0,
                source_id: 1,
                reference: "R1".into(),
                value: "2k".into(),
                symbol_name: "RESISTOR".into(),
                position: DsnPoint { x: 0, y: 0 },
                angle_tenths: 0,
                mirror_flags: 0,
                properties_raw: String::new(),
                pins: vec![
                    DsnPin {
                        name: "1".into(),
                        number: "1".into(),
                        position: DsnPoint { x: 0, y: 0 },
                    },
                    DsnPin {
                        name: "2".into(),
                        number: "2".into(),
                        position: DsnPoint { x: 1000, y: 0 },
                    },
                ],
                pins_complete: true,
            }],
            ..DsnDocument::default()
        };
        let imported = convert_document("Teste", &doc, None, Some(800)).unwrap();
        let comp = imported.project.components.values().next().unwrap();
        assert_eq!(comp.kind, ComponentKind::Resistor);
        assert_eq!(comp.value, 2000.0);
        assert_eq!(comp.pins()[0].position, Point::new(0, 0));
        assert_eq!(comp.pins()[1].position, Point::new(2, 0));
        let json = serde_json::to_vec(&imported.project).unwrap();
        let project: Project = serde_json::from_slice(&json).unwrap();
        assert_eq!(project, imported.project);
        assert!(!project.import_info.unwrap().electrical_complete);
    }
}
