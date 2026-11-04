//! Catálogo offline normalizado; dados CC-BY-SA com exceção KiCad, código original.
mod pcb_examples;
mod power;
mod power_copper;
use eda_core::{
    Command, DataSource, DeviceDefinition, DevicePin, Footprint, FootprintLine, NmBounds, NmPoint,
    Pad, PadShape, PadTechnology, Point,
};
pub use pcb_examples::{PcbExampleInfo, load_pcb_example, pcb_example_catalog};
pub use power::{BuckDesignResult, BuckDesignSpec, buck_pi4_project, lm61460_definition};
pub use power_copper::{CopperTraceResult, CopperTraceSpec};
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
            let mut catalog: Catalog =
                serde_json::from_str(include_str!("../../../assets/library/catalog.json"))
                    .map_err(|e| format!("Catálogo inválido: {e}"))?;
            if catalog.schema_version != 1 {
                return Err("Versão do catálogo não suportada.".into());
            }
            for (device, footprint) in raspberry_gpio_definitions() {
                if catalog.devices.iter().any(|entry| entry.id == device.id) {
                    continue;
                }
                let symbol_id = device.id.clone();
                let record = DeviceRecord {
                    id: device.id.clone(),
                    manufacturer: device.manufacturer.clone(),
                    family: device.family.clone(),
                    form: device.form.clone(),
                    description: device.description.clone(),
                    datasheet: device.datasheet.clone(),
                    symbol: symbol_id.clone(),
                    footprint: footprint.id.clone(),
                    source: device.source.clone(),
                };
                catalog.symbols.insert(symbol_id, device.pins);
                catalog.footprints.insert(footprint.id.clone(), footprint);
                catalog.devices.push(record);
            }
            let (device, footprint) = lm61460_definition();
            if !catalog.devices.iter().any(|entry| entry.id == device.id) {
                let symbol_id = device.id.clone();
                let record = DeviceRecord {
                    id: device.id.clone(),
                    manufacturer: device.manufacturer.clone(),
                    family: device.family.clone(),
                    form: device.form.clone(),
                    description: device.description.clone(),
                    datasheet: device.datasheet.clone(),
                    symbol: symbol_id.clone(),
                    footprint: footprint.id.clone(),
                    source: device.source.clone(),
                };
                catalog.symbols.insert(symbol_id, device.pins);
                catalog.footprints.insert(footprint.id.clone(), footprint);
                catalog.devices.push(record);
            }
            Ok(catalog)
        })
        .as_ref()
        .map_err(Clone::clone)
}

/// Pinagem do header GPIO de 40 posições do Raspberry Pi 4 Model B.
pub fn raspberry_pi_4_gpio_definition() -> (DeviceDefinition, Footprint) {
    raspberry_classic_40pin_definition("RaspberryPi4B_GPIO", "Raspberry Pi 4 Model B", false, false)
}

const RASPBERRY_PI_40_PIN_SIGNALS: [&str; 40] = [
    "3V3",
    "5V",
    "GPIO2/SDA1",
    "5V",
    "GPIO3/SCL1",
    "GND",
    "GPIO4/GPCLK0",
    "GPIO14/TXD0",
    "GND",
    "GPIO15/RXD0",
    "GPIO17",
    "GPIO18/PWM0",
    "GPIO27",
    "GND",
    "GPIO22",
    "GPIO23",
    "3V3",
    "GPIO24",
    "GPIO10/SPI0_MOSI",
    "GND",
    "GPIO9/SPI0_MISO",
    "GPIO25",
    "GPIO11/SPI0_SCLK",
    "GPIO8/SPI0_CE0",
    "GND",
    "GPIO7/SPI0_CE1",
    "ID_SD",
    "ID_SC",
    "GPIO5",
    "GND",
    "GPIO6",
    "GPIO12/PWM0",
    "GPIO13/PWM1",
    "GND",
    "GPIO19/PCM_FS",
    "GPIO16",
    "GPIO26",
    "GPIO20/PCM_DIN",
    "GND",
    "GPIO21/PCM_DOUT",
];

