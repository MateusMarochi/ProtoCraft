//! Catálogo offline normalizado; dados CC-BY-SA com exceção KiCad, código original.
use eda_core::{Command, DataSource, DeviceDefinition, DevicePin, Footprint, Point};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceRecord {
    pub id: String,
    pub manufacturer: String,
    pub family: String,
    pub form: String,
    pub description: String,
    pub datasheet: String,
    pub symbol: String,
    pub footprint: String,
    pub source: DataSource,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub schema_version: u32,
    pub devices: Vec<DeviceRecord>,
    pub symbols: BTreeMap<String, Vec<DevicePin>>,
    pub footprints: BTreeMap<String, Footprint>,
    #[serde(skip)]
    search_index: OnceLock<DeviceSearchIndex>,
}

/// Filtros opcionais combináveis para o catálogo físico.
#[derive(Debug, Clone, Default)]
pub struct DeviceSearchFilters {
    pub manufacturer: Option<String>,
    pub family: Option<String>,
    pub form: Option<String>,
    pub feature: Option<String>,
    pub min_pins: Option<usize>,
    pub max_pins: Option<usize>,
    /// Dimensão máxima do corpo, em nanômetros; considera o maior lado.
    pub max_body_dimension_nm: Option<i64>,
}

/// Resultado com atributos físicos prontos para exibição sem novo acesso ao catálogo.
#[derive(Debug, Clone, Copy)]
pub struct DeviceSearchHit<'a> {
    pub record: &'a DeviceRecord,
    pub pin_count: usize,
    pub width_nm: i64,
    pub height_nm: i64,
}

impl DeviceSearchFilters {
    pub fn is_empty(&self) -> bool {
        self.manufacturer.is_none()
            && self.family.is_none()
            && self.form.is_none()
            && self.feature.is_none()
            && self.min_pins.is_none()
            && self.max_pins.is_none()
            && self.max_body_dimension_nm.is_none()
    }
}

#[derive(Debug, Default)]
struct DeviceSearchIndex {
    documents: Vec<DeviceSearchDocument>,
    manufacturers: Vec<String>,
    families: Vec<String>,
    forms: Vec<String>,
}

#[derive(Debug)]
struct DeviceSearchDocument {
    id: String,
    manufacturer: String,
    family: String,
    form: String,
    description: String,
    features: String,
    footprint: String,
    footprint_description: String,
    pin_count: usize,
    width_nm: i64,
    height_nm: i64,
}
static CATALOG: OnceLock<Result<Catalog, String>> = OnceLock::new();
pub fn catalog() -> Result<&'static Catalog, String> {
    CATALOG
        .get_or_init(|| {
            let catalog: Catalog =
                serde_json::from_str(include_str!("../../../assets/library/catalog.json"))
                    .map_err(|e| format!("Catálogo inválido: {e}"))?;
            if catalog.schema_version != 1 {
                return Err("Versão do catálogo não suportada.".into());
            }
            Ok(catalog)
        })
        .as_ref()
        .map_err(Clone::clone)
}
impl Catalog {
    pub fn search(&self, query: &str) -> Vec<&DeviceRecord> {
        let results: Vec<_> = self
            .search_faceted(query, &DeviceSearchFilters::default())
            .into_iter()
            .map(|hit| hit.record)
            .collect();
        if results.is_empty()
            && !query.contains(char::is_whitespace)
            && let Ok(record) = self.find(query)
        {
            return vec![record];
        }
        results
    }

