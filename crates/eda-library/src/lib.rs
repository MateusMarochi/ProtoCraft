//! Catálogo offline normalizado; dados CC-BY-SA com exceção KiCad, código original.
use eda_core::{Command, DataSource, DeviceDefinition, DevicePin, Footprint, Point};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock};

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
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub schema_version: u32,
    pub devices: Vec<DeviceRecord>,
    pub symbols: BTreeMap<String, Vec<DevicePin>>,
    pub footprints: BTreeMap<String, Footprint>,
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
        let terms: Vec<_> = query.split_whitespace().map(str::to_lowercase).collect();
        let results: Vec<_> = self
            .devices
            .iter()
            .filter(|d| {
                let text = format!(
                    "{} {} {} {} {} {}",
                    d.id, d.manufacturer, d.family, d.form, d.description, d.footprint
                )
                .to_lowercase();
                terms.iter().all(|q| text.contains(q))
            })
            .collect();
        if results.is_empty()
            && !query.contains(char::is_whitespace)
            && let Ok(record) = self.find(query)
        {
            return vec![record];
        }
        results
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