const RASPBERRY_PI_5_PIN_SIGNALS: [&str; 40] = [
    "3V3", "5V", "GPIO2", "5V", "GPIO3", "GND", "GPIO4", "GPIO14", "GND", "GPIO15", "GPIO17",
    "GPIO18", "GPIO27", "GND", "GPIO22", "GPIO23", "3V3", "GPIO24", "GPIO10", "GND", "GPIO9",
    "GPIO25", "GPIO11", "GPIO8", "GND", "GPIO7", "ID_SD", "ID_SC", "GPIO5", "GND", "GPIO6",
    "GPIO12", "GPIO13", "GND", "GPIO19", "GPIO16", "GPIO26", "GPIO20", "GND", "GPIO21",
];

const RASPBERRY_PI_26_PIN_REV1: [&str; 26] = [
    "3V3",
    "5V",
    "GPIO0/SDA0",
    "5V",
    "GPIO1/SCL0",
    "GND",
    "GPIO4/GPCLK0",
    "GPIO14/TXD0",
    "GND",
    "GPIO15/RXD0",
    "GPIO17",
    "GPIO18",
    "GPIO21",
    "GND",
    "GPIO22",
    "GPIO23",
    "3V3",
    "GPIO24",
    "GPIO10/SPI0_MOSI",
    "GND",
    "GPIO9/SPI0_MISO",
    "GPIO25",
    "GPIO11/SPI0_SCLK",
    "GPIO8/SPI0_CE0",
    "GND",
    "GPIO7/SPI0_CE1",
];

const RASPBERRY_PI_26_PIN_REV2: [&str; 26] = [
    "3V3",
    "5V",
    "GPIO2/SDA1",
    "5V",
    "GPIO3/SCL1",
    "GND",
    "GPIO4/GPCLK0",
    "GPIO14/TXD0",
    "GND",
    "GPIO15/RXD0",
    "GPIO17",
    "GPIO18",
    "GPIO27",
    "GND",
    "GPIO22",
    "GPIO23",
    "3V3",
    "GPIO24",
    "GPIO10/SPI0_MOSI",
    "GND",
    "GPIO9/SPI0_MISO",
    "GPIO25",
    "GPIO11/SPI0_SCLK",
    "GPIO8/SPI0_CE0",
    "GND",
    "GPIO7/SPI0_CE1",
];

const RASPBERRY_PICO_PIN_SIGNALS: [&str; 40] = [
    "GP0",
    "GP1",
    "GND",
    "GP2",
    "GP3",
    "GP4",
    "GP5",
    "GND",
    "GP6",
    "GP7",
    "GP8",
    "GP9",
    "GND",
    "GP10",
    "GP11",
    "GP12",
    "GP13",
    "GND",
    "GP14",
    "GP15",
    "GP16",
    "GP17",
    "GND",
    "GP18",
    "GP19",
    "GP20",
    "GP21",
    "GND",
    "GP22",
    "RUN",
    "GP26/ADC0",
    "GP27/ADC1",
    "GND/AGND",
    "GP28/ADC2",
    "ADC_VREF",
    "3V3(OUT)",
    "3V3_EN",
    "GND",
    "VSYS",
    "VBUS",
];

#[derive(Clone, Copy)]
enum RaspberryPinLayout {
    Alternating { pin_count: usize, flipped: bool },
    Pico,
}