    /// Busca ranqueada com filtros combináveis. O índice textual e as facetas são criados uma vez.
    pub fn search_faceted<'a>(
        &'a self,
        query: &str,
        filters: &DeviceSearchFilters,
    ) -> Vec<DeviceSearchHit<'a>> {
        let index = self.search_index();
        let query = expand_query_aliases(query);
        let terms: Vec<_> = query.split_whitespace().collect();
        let manufacturer = filters
            .manufacturer
            .as_deref()
            .filter(|value| !value.is_empty())
            .map(normalize_search_text);
        let family = filters
            .family
            .as_deref()
            .filter(|value| !value.is_empty())
            .map(normalize_search_text);
        let form = filters
            .form
            .as_deref()
            .filter(|value| !value.is_empty())
            .map(normalize_search_text);
        let feature = filters
            .feature
            .as_deref()
            .filter(|value| !value.is_empty())
            .map(|value| expand_query_aliases(value));

        let mut ranked: Vec<_> = index
            .documents
            .iter()
            .enumerate()
            .filter_map(|(position, document)| {
                if manufacturer
                    .as_deref()
                    .is_some_and(|value| document.manufacturer != value)
                    || family
                        .as_deref()
                        .is_some_and(|value| document.family != value)
                    || form.as_deref().is_some_and(|value| document.form != value)
                    || filters
                        .min_pins
                        .is_some_and(|minimum| document.pin_count < minimum)
                    || filters
                        .max_pins
                        .is_some_and(|maximum| document.pin_count > maximum)
                    || filters.max_body_dimension_nm.is_some_and(|maximum| {
                        document.width_nm <= 0
                            || document.height_nm <= 0
                            || document.width_nm.max(document.height_nm) > maximum
                    })
                {
                    return None;
                }

                // Recursos são filtrados pela descrição catalogada, não inferidos do nome.
                if let Some(feature) = &feature
                    && !feature_matches(&document.features, feature)
                {
                    return None;
                }

                if !terms
                    .iter()
                    .all(|term| document_matches_term(document, term))
                {
                    return None;
                }

                let compact_query = query.replace(' ', "");
                let score =
                    if !compact_query.is_empty() && document.id.replace(' ', "") == compact_query {
                        10_000
                    } else if !compact_query.is_empty()
                        && document.id.replace(' ', "").starts_with(&compact_query)
                    {
                        8_000
                    } else {
                        terms
                            .iter()
                            .map(|term| {
                                let alternatives = search_term_aliases(term);
                                if alternatives.iter().any(|value| {
                                    document.id.contains(value) && !value.contains(' ')
                                }) {
                                    700
                                } else if field_matches(&document.family, &alternatives) {
                                    500
                                } else if field_matches(&document.manufacturer, &alternatives) {
                                    400
                                } else if alternatives.iter().any(|value| {
                                    field_matches(&document.footprint, &[*value])
                                        || field_matches(&document.footprint_description, &[*value])
                                }) {
                                    300
                                } else if field_matches(&document.form, &alternatives) {
                                    250
                                } else {
                                    100
                                }
                            })
                            .sum()
                    };
                Some((position, score))
            })
            .collect();
        ranked.sort_by(
            |(left_position, left_score), (right_position, right_score)| {
                right_score.cmp(left_score).then_with(|| {
                    index.documents[*left_position]
                        .id
                        .cmp(&index.documents[*right_position].id)
                })
            },
        );

        ranked
            .into_iter()
            .filter_map(|(position, _)| {
                let document = &index.documents[position];
                self.devices.get(position).map(|record| DeviceSearchHit {
                    record,
                    pin_count: document.pin_count,
                    width_nm: document.width_nm,
                    height_nm: document.height_nm,
                })
            })
            .collect()
    }

    pub fn manufacturers(&self) -> &[String] {
        &self.search_index().manufacturers
    }

    pub fn families(&self) -> &[String] {
        &self.search_index().families
    }

    pub fn forms(&self) -> &[String] {
        &self.search_index().forms
    }

    fn search_index(&self) -> &DeviceSearchIndex {
        self.search_index.get_or_init(|| {
            let mut manufacturers = BTreeSet::new();
            let mut families = BTreeSet::new();
            let mut forms = BTreeSet::new();
            let documents = self
                .devices
                .iter()
                .map(|record| {
                    manufacturers.insert(record.manufacturer.clone());
                    families.insert(record.family.clone());
                    forms.insert(record.form.clone());
                    let footprint = self.footprints.get(&record.footprint);
                    let footprint_description = footprint
                        .map(|value| value.description.as_str())
                        .unwrap_or_default();
                    let id = normalize_search_text(&record.id);
                    let manufacturer = normalize_search_text(&record.manufacturer);
                    let family = normalize_search_text(&record.family);
                    let form = normalize_search_text(&record.form);
                    let description = normalize_search_text(&record.description);
                    let footprint_id = normalize_search_text(&record.footprint);
                    let footprint_description = normalize_search_text(footprint_description);
                    let features = format!("{description} {footprint_description}");
                    let (width_nm, height_nm) = footprint.map_or((0, 0), |value| {
                        (value.body_bounds.width(), value.body_bounds.height())
                    });
                    DeviceSearchDocument {
                        id,
                        manufacturer,
                        family,
                        form,
                        description,
                        features,
                        footprint: footprint_id,
                        footprint_description,
                        pin_count: self.symbols.get(&record.symbol).map_or(0, Vec::len),
                        width_nm,
                        height_nm,
                    }
                })
                .collect();
            DeviceSearchIndex {
                documents,
                manufacturers: manufacturers.into_iter().collect(),
                families: families.into_iter().collect(),
                forms: forms.into_iter().collect(),
            }
        })
    }
    /// O x terminal usado pelo catálogo pode representar sufixo de grau/embalagem;
    /// não criamos aliases de encapsulamento ou de memória por semelhança de nome.
    pub fn find(&self, id: &str) -> Result<&DeviceRecord, String> {
        if let Some(record) = self.devices.iter().find(|d| d.id.eq_ignore_ascii_case(id)) {
            return Ok(record);
        }
        let matches: Vec<_> = self
            .devices
            .iter()
            .filter(|d| {
                d.manufacturer == "STMicroelectronics"
                    && d.id.len() == id.len()
                    && d.id
                        .bytes()
                        .zip(id.bytes())
                        .all(|(a, b)| a.eq_ignore_ascii_case(&b) || a == b'x')
            })
            .collect();
        if matches.len() == 1 {
            Ok(matches[0])
        } else {
            Err(format!(
                "Dispositivo ausente ou ambíguo: {id}. Consulte devices --filter."
            ))
        }
    }
    pub fn definition(&self, id: &str) -> Result<(DeviceDefinition, Footprint), String> {
        let record = self.find(id)?;
        let pins = self
            .symbols
            .get(&record.symbol)
            .ok_or("Símbolo ausente no catálogo")?
            .clone();
        let footprint = self
            .footprints
            .get(&record.footprint)
            .ok_or("Footprint ausente no catálogo")?
            .clone();
        let device = DeviceDefinition {
            id: record.id.clone(),
            requested_part: id.to_owned(),
            manufacturer: record.manufacturer.clone(),
            family: record.family.clone(),
            form: record.form.clone(),
            description: record.description.clone(),
            datasheet: record.datasheet.clone(),
            pins,
            source: record.source.clone(),
        };
        Ok((device, footprint))
    }
    pub fn place_command(&self, id: &str, position: Point) -> Result<Command, String> {
        let (device, footprint) = self.definition(id)?;
        Ok(Command::PlaceDevice {
            device: Box::new(device),
            footprint: Box::new(footprint),
            position,
        })
    }
}

