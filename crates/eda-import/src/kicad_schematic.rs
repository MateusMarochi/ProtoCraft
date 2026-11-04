//! Intercâmbio independente de conectividade para esquemas KiCad modernos.
use crate::interchange::{InterchangeIssue, InterchangeReport, InterchangeResult};
use crate::sexpr::{self, SExpr};
use eda_core::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::Path,
};

const MAX_SCHEMATIC_BYTES: u64 = 8 * 1024 * 1024;

fn error() -> String {
    eda_i18n::text("Esquemático KiCad inválido ou fora do subconjunto suportado.")
}
fn report() -> InterchangeReport {
    InterchangeReport {
        format: "kicad_schematic".into(),
        electrical_complete: true,
        geometry_complete: true,
        losses: vec![],
        warnings: vec![],
    }
}
fn loss(r: &mut InterchangeReport, code: &str, electrical: bool) {
    r.geometry_complete = false;
    if electrical {
        r.electrical_complete = false;
    }
    if !r.losses.iter().any(|i| i.code == code) {
        r.losses.push(InterchangeIssue {
            code: code.into(),
            message: eda_i18n::text(match code {
                "SCHEMATIC_GRAPHICS" => "Gráficos de símbolos e anotações não foram preservados.",
                "UNSUPPORTED_SCHEMATIC_OBJECT" => {
                    "Objetos de esquema sem suporte foram omitidos; revise a conectividade."
                }
                "MISSING_SYMBOL" => {
                    "Símbolo sem definição incorporada ou com herança não resolvida."
                }
                "PIN_MAPPING" => {
                    "Unidades ou pinos de símbolo não puderam ser mapeados integralmente."
                }
                "POWER_SYMBOL_MAPPING" => {
                    "Símbolo de alimentação não pôde ser convertido em uma rede global sem ambiguidade."
                }
                "COMPONENT_MODEL" => {
                    "Tipos elétricos de pinos e modelos de simulação não foram importados; somente pinos e campos estão disponíveis."
                }
                "FOOTPRINT_MAPPING" => {
                    "A referência de footprint foi preservada como propriedade, mas não foi associada ao catálogo físico."
                }
                "PCB_NOT_SCHEMATIC" => {
                    "A geometria PCB não representa o esquema original e não foi exportada."
                }
                _ => "Metadados e modelos de simulação não são convertidos por este intercâmbio.",
            }),
        });
    }
}
fn num(s: &str) -> Result<f64, String> {
    s.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(error)
}
fn grid(v: f64) -> Result<i32, String> {
    let n = v * 1000.0;
    if !n.is_finite() || n.abs() > 1_000_000.0 || (n - n.round()).abs() > 1e-6 {
        return Err(error());
    }
    Ok(n.round() as i32)
}
fn point(n: &SExpr) -> Result<Point, String> {
    Ok(Point::new(
        grid(num(n.value(1).ok_or_else(error)?)?)?,
        grid(num(n.value(2).ok_or_else(error)?)?)?,
    ))
}
fn val<'a>(n: &'a SExpr, k: &str) -> Option<&'a str> {
    n.child(k).and_then(|c| c.value(1))
}
fn next(p: &mut Project) -> u64 {
    let id = p.next_id;
    p.next_id += 1;
    id
}

fn normalized_reference(raw: &str, index: usize, used: &mut BTreeSet<String>) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .take(48)
        .collect();
    let base = if cleaned.trim_matches('_').is_empty() {
        format!("KICAD{}", index + 1)
    } else {
        cleaned
    };
    let mut result = base.clone();
    let mut suffix = 1;
    while !used.insert(result.to_ascii_uppercase()) {
        result = format!("{base}_{suffix}");
        suffix += 1;
    }
    result
}
fn property<'a>(n: &'a SExpr, name: &str) -> Option<&'a str> {
    n.children("property")
        .find(|p| p.value(1) == Some(name))
        .and_then(|p| p.value(2))
}
fn pins<'a>(n: &'a SExpr, unit: u32, out: &mut Vec<&'a SExpr>) {
    for p in n.children("pin") {
        out.push(p);
    }
    for s in n.children("symbol") {
        let name = s.value(1).unwrap_or("");
        let chunks: Vec<_> = name.rsplit('_').take(2).collect();
        let u = chunks
            .get(1)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(unit);
        let style = chunks
            .first()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1);
        if (u == 0 || u == unit) && style <= 1 {
            pins(s, unit, out);
        }
    }
}