fn raspberry_gpio_definitions() -> Vec<(DeviceDefinition, Footprint)> {
    let mut entries = vec![raspberry_pi_4_gpio_definition()];
    let classic_40 = [
        ("RaspberryPi1BPlus_GPIO40", "Raspberry Pi 1 Model B+", false),
        ("RaspberryPi1APlus_GPIO40", "Raspberry Pi 1 Model A+", false),
        ("RaspberryPi2B_GPIO40", "Raspberry Pi 2 Model B", false),
        ("RaspberryPi3B_GPIO40", "Raspberry Pi 3 Model B", false),
        ("RaspberryPi3BPlus_GPIO40", "Raspberry Pi 3 Model B+", false),
        ("RaspberryPi3APlus_GPIO40", "Raspberry Pi 3 Model A+", false),
        ("RaspberryPiZero_GPIO40", "Raspberry Pi Zero", false),
        ("RaspberryPiZeroW_GPIO40", "Raspberry Pi Zero W", false),
        ("RaspberryPiZeroWH_GPIO40", "Raspberry Pi Zero WH", false),
        ("RaspberryPiZero2W_GPIO40", "Raspberry Pi Zero 2 W", false),
        (
            "RaspberryPiZero2WWithHeaders_GPIO40",
            "Raspberry Pi Zero 2 W with headers",
            false,
        ),
        ("RaspberryPi400_GPIO40", "Raspberry Pi 400", true),
        ("RaspberryPi5_GPIO40", "Raspberry Pi 5", false),
        ("RaspberryPi500_GPIO40", "Raspberry Pi 500", true),
        ("RaspberryPi500Plus_GPIO40", "Raspberry Pi 500+", true),
    ];
    for (id, family, keyboard) in classic_40 {
        let pi5 = family.contains("Pi 5") || family.contains("Pi 500");
        entries.push(raspberry_classic_40pin_definition(
            id, family, keyboard, pi5,
        ));
    }

    entries.push(raspberry_26pin_definition(
        "RaspberryPi1BRev1_GPIO26",
        "Raspberry Pi 1 Model B, revisão 1.0",
        &RASPBERRY_PI_26_PIN_REV1,
        "https://www.raspberrypi.com/news/upcoming-board-revision/",
    ));
    entries.push(raspberry_26pin_definition(
        "RaspberryPi1BRev2_GPIO26",
        "Raspberry Pi 1 Model B, revisão 2.0",
        &RASPBERRY_PI_26_PIN_REV2,
        "https://www.raspberrypi.com/news/upcoming-board-revision/",
    ));
    entries.push(raspberry_26pin_definition(
        "RaspberryPi1A_GPIO26",
        "Raspberry Pi 1 Model A (pinagem da revisão 2.0)",
        &RASPBERRY_PI_26_PIN_REV2,
        "https://www.raspberrypi.com/news/raspberry-pi-model-a/",
    ));

    let pico_variants = [
        ("RaspberryPiPico_GPIO40", "Raspberry Pi Pico", false),
        ("RaspberryPiPicoH_GPIO40", "Raspberry Pi Pico H", false),
        ("RaspberryPiPicoW_GPIO40", "Raspberry Pi Pico W", true),
        ("RaspberryPiPicoWH_GPIO40", "Raspberry Pi Pico WH", true),
        ("RaspberryPiPico2_GPIO40", "Raspberry Pi Pico 2", false),
        (
            "RaspberryPiPico2WithHeaders_GPIO40",
            "Raspberry Pi Pico 2 with headers",
            false,
        ),
        ("RaspberryPiPico2W_GPIO40", "Raspberry Pi Pico 2 W", true),
        (
            "RaspberryPiPico2WWithHeaders_GPIO40",
            "Raspberry Pi Pico 2 W with headers",
            true,
        ),
    ];
    for (id, family, wireless) in pico_variants {
        entries.push(raspberry_pico_definition(id, family, wireless));
    }
    entries
}

fn raspberry_classic_40pin_definition(
    id: &str,
    family: &str,
    keyboard_orientation: bool,
    pi5_functions: bool,
) -> (DeviceDefinition, Footprint) {
    let source = DataSource {
        project: "Raspberry Pi official computer documentation".into(),
        revision: format!("{family} 40-pin GPIO header"),
        url: "https://www.raspberrypi.com/documentation/computers/raspberry-pi.html".into(),
        license: "Factual connector pin names; original ProtoCraft symbol and footprint geometry"
            .into(),
    };
    let signals: Vec<&str> = if pi5_functions {
        RASPBERRY_PI_5_PIN_SIGNALS.to_vec()
    } else {
        RASPBERRY_PI_40_PIN_SIGNALS.to_vec()
    };
    let pins = raspberry_device_pins(
        &signals,
        RaspberryPinLayout::Alternating {
            pin_count: 40,
            flipped: false,
        },
    );
    let footprint = raspberry_gpio_footprint(
        if keyboard_orientation {
            "ProtoCraft_RaspberryPi:GPIO_2x20_2.54mm_KeyboardInverted"
        } else {
            "ProtoCraft_RaspberryPi:GPIO_2x20_2.54mm"
        },
        if keyboard_orientation {
            "Header GPIO 2×20, orientação física invertida 180° dos computadores de teclado"
        } else {
            "Header GPIO 2×20, passo 2,54 mm; verificar altura e padrão do conector"
        },
        40,
        RaspberryPinLayout::Alternating {
            pin_count: 40,
            flipped: keyboard_orientation,
        },
        source.clone(),
    );
    let device = raspberry_device(
        id,
        family,
        "Header GPIO 2×20",
        "Pinagem elétrica do header; GPIO usa lógica de 3,3 V e não é tolerante a 5 V. Não emula CPU ou firmware.",
        "https://www.raspberrypi.com/documentation/computers/raspberry-pi.html",
        pins,
        source,
    );
    (device, footprint)
}