/// Normalização compartilhada pelo índice para consultas sem acentos e sem pontuação.
fn normalize_search_text(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    for character in value.chars().flat_map(char::to_lowercase) {
        let replacement = match character {
            'á' | 'à' | 'â' | 'ã' | 'ä' => Some('a'),
            'é' | 'è' | 'ê' | 'ë' => Some('e'),
            'í' | 'ì' | 'î' | 'ï' => Some('i'),
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => Some('o'),
            'ú' | 'ù' | 'û' | 'ü' => Some('u'),
            'ç' => Some('c'),
            'ñ' => Some('n'),
            value if value.is_ascii_alphanumeric() => Some(value),
            _ => None,
        };
        if let Some(character) = replacement {
            normalized.push(character);
        } else if !normalized.ends_with(' ') && !normalized.is_empty() {
            normalized.push(' ');
        }
    }
    normalized
        .trim()
        .replace("wi fi", "wifi")
        .replace("low power", "lowpower")
}

fn expand_query_aliases(value: &str) -> String {
    let mut value = normalize_search_text(value);
    for (source, replacement) in [
        ("amplificador operacional", "op amp"),
        ("operational amplifier", "op amp"),
        ("baixo consumo", "low power"),
        ("low power", "lowpower"),
        ("sem fio", "wireless"),
        ("wi fi", "wifi"),
        ("circuito integrado", "integrated circuit"),
    ] {
        value = value.replace(source, replacement);
    }
    value
}

fn search_term_aliases<'a>(term: &'a str) -> Vec<&'a str> {
    let aliases: &[&str] = match term {
        "microcontrolador" | "microcontroladores" | "mcu" => {
            &["microcontroller", "microcontrollers", "mcu"]
        }
        "microcontroller" | "microcontrollers" => &["microcontroller", "microcontrollers", "mcu"],
        "modulo" | "modulos" | "module" | "modules" => &["module", "modules", "modulo", "modulos"],
        "placa" | "board" | "devkit" | "development" => {
            &["board", "devkit", "development", "placa"]
        }
        "encapsulamento" | "package" | "footprint" => &["package", "footprint", "encapsulamento"],
        "pino" | "pinos" | "pin" | "pins" => &["pin", "pins", "pino", "pinos"],
        "wifi" | "wireless" => &["wifi", "wireless"],
        "bluetooth" => &["bluetooth"],
        "ble" => &["bluetooth low energy", "ble"],
        "usb" => &["usb"],
        "ethernet" => &["ethernet"],
        "can" => &["can"],
        "adc" => &["adc", "analog to digital", "analog-to-digital"],
        "lowpower" | "baixo" | "consumo" => &["lowpower"],
        "integrated" | "circuit" => &["integrated", "circuit", "ic"],
        _ => return vec![term],
    };
    aliases.to_vec()
}