pub fn import_kicad_schematic(path: impl AsRef<Path>) -> Result<InterchangeResult, String> {
    let path = path.as_ref();
    let file = std::fs::File::open(path).map_err(|_| error())?;
    if file.metadata().map_err(|_| error())?.len() > MAX_SCHEMATIC_BYTES {
        return Err(error());
    }
    let mut bytes = Vec::new();
    file.take(MAX_SCHEMATIC_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| error())?;
    if bytes.len() as u64 > MAX_SCHEMATIC_BYTES {
        return Err(error());
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| error())?;
    parse_kicad_schematic(
        path.file_stem().and_then(|s| s.to_str()).unwrap_or("KiCad"),
        text,
    )
}

pub fn parse_kicad_schematic(name: &str, text: &str) -> Result<InterchangeResult, String> {
    let root = sexpr::parse(text)?;
    if root.head() != Some("kicad_sch") {
        return Err(error());
    }
    let mut p = Project::empty(name);
    let mut r = report();
    let libs = root.child("lib_symbols");
    let mut refs: BTreeMap<String, ComponentId> = BTreeMap::new();
    let mut used_references = BTreeSet::new();
    for obj in root.list().ok_or_else(error)?.iter().skip(1) {
        match obj.head() {
            Some("symbol") => {
                let reference = property(obj, "Reference").ok_or_else(error)?.to_owned();
                let device = val(obj, "lib_id")
                    .or_else(|| obj.value(1))
                    .ok_or_else(error)?;
                let lib =
                    libs.and_then(|l| l.children("symbol").find(|s| s.value(1) == Some(device)));
                let Some(lib) = lib else {
                    loss(&mut r, "MISSING_SYMBOL", true);
                    continue;
                };
                if lib.child("extends").is_some() {
                    loss(&mut r, "MISSING_SYMBOL", true);
                    continue;
                }
                let at = obj.child("at").ok_or_else(error)?;
                let pos = point(at)?;
                let angle = num(at.value(3).unwrap_or("0"))?;
                if (angle / 90.0 - (angle / 90.0).round()).abs() > 1e-8 {
                    loss(&mut r, "PIN_MAPPING", true);
                    continue;
                }
                let unit = val(obj, "unit")
                    .unwrap_or("1")
                    .parse::<u32>()
                    .map_err(|_| error())?;
                let mut local = vec![];
                pins(lib, unit, &mut local);
                if local.is_empty() {
                    loss(&mut r, "PIN_MAPPING", true);
                    continue;
                }
                let mirror = val(obj, "mirror");
                if mirror.is_some_and(|m| m != "x" && m != "y") {
                    loss(&mut r, "PIN_MAPPING", true);
                    continue;
                }
                let id = if let Some(id) = refs.get(&reference) {
                    *id
                } else {
                    let safe_reference =
                        normalized_reference(&reference, p.components.len(), &mut used_references);
                    if safe_reference != reference {
                        r.warnings.push(eda_i18n::format(
                            "Referência KiCad {reference} foi normalizada para {safe_reference}; o nome original foi mantido nos metadados.",
                            &[
                                ("reference", reference.clone()),
                                ("safe_reference", safe_reference.clone()),
                            ],
                        ));
                    }
                    let id = ComponentId(next(&mut p));
                    refs.insert(reference.clone(), id);
                    p.components.insert(
                        id,
                        Component {
                            id,
                            reference: safe_reference,
                            kind: ComponentKind::Imported,
                            value: 0.0,
                            position: Point::new(0, 0),
                            rotation: 0,
                            imported: Some(ImportedComponent {
                                source_format: "kicad_schematic".into(),
                                device: device.into(),
                                value_text: property(obj, "Value").unwrap_or("").into(),
                                properties: BTreeMap::new(),
                                pins: vec![],
                            }),
                            device: None,
                            footprint: None,
                            parameters: Default::default(),
                        },
                    );
                    id
                };
                if let Some(imported) = p
                    .components
                    .get_mut(&id)
                    .and_then(|component| component.imported.as_mut())
                {
                    for field in ["Reference", "Value", "Footprint", "Datasheet"] {
                        if let Some(value) = property(obj, field) {
                            imported.properties.insert(
                                format!("KICAD_{}", field.to_ascii_uppercase()),
                                value.into(),
                            );
                        }
                    }
                    imported
                        .properties
                        .insert("KICAD_ORIGINAL_REFERENCE".into(), reference.clone());
                    for field in obj.children("property") {
                        if let (Some(name), Some(value)) = (field.value(1), field.value(2)) {
                            imported
                                .properties
                                .insert(format!("KICAD_PROPERTY_{name}"), value.into());
                        }
                    }
                    for field in ["dnp", "in_bom", "on_board"] {
                        if let Some(value) = val(obj, field) {
                            imported.properties.insert(
                                format!("KICAD_{}", field.to_ascii_uppercase()),
                                value.into(),
                            );
                        }
                    }
                    for field in ["unit", "mirror"] {
                        if let Some(value) = val(obj, field) {
                            imported.properties.insert(
                                format!("KICAD_{}", field.to_ascii_uppercase()),
                                value.into(),
                            );
                        }
                    }
                    if let Some(value) = obj.child("at").and_then(|at| at.value(3)) {
                        imported
                            .properties
                            .insert("KICAD_ANGLE".into(), value.into());
                    }
                    for pin in &local {
                        let number = val(pin, "number").unwrap_or("");
                        let electrical_type = val(pin, "type").unwrap_or("unknown");
                        if !number.is_empty() {
                            imported.properties.insert(
                                format!("KICAD_PIN_{number}_ELECTRICAL_TYPE"),
                                electrical_type.into(),
                            );
                        }
                    }
                }
                for pin in local {
                    let at = pin.child("at").ok_or_else(error)?;
                    let mut x = num(at.value(1).ok_or_else(error)?)?;
                    let mut y = num(at.value(2).ok_or_else(error)?)?;
                    if mirror == Some("x") {
                        y = -y;
                    }
                    if mirror == Some("y") {
                        x = -x;
                    }
                    let rad = angle.to_radians();
                    let offset = Point::new(
                        pos.x + grid(rad.cos() * x - rad.sin() * y)?,
                        pos.y - grid(rad.sin() * x + rad.cos() * y)?,
                    );
                    let number = val(pin, "number").ok_or_else(error)?.to_owned();
                    let name = val(pin, "name").unwrap_or("").to_owned();
                    let imported = p
                        .components
                        .get_mut(&id)
                        .and_then(|c| c.imported.as_mut())
                        .ok_or_else(error)?;
                    if let Some(prior) = imported.pins.iter().find(|v| v.number == number) {
                        if prior.offset != offset {
                            loss(&mut r, "PIN_MAPPING", true);
                        }
                    } else {
                        imported.pins.push(ImportedPin {
                            number,
                            name,
                            offset,
                        });
                    }
                }
                // O KiCad codifica alimentação como símbolo, enquanto o núcleo
                // representa essa união por nome de rede. Só convertemos quando
                // valor e pino concordam; nomes ambíguos não podem virar conexão.
                if lib.child("power").is_some() {
                    let value = property(obj, "Value").unwrap_or("").trim();
                    let imported = p
                        .components
                        .get(&id)
                        .and_then(|component| component.imported.as_ref());
                    let matching_pin = imported.and_then(|component| {
                        (component.pins.len() == 1 && component.pins[0].name.trim() == value)
                            .then_some(&component.pins[0])
                    });
                    if value.is_empty() || matching_pin.is_none() {
                        loss(&mut r, "POWER_SYMBOL_MAPPING", true);
                    } else if let Some(pin) = matching_pin {
                        let position = Point::new(
                            p.components[&id].position.x + pin.offset.x,
                            p.components[&id].position.y + pin.offset.y,
                        );
                        let label_id = LabelId(next(&mut p));
                        p.labels.insert(
                            label_id,
                            NetLabel {
                                id: label_id,
                                position,
                                name: value.to_owned(),
                            },
                        );
                    }
                }
                loss(&mut r, "SCHEMATIC_GRAPHICS", false);
                // O núcleo conserva a posição e o número dos pinos, mas ainda não
                // executa os tipos ERC nem modelos elétricos declarados no KiCad.
                loss(&mut r, "COMPONENT_MODEL", true);
                if property(obj, "Footprint").is_some_and(|value| !value.trim().is_empty()) {
                    loss(&mut r, "FOOTPRINT_MAPPING", false);
                }
            }
            Some("wire") => {
                let pts = obj.child("pts").ok_or_else(error)?;
                let pts: Vec<_> = pts.children("xy").map(point).collect::<Result<_, _>>()?;
                if pts.len() != 2 {
                    return Err(error());
                }
                let id = WireId(next(&mut p));
                p.wires.insert(
                    id,
                    Wire {
                        id,
                        start: pts[0],
                        end: pts[1],
                    },
                );
            }
            Some("junction") => {
                let id = JunctionId(next(&mut p));
                p.junctions.insert(
                    id,
                    Junction {
                        id,
                        position: point(obj.child("at").ok_or_else(error)?)?,
                    },
                );
            }
            Some("label" | "global_label") => {
                let id = LabelId(next(&mut p));
                p.labels.insert(
                    id,
                    NetLabel {
                        id,
                        position: point(obj.child("at").ok_or_else(error)?)?,
                        name: obj.value(1).ok_or_else(error)?.into(),
                    },
                );
            }
            Some(
                "version" | "generator" | "generator_version" | "uuid" | "paper" | "lib_symbols"
                | "sheet_instances" | "embedded_fonts",
            ) => {}
            Some("title_block" | "text" | "text_box" | "polyline" | "image") => {
                loss(&mut r, "SCHEMATIC_GRAPHICS", false)
            }
            _ => loss(&mut r, "UNSUPPORTED_SCHEMATIC_OBJECT", true),
        }
    }
    r.warnings.push(eda_i18n::text("Conectividade importada sem modelos de simulação; símbolos são representados por pinos e metadados."));
    p.import_info = Some(ImportedProjectInfo {
        source_format: "kicad_schematic".into(),
        electrical_complete: r.electrical_complete,
        notes: r.warnings.clone(),
    });
    p.validate().map_err(|e| e.to_string())?;
    Ok(InterchangeResult {
        project: p,
        report: r,
    })
}