fn raspberry_26pin_definition(
    id: &str,
    family: &str,
    signals: &[&str; 26],
    url: &str,
) -> (DeviceDefinition, Footprint) {
    let source = DataSource {
        project: "Raspberry Pi official historical board documentation".into(),
        revision: family.into(),
        url: url.into(),
        license: "Factual connector pin names; original ProtoCraft symbol and footprint geometry"
            .into(),
    };
    let pins = raspberry_device_pins(
        signals,
        RaspberryPinLayout::Alternating {
            pin_count: 26,
            flipped: false,
        },
    );
    let footprint = raspberry_gpio_footprint(
        "ProtoCraft_RaspberryPi:GPIO_2x13_2.54mm",
        "Header GPIO legado 2×13, passo 2,54 mm",
        26,
        RaspberryPinLayout::Alternating {
            pin_count: 26,
            flipped: false,
        },
        source.clone(),
    );
    let device = raspberry_device(
        id,
        family,
        "Header GPIO legado 2×13",
        "Pinagem de revisão documentada; a revisão 1.0 e a 2.0 diferem nos pinos 3, 5 e 13. GPIO usa lógica de 3,3 V e não é tolerante a 5 V.",
        url,
        pins,
        source,
    );
    (device, footprint)
}

fn raspberry_pico_definition(
    id: &str,
    family: &str,
    wireless: bool,
) -> (DeviceDefinition, Footprint) {
    let source = DataSource {
        project: "Raspberry Pi official Pico-series documentation".into(),
        revision: format!("{family} GPIO header"),
        url: "https://www.raspberrypi.com/documentation/microcontrollers/pico-series.html".into(),
        license: "Factual connector pin names; original ProtoCraft symbol and footprint geometry"
            .into(),
    };
    let pins = raspberry_device_pins(&RASPBERRY_PICO_PIN_SIGNALS, RaspberryPinLayout::Pico);
    let footprint = raspberry_gpio_footprint(
        "ProtoCraft_RaspberryPi:Pico_GPIO_2x20_2.54mm_17.78mmRows",
        "Pinagem Pico 2×20, passo 2,54 mm e distância entre fileiras 17,78 mm",
        40,
        RaspberryPinLayout::Pico,
        source.clone(),
    );
    let description = if wireless {
        "Pinagem física compartilhada da família Pico W; GPIO usa lógica de 3,3 V. Recursos internos de Wi-Fi ocupam alguns GPIOs no módulo. Sem simulação de firmware."
    } else {
        "Pinagem física da família Pico; GPIO usa lógica de 3,3 V e não é tolerante a 5 V. Sem simulação de firmware."
    };
    let device = raspberry_device(
        id,
        family,
        "Pinagem de placa / GPIO 2×20",
        description,
        "https://www.raspberrypi.com/documentation/microcontrollers/pico-series.html",
        pins,
        source,
    );
    (device, footprint)
}

fn raspberry_device(
    id: &str,
    family: &str,
    form: &str,
    description: &str,
    datasheet: &str,
    pins: Vec<DevicePin>,
    source: DataSource,
) -> DeviceDefinition {
    DeviceDefinition {
        id: id.into(),
        requested_part: id.into(),
        manufacturer: "Raspberry Pi Ltd".into(),
        family: family.into(),
        form: form.into(),
        description: description.into(),
        datasheet: datasheet.into(),
        pins,
        source,
    }
}

fn raspberry_device_pins(signals: &[&str], layout: RaspberryPinLayout) -> Vec<DevicePin> {
    signals
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let (column, row, first_y) = match layout {
                RaspberryPinLayout::Alternating { pin_count, .. } => {
                    (index % 2, index / 2, 1 - pin_count as i32 / 2)
                }
                RaspberryPinLayout::Pico if index < 20 => (0, index, -19),
                RaspberryPinLayout::Pico => (1, 39 - index, -19),
            };
            let electrical_type = match *name {
                "GND" | "GND/AGND" => "passive",
                "3V3(OUT)" => "power_out",
                "3V3" | "5V" | "3V3_EN" | "VSYS" | "VBUS" | "ADC_VREF" => "power_in",
                _ => "bidirectional",
            };
            let x = if column == 0 { -3 } else { 3 };
            DevicePin {
                number: (index + 1).to_string(),
                name: (*name).into(),
                electrical_type: electrical_type.into(),
                offset: Point::new(x, first_y + row as i32 * 2),
                pads: vec![(index + 1).to_string()],
            }
        })
        .collect()
}