fn field_matches(field: &str, alternatives: &[&str]) -> bool {
    alternatives.iter().any(|term| {
        if term.contains(' ') {
            field.contains(term)
        } else if term.len() <= 3 {
            field.split_whitespace().any(|word| word == *term)
        } else {
            field.contains(term)
        }
    })
}

fn document_matches_term(document: &DeviceSearchDocument, term: &str) -> bool {
    let alternatives = search_term_aliases(term);
    alternatives.iter().any(|alternative| {
        document.id.contains(alternative)
            || field_matches(&document.manufacturer, &[*alternative])
            || field_matches(&document.family, &[*alternative])
            || field_matches(&document.form, &[*alternative])
            || field_matches(&document.description, &[*alternative])
            || field_matches(&document.footprint, &[*alternative])
            || field_matches(&document.footprint_description, &[*alternative])
    })
}

fn feature_matches(description: &str, feature: &str) -> bool {
    let terms: &[&str] = match feature {
        "wifi" | "wireless" => &["wifi", "wireless"],
        "bluetooth" => &["bluetooth"],
        "ble" => &["bluetooth low energy", "ble"],
        "usb" => &["usb"],
        "can" => &["can"],
        "ethernet" => &["ethernet"],
        "lowpower" => &["lowpower"],
        "adc" => &["adc", "analog to digital"],
        _ => {
            return feature
                .split_whitespace()
                .all(|term| field_matches(description, &[term]));
        }
    };
    terms.iter().any(|term| {
        if term.contains(' ') {
            description.contains(term)
        } else {
            description.split_whitespace().any(|word| word == *term)
        }
    })
}

/// Demonstra vínculo símbolo/pad e duas famílias. Não é uma placa de aplicação completa.
pub fn physical_example() -> Result<eda_core::Project, String> {
    use eda_core::*;
    let c = catalog()?;
    let mut e = Engine::new(Project::empty("STM32 + ESP32-S3 — pinagem e PCB"))
        .map_err(|e| e.to_string())?;
    e.execute(Command::Batch(vec![
        c.place_command("STM32F103C8Tx", Point::new(-35, 0))?,
        c.place_command("ESP32-S3-WROOM-1", Point::new(35, 0))?,
        Command::PlaceComponent {
            kind: ComponentKind::Ground,
            position: Point::new(0, 45),
        },
    ]))
    .map_err(|e| e.to_string())?;
    let mut labels = Vec::new();
    for component in e.project().components.values() {
        for pin in component.pins() {
            let name = component.pin_name(pin.reference.pin);
            let net = if component.kind == ComponentKind::Ground
                || name.starts_with("VSS")
                || name == "GND"
            {
                Some("GND")
            } else if matches!(name, "VDD" | "VDDA" | "VBAT" | "3V3") {
                Some("3V3")
            } else if name == "PA9" || name.starts_with("U0RXD/") {
                Some("UART_A")
            } else if name == "PA10" || name.starts_with("U0TXD/") {
                Some("UART_B")
            } else {
                None
            };
            if let Some(net) = net {
                labels.push(Command::AddLabel {
                    position: pin.position,
                    name: net.into(),
                });
            }
        }
    }
    let board = Board {
        width_nm: 90_000_000,
        height_nm: 70_000_000,
        ..Default::default()
    };
    labels.extend([
        Command::SetBoard { board },
        Command::PlaceOnBoard {
            id: ComponentId(1),
            placement: Placement {
                position: NmPoint::new(25_000_000, 35_000_000),
                rotation: 0,
                side: BoardSide::Top,
            },
        },
        Command::PlaceOnBoard {
            id: ComponentId(2),
            placement: Placement {
                position: NmPoint::new(60_000_000, 35_000_000),
                rotation: 0,
                side: BoardSide::Top,
            },
        },
    ]);
    e.execute(Command::Batch(labels))
        .map_err(|e| e.to_string())?;
    Ok(e.project().clone())
}