fn q(v: &str) -> Result<String, String> {
    if v.chars().any(char::is_control) {
        return Err(eda_i18n::text(
            "Texto contém caracteres de controle sem representação segura em S-expression.",
        ));
    }
    let mut result = String::with_capacity(v.len() + 2);
    result.push('"');
    for character in v.chars() {
        match character {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            value => result.push(value),
        }
    }
    result.push('"');
    Ok(result)
}
fn uuid(kind: u32, id: u64) -> String {
    format!("{:08x}-0000-4000-8000-{:012x}", kind, id & 0xffffffffffff)
}
fn mm(v: i32) -> f64 {
    v as f64 / 1000.0
}
pub fn export_kicad_schematic(project: &Project) -> Result<(String, InterchangeReport), String> {
    project.validate().map_err(|e| e.to_string())?;
    let mut r = report();
    loss(&mut r, "SCHEMATIC_GRAPHICS", false);
    if project.board.is_some() {
        loss(&mut r, "PCB_NOT_SCHEMATIC", false);
    }
    if !project.components.is_empty() {
        loss(&mut r, "COMPONENT_MODEL", true);
    }
    let root_id = uuid(1, 1);
    let mut s = format!(
        "(kicad_sch (version 20250114) (generator protocraft) (uuid {root_id}) (paper \"A4\") (lib_symbols\n"
    );
    for c in project.components.values() {
        let name = format!("ProtoCraft:Component{}", c.id.0);
        s += &format!(
            "(symbol {} (pin_names (offset 0)) (in_bom yes) (on_board yes) (property \"Reference\" {} (at 0 2.54 0) (effects (font (size 1.27 1.27)))) (property \"Value\" {} (at 0 0 0) (effects (font (size 1.27 1.27)))) (symbol {}",
            q(&name)?,
            q(&c.reference)?,
            q(&c.value.to_string())?,
            q(&format!("Component{}_1_1", c.id.0))?
        );
        for p in c.pins() {
            let number = c.pin_number(p.reference.pin);
            let name = c.pin_name(p.reference.pin);
            let x = mm(p.position.x - c.position.x);
            let y = -mm(p.position.y - c.position.y);
            s += &format!(
                " (pin passive line (at {x} {y} 0) (length 0) (name {} (effects (font (size 1.27 1.27)))) (number {} (effects (font (size 1.27 1.27)))))",
                q(name)?,
                q(&number)?
            );
        }
        s += "))\n";
    }
    s += ")\n";
    for c in project.components.values() {
        let lib = format!("ProtoCraft:Component{}", c.id.0);
        let x = mm(c.position.x);
        let y = mm(c.position.y);
        let reference = c
            .imported
            .as_ref()
            .and_then(|imported| imported.properties.get("KICAD_ORIGINAL_REFERENCE"))
            .map_or(c.reference.as_str(), String::as_str);
        let value = c
            .imported
            .as_ref()
            .map(|i| i.value_text.clone())
            .unwrap_or_else(|| c.value.to_string());
        s += &format!(
            "(symbol (lib_id {}) (at {x} {y} 0) (unit 1) (in_bom yes) (on_board yes) (dnp no) (uuid {}) (property \"Reference\" {} (at {x} {y} 0) (effects (font (size 1.27 1.27)))) (property \"Value\" {} (at {x} {y} 0) (effects (font (size 1.27 1.27))))",
            q(&lib)?,
            uuid(2, c.id.0),
            q(reference)?,
            q(&value)?,
        );
        let footprint = c
            .footprint
            .as_ref()
            .map(|footprint| footprint.id.as_str())
            .or_else(|| {
                c.imported
                    .as_ref()
                    .and_then(|imported| imported.properties.get("KICAD_FOOTPRINT"))
                    .map(String::as_str)
            });
        if let Some(footprint) = footprint.filter(|footprint| !footprint.is_empty()) {
            s += &format!(
                " (property \"Footprint\" {} (at {x} {y} 0) (effects (font (size 1.27 1.27))))",
                q(footprint)?
            );
        }
        if let Some(datasheet) = c
            .imported
            .as_ref()
            .and_then(|imported| imported.properties.get("KICAD_DATASHEET"))
            .filter(|value| !value.is_empty())
        {
            s += &format!(
                " (property \"Datasheet\" {} (at {x} {y} 0) (effects (font (size 1.27 1.27))))",
                q(datasheet)?
            );
        }
        if let Some(imported) = &c.imported {
            for (key, property_value) in &imported.properties {
                let Some(name) = key.strip_prefix("KICAD_PROPERTY_") else {
                    continue;
                };
                if matches!(name, "Reference" | "Value" | "Footprint" | "Datasheet") {
                    continue;
                }
                s += &format!(
                    " (property {} {} (at {x} {y} 0) (effects (font (size 1.27 1.27))))",
                    q(name)?,
                    q(property_value)?
                );
            }
        }
        s += &format!(
            " (instances (project {} (path \"/{root_id}\" (reference {}) (unit 1)))))\n",
            q(&project.name)?,
            q(reference)?
        );
    }
    for w in project.wires.values() {
        s += &format!(
            "(wire (pts (xy {} {}) (xy {} {})) (stroke (width 0) (type default)) (uuid {}))\n",
            mm(w.start.x),
            mm(w.start.y),
            mm(w.end.x),
            mm(w.end.y),
            uuid(3, w.id.0)
        );
    }
    for j in project.junctions.values() {
        s += &format!(
            "(junction (at {} {}) (diameter 0) (color 0 0 0 0) (uuid {}))\n",
            mm(j.position.x),
            mm(j.position.y),
            uuid(4, j.id.0)
        );
    }
    for l in project.labels.values() {
        s += &format!(
            "(label {} (at {} {} 0) (effects (font (size 1.27 1.27))) (uuid {}))\n",
            q(&l.name)?,
            mm(l.position.x),
            mm(l.position.y),
            uuid(5, l.id.0)
        );
    }
    // Em ProtoCraft todos os terminais Ground pertencem à mesma rede; rótulos
    // locais com o mesmo nome preservam essa união no esquema KiCad gerado.
    for component in project
        .components
        .values()
        .filter(|component| component.kind == ComponentKind::Ground)
    {
        for pin in component.pins() {
            s += &format!(
                "(label {} (at {} {} 0) (effects (font (size 1.27 1.27))) (uuid {}))\n",
                q("GND")?,
                mm(pin.position.x),
                mm(pin.position.y),
                uuid(6, component.id.0)
            );
        }
    }
    s += ")\n";
    Ok((s, r))
}
