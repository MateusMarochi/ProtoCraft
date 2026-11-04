//! Geometria de fabricação em nanômetros, distinta da grade do esquemático.
use crate::{Component, ComponentId, ComponentKind, CoreError, PinRef, Point, Project};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const NM_PER_MM: i64 = 1_000_000;
pub const PHYSICAL_LIMIT: i64 = 2_000 * NM_PER_MM;
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NmPoint {
    pub x: i64,
    pub y: i64,
}
impl NmPoint {
    pub const fn new(x: i64, y: i64) -> Self {
        Self { x, y }
    }
    pub fn valid(self) -> bool {
        self.x.abs_diff(0) <= PHYSICAL_LIMIT as u64 && self.y.abs_diff(0) <= PHYSICAL_LIMIT as u64
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NmBounds {
    pub min: NmPoint,
    pub max: NmPoint,
}
impl NmBounds {
    pub fn valid(self) -> bool {
        self.min.valid() && self.max.valid() && self.min.x < self.max.x && self.min.y < self.max.y
    }
    pub fn width(self) -> i64 {
        self.max.x - self.min.x
    }
    pub fn height(self) -> i64 {
        self.max.y - self.min.y
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataSource {
    pub project: String,
    pub revision: String,
    pub url: String,
    pub license: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DevicePin {
    pub number: String,
    pub name: String,
    pub electrical_type: String,
    pub offset: Point,
    pub pads: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceDefinition {
    pub id: String,
    #[serde(default)]
    pub requested_part: String,
    pub manufacturer: String,
    pub family: String,
    pub form: String,
    pub description: String,
    pub datasheet: String,
    pub pins: Vec<DevicePin>,
    pub source: DataSource,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PadShape {
    Rect,
    RoundRect,
    Circle,
    Oval,
    Custom,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PadTechnology {
    Smd,
    ThroughHole,
    NonPlated,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pad {
    pub number: String,
    pub position: NmPoint,
    pub size: NmPoint,
    pub rotation_mdeg: i32,
    pub shape: PadShape,
    pub technology: PadTechnology,
    pub drill: Option<NmPoint>,
    pub layers: Vec<String>,
    pub roundrect_ratio_ppm: u32,
    #[serde(default)]
    pub mask_margin_nm: i64,
    #[serde(default)]
    pub paste_margin_nm: i64,
    #[serde(default)]
    pub paste_ratio_ppm: i32,
    #[serde(default)]
    pub polygons: Vec<Vec<NmPoint>>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FootprintLine {
    pub start: NmPoint,
    pub end: NmPoint,
    pub width_nm: i64,
    pub layer: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Keepout {
    pub polygon: Vec<NmPoint>,
    pub tracks: bool,
    pub vias: bool,
    pub pads: bool,
    pub copper: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Footprint {
    pub id: String,
    pub description: String,
    pub body_layer: String,
    pub body_bounds: NmBounds,
    pub pads: Vec<Pad>,
    pub lines: Vec<FootprintLine>,
    pub keepouts: Vec<Keepout>,
    pub source: DataSource,
}
fn invalid(message: &str) -> CoreError {
    CoreError::Invalid(message.into())
}
fn text(s: &str, max: usize) -> bool {
    s.len() <= max && !s.chars().any(char::is_control)
}
impl Footprint {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.id.is_empty()
            || !text(&self.id, 256)
            || !text(&self.description, 4096)
            || !self.body_bounds.valid()
            || self.pads.is_empty()
            || self.pads.len() > 4096
            || self.lines.len() > 10000
            || self.keepouts.len() > 256
        {
            return Err(invalid("Footprint inválido ou acima dos limites."));
        }
        for pad in &self.pads {
            if !text(&pad.number, 32)
                || !pad.position.valid()
                || !pad.size.valid()
                || pad.size.x <= 0
                || pad.size.y <= 0
                || pad.rotation_mdeg.abs_diff(0) > 360000
                || pad.layers.is_empty()
                || pad.layers.len() > 16
                || pad.layers.iter().any(|l| !text(l, 32))
                || pad.mask_margin_nm.abs_diff(0) > 1_000_000
                || pad.paste_margin_nm.abs_diff(0) > 1_000_000
                || pad.paste_ratio_ppm.abs_diff(0) > 1_000_000
                || pad.roundrect_ratio_ppm > 500000
                || pad.polygons.len() > 256
                || pad
                    .polygons
                    .iter()
                    .any(|p| p.len() < 3 || p.len() > 4096 || p.iter().any(|p| !p.valid()))
                || (pad.shape == PadShape::Custom && pad.polygons.is_empty())
                || pad.drill.is_some_and(|d| {
                    !d.valid() || d.x <= 0 || d.y <= 0 || d.x > pad.size.x || d.y > pad.size.y
                })
                || (pad.technology != PadTechnology::Smd && pad.drill.is_none())
            {
                return Err(invalid(
                    "Pad inválido: confira número, tamanho, camadas e furo.",
                ));
            }
        }
        if self.lines.iter().any(|l| {
            !l.start.valid()
                || !l.end.valid()
                || l.width_nm <= 0
                || l.width_nm > NM_PER_MM * 10
                || !text(&l.layer, 32)
        }) || self.keepouts.iter().any(|k| {
            k.polygon.len() < 3 || k.polygon.len() > 4096 || k.polygon.iter().any(|p| !p.valid())
        }) {
            return Err(invalid("Contorno ou área de exclusão inválida."));
        }
        validate_source(&self.source)
    }
}
fn validate_source(s: &DataSource) -> Result<(), CoreError> {
    if s.project.is_empty()
        || !text(&s.project, 256)
        || !text(&s.revision, 128)
        || !text(&s.url, 2048)
        || !text(&s.license, 256)
    {
        return Err(invalid("Origem de biblioteca inválida."));
    }
    Ok(())
}
pub(crate) fn validate_component(c: &Component) -> Result<(), CoreError> {
    if (c.kind == ComponentKind::Device) != c.device.is_some()
        || (c.device.is_some() && c.imported.is_some())
    {
        return Err(invalid(
            "Dispositivo de biblioteca sem definição ou com origem conflitante.",
        ));
    }
    if let Some(f) = &c.footprint {
        f.validate()?;
        if c.device.is_none() {
            let numbers: BTreeSet<_> = c
                .pins()
                .iter()
                .map(|p| c.pin_number(p.reference.pin))
                .collect();
            let pads: BTreeSet<_> = f
                .pads
                .iter()
                .filter(|p| p.technology != PadTechnology::NonPlated && !p.number.is_empty())
                .map(|p| p.number.clone())
                .collect();
            if numbers != pads {
                return Err(invalid(
                    "Footprint incompatível com a numeração dos pinos do componente.",
                ));
            }
        }
    }
    if let Some(d) = &c.device {
        if !text(&d.requested_part, 256)
            || d.id.is_empty()
            || !text(&d.id, 256)
            || !text(&d.manufacturer, 128)
            || !text(&d.family, 128)
            || !text(&d.form, 64)
            || !text(&d.description, 4096)
            || !text(&d.datasheet, 2048)
            || d.pins.is_empty()
            || d.pins.len() > 1024
        {
            return Err(invalid("Definição de dispositivo inválida."));
        }
        validate_source(&d.source)?;
        let f = c
            .footprint
            .as_ref()
            .ok_or_else(|| invalid("Dispositivo sem footprint."))?;
        let pads: BTreeSet<_> = f
            .pads
            .iter()
            .filter(|p| p.technology != PadTechnology::NonPlated && !p.number.is_empty())
            .map(|p| p.number.as_str())
            .collect();
        let mut numbers = BTreeSet::new();
        let mut mapped = BTreeSet::new();
        for pin in &d.pins {
            if pin.number.is_empty()
                || !text(&pin.number, 32)
                || !text(&pin.name, 128)
                || !text(&pin.electrical_type, 32)
                || !pin.offset.is_valid()
                || !numbers.insert(&pin.number)
                || pin.pads.is_empty()
                || pin.pads.len() > 64
                || pin
                    .pads
                    .iter()
                    .any(|p| !pads.contains(p.as_str()) || !mapped.insert(p.as_str()))
            {
                return Err(invalid(
                    "Mapeamento pino → pad incompleto, duplicado ou inválido.",
                ));
            }
        }
        if mapped != pads {
            return Err(invalid(
                "Footprint contém pads elétricos sem pino correspondente.",
            ));
        }
    }
    Ok(())
}
impl Component {
    pub fn is_electrical_pin(&self, pin: u16) -> bool {
        if let Some(d) = &self.device {
            return d
                .pins
                .get(pin as usize)
                .is_some_and(|p| p.electrical_type != "no_connect");
        }
        (pin as usize) < self.pins().len()
    }

    pub fn pin_name(&self, pin: u16) -> &str {
        if let Some(d) = &self.device {
            return d.pins.get(pin as usize).map_or("?", |p| p.name.as_str());
        }
        if let Some(d) = &self.imported {
            return d.pins.get(pin as usize).map_or("?", |p| p.name.as_str());
        }
        if pin < u16::from(self.kind.pin_count()) {
            self.kind.pin_name(pin as u8)
        } else {
            "?"
        }
    }
    pub fn pin_number(&self, pin: u16) -> String {
        if let Some(d) = &self.device {
            return d
                .pins
                .get(pin as usize)
                .map_or_else(|| "?".into(), |p| p.number.clone());
        }
        if let Some(d) = &self.imported {
            return d
                .pins
                .get(pin as usize)
                .map_or_else(|| "?".into(), |p| p.number.clone());
        }
        (pin + 1).to_string()
    }
    pub fn pads_for_pin(&self, pin: u16) -> Vec<String> {
        self.device
            .as_ref()
            .and_then(|d| d.pins.get(pin as usize))
            .map_or_else(|| vec![self.pin_number(pin)], |p| p.pads.clone())
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoardSide {
    #[default]
    Top,
    Bottom,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    pub position: NmPoint,
    pub rotation: u8,
    #[serde(default)]
    pub side: BoardSide,
}
impl Placement {
    pub fn transform(&self, mut p: NmPoint) -> NmPoint {
        if self.side == BoardSide::Bottom {
            p.x = -p.x;
        }
        for _ in 0..self.rotation {
            p = NmPoint::new(-p.y, p.x);
        }
        NmPoint::new(self.position.x + p.x, self.position.y + p.y)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Track {
    pub id: u64,
    pub start: NmPoint,
    pub end: NmPoint,
    pub width_nm: i64,
    pub layer: BoardSide,
    pub net: PinRef,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Via {
    pub id: u64,
    pub position: NmPoint,
    pub diameter_nm: i64,
    pub drill_nm: i64,
    pub net: PinRef,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MountingHole {
    pub id: u64,
    pub position: NmPoint,
    pub drill_nm: i64,
}

/// Restrições elétricas e geométricas aplicadas a redes nomeadas da PCB.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PadEscapeRule {
    pub track_width_nm: i64,
    pub max_length_nm: i64,
    pub via_diameter_nm: i64,
    pub via_drill_nm: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetClass {
    /// Nomes exatos das redes do netlist; a comparação respeita maiúsculas/minúsculas.
    pub net_names: BTreeSet<String>,
    pub clearance_nm: i64,
    pub track_width_nm: i64,
    pub via_diameter_nm: i64,
    pub via_drill_nm: i64,
    /// Exceção explícita para o trecho curto entre um pad SMD e seu escape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pad_escape: Option<PadEscapeRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Board {
    pub width_nm: i64,
    pub height_nm: i64,
    /// Raio uniforme dos quatro cantos do contorno retangular arredondado.
    #[serde(default)]
    pub corner_radius_nm: i64,
    pub thickness_nm: i64,
    pub clearance_nm: i64,
    pub edge_clearance_nm: i64,
    pub placements: BTreeMap<ComponentId, Placement>,
    pub tracks: BTreeMap<u64, Track>,
    pub vias: BTreeMap<u64, Via>,
    #[serde(default)]
    pub holes: BTreeMap<u64, MountingHole>,
    /// Regras por rede; redes sem classe herdam os valores globais da placa.
    #[serde(default)]
    pub net_classes: BTreeMap<String, NetClass>,
}
impl Default for Board {
    fn default() -> Self {
        Self {
            width_nm: 100 * NM_PER_MM,
            height_nm: 80 * NM_PER_MM,
            corner_radius_nm: 0,
            thickness_nm: 1_600_000,
            clearance_nm: 200_000,
            edge_clearance_nm: 250_000,
            placements: BTreeMap::new(),
            tracks: BTreeMap::new(),
            vias: BTreeMap::new(),
            holes: BTreeMap::new(),
            net_classes: BTreeMap::new(),
        }
    }
}
impl Board {
    /// Verifica se um ponto respeita o contorno arredondado e uma margem interna.
    pub fn contains_with_clearance(&self, point: NmPoint, clearance_nm: i64) -> bool {
        if clearance_nm < 0 {
            return false;
        }
        let left = clearance_nm;
        let top = clearance_nm;
        let right = self.width_nm.saturating_sub(clearance_nm);
        let bottom = self.height_nm.saturating_sub(clearance_nm);
        if point.x < left || point.x > right || point.y < top || point.y > bottom {
            return false;
        }
        let radius = self.corner_radius_nm.saturating_sub(clearance_nm).max(0);
        if radius == 0 {
            return true;
        }
        let cx = if point.x < left + radius {
            left + radius
        } else if point.x > right - radius {
            right - radius
        } else {
            point.x
        };
        let cy = if point.y < top + radius {
            top + radius
        } else if point.y > bottom - radius {
            bottom - radius
        } else {
            point.y
        };
        let dx = i128::from(point.x - cx);
        let dy = i128::from(point.y - cy);
        dx * dx + dy * dy <= i128::from(radius) * i128::from(radius)
    }

    pub fn validate(&self, project: &Project) -> Result<(), CoreError> {
        if !(NM_PER_MM..=PHYSICAL_LIMIT).contains(&self.width_nm)
            || !(NM_PER_MM..=PHYSICAL_LIMIT).contains(&self.height_nm)
            || !(0..=self.width_nm.min(self.height_nm) / 2).contains(&self.corner_radius_nm)
            || !(100000..=10000000).contains(&self.thickness_nm)
            || !(0..=10 * NM_PER_MM).contains(&self.clearance_nm)
            || !(0..=10 * NM_PER_MM).contains(&self.edge_clearance_nm)
            || self.tracks.len() + self.vias.len() + self.holes.len() > 20000
            || self.net_classes.len() > 64
        {
            return Err(invalid("Dimensões ou regras da PCB inválidas."));
        }
        let mut assigned_nets = BTreeSet::new();
        for (name, rules) in &self.net_classes {
            if name.trim().is_empty()
                || !text(name, 64)
                || rules.net_names.is_empty()
                || rules.net_names.len() > 256
                || !(0..=10 * NM_PER_MM).contains(&rules.clearance_nm)
                || !(50_000..=10 * NM_PER_MM).contains(&rules.track_width_nm)
                || !(300_000..=20 * NM_PER_MM).contains(&rules.via_diameter_nm)
                || !(100_000..rules.via_diameter_nm).contains(&rules.via_drill_nm)
                || rules.via_diameter_nm - rules.via_drill_nm < 200_000
                || rules.pad_escape.as_ref().is_some_and(|escape| {
                    !(50_000..=rules.track_width_nm).contains(&escape.track_width_nm)
                        || !(50_000..=2 * NM_PER_MM).contains(&escape.max_length_nm)
                        || !(300_000..=rules.via_diameter_nm).contains(&escape.via_diameter_nm)
                        || !(100_000..=rules.via_drill_nm).contains(&escape.via_drill_nm)
                        || escape.via_diameter_nm - escape.via_drill_nm < 200_000
                })
                || rules.net_names.iter().any(|net| {
                    net.trim().is_empty() || !text(net, 64) || !assigned_nets.insert(net.clone())
                })
            {
                return Err(invalid(
                    "Classe de rede inválida: confira nome, redes exclusivas, largura, folga e dimensões de via.",
                ));
            }
        }
        for (id, p) in &self.placements {
            if !p.position.valid()
                || p.rotation > 3
                || project
                    .components
                    .get(id)
                    .is_none_or(|c| c.footprint.is_none())
            {
                return Err(invalid(
                    "Posicionamento PCB sem componente/footprint válido.",
                ));
            }
        }
        let pin_valid = |p: PinRef| {
            project.components.get(&p.component).is_some_and(|c| {
                c.is_electrical_pin(p.pin) && self.placements.contains_key(&p.component)
            })
        };
        for (id, t) in &self.tracks {
            if *id != t.id
                || *id == 0
                || !t.start.valid()
                || !t.end.valid()
                || t.start == t.end
                || !(1000..=10 * NM_PER_MM).contains(&t.width_nm)
                || !pin_valid(t.net)
            {
                return Err(invalid("Trilha inválida ou sem rede de referência."));
            }
        }
        for (id, v) in &self.vias {
            if *id != v.id
                || *id == 0
                || !v.position.valid()
                || !(1000..=10 * NM_PER_MM).contains(&v.drill_nm)
                || v.diameter_nm <= v.drill_nm
                || v.diameter_nm > 20 * NM_PER_MM
                || !pin_valid(v.net)
            {
                return Err(invalid("Via inválida ou sem rede de referência."));
            }
        }
        for (id, h) in &self.holes {
            if *id == 0
                || *id != h.id
                || !h.position.valid()
                || !(100000..=50 * NM_PER_MM).contains(&h.drill_nm)
            {
                return Err(invalid(
                    "Furo mecânico inválido: diâmetro entre 0,1 e 50 mm.",
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod net_class_tests {
    use super::*;

    fn class(name: &str) -> NetClass {
        NetClass {
            net_names: BTreeSet::from([name.to_owned()]),
            clearance_nm: 200_000,
            track_width_nm: 250_000,
            via_diameter_nm: 600_000,
            via_drill_nm: 300_000,
            pad_escape: None,
        }
    }

    #[test]
    fn pad_escape_round_trip_and_validation_preserve_legacy_classes() {
        let old = class("VCC");
        let json = serde_json::to_string(&old).unwrap();
        assert!(!json.contains("pad_escape"));
        assert_eq!(serde_json::from_str::<NetClass>(&json).unwrap(), old);
        let mut project = Project::empty("escape");
        let mut board = Board::default();
        let mut rules = old;
        rules.pad_escape = Some(PadEscapeRule {
            track_width_nm: 200_000,
            max_length_nm: 1_000_000,
            via_diameter_nm: 400_000,
            via_drill_nm: 200_000,
        });
        board.net_classes.insert("POWER".into(), rules.clone());
        project.board = Some(Box::new(board));
        project.validate().unwrap();
        assert_eq!(
            serde_json::from_str::<NetClass>(&serde_json::to_string(&rules).unwrap()).unwrap(),
            rules
        );
        project
            .board
            .as_mut()
            .unwrap()
            .net_classes
            .get_mut("POWER")
            .unwrap()
            .pad_escape
            .as_mut()
            .unwrap()
            .max_length_nm = 3 * NM_PER_MM;
        assert!(project.validate().is_err());
    }

    #[test]
    fn board_rejects_a_net_assigned_to_two_classes() {
        let mut project = Project::empty("classes");
        let mut board = Board::default();
        board.net_classes.insert("POWER".into(), class("VCC"));
        board.net_classes.insert("SIGNAL".into(), class("VCC"));
        project.board = Some(Box::new(board));
        assert!(project.validate().is_err());
    }

    #[test]
    fn board_rejects_via_rules_with_insufficient_annular_ring() {
        let mut project = Project::empty("classes");
        let mut invalid = class("VCC");
        invalid.via_diameter_nm = 600_000;
        invalid.via_drill_nm = 450_000;
        let mut board = Board::default();
        board.net_classes.insert("POWER".into(), invalid);
        project.board = Some(Box::new(board));
        assert!(project.validate().is_err());
    }
}