fn raspberry_gpio_footprint(
    id: &str,
    description: &str,
    pin_count: usize,
    layout: RaspberryPinLayout,
    source: DataSource,
) -> Footprint {
    let pads: Vec<Pad> = (0..pin_count)
        .map(|index| {
            let (mut x, mut y) = match layout {
                RaspberryPinLayout::Alternating { .. } => (
                    (index / 2) as i64 * 2_540_000,
                    (index % 2) as i64 * 2_540_000,
                ),
                RaspberryPinLayout::Pico if index < 20 => (0, index as i64 * 2_540_000),
                RaspberryPinLayout::Pico => (17_780_000, (39 - index) as i64 * 2_540_000),
            };
            if let RaspberryPinLayout::Alternating {
                pin_count: layout_pin_count,
                flipped: true,
            } = layout
            {
                let last_x = (layout_pin_count as i64 / 2 - 1) * 2_540_000;
                x = last_x - x;
                y = 2_540_000 - y;
            }
            Pad {
                number: (index + 1).to_string(),
                position: NmPoint::new(x, y),
                size: NmPoint::new(1_800_000, 1_800_000),
                rotation_mdeg: 0,
                shape: PadShape::Circle,
                technology: PadTechnology::ThroughHole,
                drill: Some(NmPoint::new(1_000_000, 1_000_000)),
                layers: vec!["*.Cu".into(), "F.Mask".into()],
                roundrect_ratio_ppm: 0,
                mask_margin_nm: 0,
                paste_margin_nm: 0,
                paste_ratio_ppm: 0,
                polygons: vec![],
            }
        })
        .collect();
    let max_x = pads.iter().map(|pad| pad.position.x).max().unwrap_or(0);
    let max_y = pads.iter().map(|pad| pad.position.y).max().unwrap_or(0);
    let min_x = pads.iter().map(|pad| pad.position.x).min().unwrap_or(0);
    let min_y = pads.iter().map(|pad| pad.position.y).min().unwrap_or(0);
    let min = NmPoint::new(min_x - 1_000_000, min_y - 1_000_000);
    let max = NmPoint::new(max_x + 1_000_000, max_y + 1_000_000);
    Footprint {
        id: id.into(),
        description: description.into(),
        body_layer: "F.Fab".into(),
        body_bounds: NmBounds { min, max },
        pads,
        lines: vec![
            FootprintLine {
                start: NmPoint::new(min.x, min.y),
                end: NmPoint::new(max.x, min.y),
                width_nm: 100_000,
                layer: "F.Fab".into(),
            },
            FootprintLine {
                start: NmPoint::new(max.x, min.y),
                end: NmPoint::new(max.x, max.y),
                width_nm: 100_000,
                layer: "F.Fab".into(),
            },
            FootprintLine {
                start: NmPoint::new(max.x, max.y),
                end: NmPoint::new(min.x, max.y),
                width_nm: 100_000,
                layer: "F.Fab".into(),
            },
            FootprintLine {
                start: NmPoint::new(min.x, max.y),
                end: NmPoint::new(min.x, min.y),
                width_nm: 100_000,
                layer: "F.Fab".into(),
            },
        ],
        keepouts: vec![],
        source,
    }
}
impl Catalog {
    pub fn search(&self, query: &str) -> Vec<&DeviceRecord> {
        if !query.contains(char::is_whitespace)
            && let Some(record) = self
                .devices
                .iter()
                .find(|record| record.id.eq_ignore_ascii_case(query))
        {
            return vec![record];
        }
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
            .map(expand_query_aliases);

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
        let id = if id.eq_ignore_ascii_case("Raspberry Pi 4")
            || id.eq_ignore_ascii_case("Raspberry Pi 4 Model B")
        {
            "RaspberryPi4B_GPIO"
        } else {
            id
        };
        if let Some(record) = self.devices.iter().find(|d| d.id.eq_ignore_ascii_case(id)) {
            return Ok(record);
        }
        let family_query = normalize_search_text(id);
        if let Some(record) = self.devices.iter().find(|record| {
            record.manufacturer == "Raspberry Pi Ltd"
                && normalize_search_text(&record.family) == family_query
        }) {
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

fn search_term_aliases(term: &str) -> Vec<&str> {
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
