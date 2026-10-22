//! Leitura independente do subconjunto ISIS 8 observado em projetos de exemplo.
//! As posições elétricas usam os marcadores de nó das bibliotecas embutidas.

use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DsnPoint {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DsnPin {
    pub name: String,
    pub number: String,
    pub position: DsnPoint,
}

#[derive(Debug, Clone)]
pub struct DsnComponent {
    pub offset: usize,
    pub source_id: u32,
    pub reference: String,
    pub value: String,
    pub symbol_name: String,
    pub position: DsnPoint,
    pub angle_tenths: i16,
    pub mirror_flags: u16,
    pub properties_raw: String,
    pub pins: Vec<DsnPin>,
    pub pins_complete: bool,
}

#[derive(Debug, Clone)]
pub struct DsnWire {
    pub offset: usize,
    pub points: Vec<DsnPoint>,
}

#[derive(Debug, Clone)]
pub struct DsnLabel {
    pub name: String,
    pub position: DsnPoint,
    pub is_ground: bool,
}

#[derive(Debug, Clone, Default)]
pub struct DsnDocument {
    pub components: Vec<DsnComponent>,
    pub wires: Vec<DsnWire>,
    pub junctions: Vec<DsnPoint>,
    pub labels: Vec<DsnLabel>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DsnError {
    pub offset: usize,
    pub message: String,
}

impl fmt::Display for DsnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DSN no deslocamento 0x{:X}: {}",
            self.offset, self.message
        )
    }
}
impl std::error::Error for DsnError {}

fn error(offset: usize, message: &str) -> DsnError {
    DsnError {
        offset,
        message: message.to_owned(),
    }
}

fn occurrences<'a>(data: &'a [u8], needle: &'a [u8]) -> impl Iterator<Item = usize> + 'a {
    data.windows(needle.len())
        .enumerate()
        .filter_map(move |(i, v)| (v == needle).then_some(i))
}
fn word(data: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        data.get(offset..offset.checked_add(2)?)?.try_into().ok()?,
    ))
}
fn dword(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        data.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}
fn point(data: &[u8], offset: usize) -> Option<DsnPoint> {
    Some(DsnPoint {
        x: dword(data, offset)? as i32,
        y: dword(data, offset + 4)? as i32,
    })
}
fn cstring(data: &[u8], offset: usize) -> Option<(&[u8], usize)> {
    let end = data.get(offset..)?.iter().take(256).position(|b| *b == 0)? + offset;
    Some((&data[offset..end], end + 1))
}
fn text(bytes: &[u8]) -> String {
    // Os exemplos usam ASCII. Latin-1 preserva cada byte de nomes legados.
    bytes.iter().map(|b| char::from(*b)).collect()
}
fn printable(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .all(|b| *b >= 32 || matches!(*b, b'\n' | b'\r' | b'\t'))
}

/// Espelhos precedem a rotação; apenas quartos de volta têm suporte exato.
fn transform(p: DsnPoint, origin: DsnPoint, angle: i16, flags: u16) -> Option<DsnPoint> {
    if angle % 900 != 0 || flags & !3 != 0 {
        return None;
    }
    let x = i64::from(p.x) * if flags & 1 == 0 { 1 } else { -1 };
    let y = i64::from(p.y) * if flags & 2 == 0 { 1 } else { -1 };
    let (x, y) = match (angle / 900).rem_euclid(4) {
        0 => (x, y),
        1 => (-y, x),
        2 => (-x, -y),
        _ => (y, -x),
    };
    Some(DsnPoint {
        x: i32::try_from(x + i64::from(origin.x)).ok()?,
        y: i32::try_from(y + i64::from(origin.y)).ok()?,
    })
}

/// Localiza um bloco com nome contado e tamanho explícito; o tamanho inclui seu
/// próprio u32. Não utiliza a posição de strings soltas como definição válida.
fn library<'a>(data: &'a [u8], name: &str, budget: &mut usize) -> Option<&'a [u8]> {
    if !reserve_scan(budget, data.len()) {
        return None;
    }
    for off in occurrences(data, name.as_bytes()) {
        if off == 0 || usize::from(data[off - 1]) != name.len() {
            continue;
        }
        let size_pos = off + name.len() + 4;
        let Some(size) = dword(data, size_pos).map(|v| v as usize) else {
            continue;
        };
        if size < 16 {
            continue;
        }
        let Some(end) = size_pos.checked_add(size) else {
            continue;
        };
        let Some(blob) = data.get(size_pos + 4..end) else {
            continue;
        };
        if !reserve_scan(budget, blob.len()) {
            return None;
        }
        if occurrences(blob, b"$MKRORIGIN\0").next().is_some() {
            return Some(blob);
        }
    }
    None
}

fn reserve_scan(budget: &mut usize, bytes: usize) -> bool {
    match budget.checked_sub(bytes) {
        Some(left) => {
            *budget = left;
            true
        }
        None => {
            *budget = 0;
            false
        }
    }
}

/// O orçamento considera cada região efetivamente examinada, inclusive blocos
/// candidatos sobrepostos. O cache também guarda buscas que não encontraram nada.
struct Libraries<'a> {
    data: &'a [u8],
    budget: usize,
    cache: BTreeMap<String, Option<&'a [u8]>>,
    exhausted: bool,
}

impl<'a> Libraries<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            budget: 64 * 1024 * 1024,
            cache: BTreeMap::new(),
            exhausted: false,
        }
    }

    fn get(&mut self, name: &str) -> Option<&'a [u8]> {
        if let Some(cached) = self.cache.get(name) {
            return *cached;
        }
        if self.cache.len() >= 128 || self.budget < self.data.len() {
            self.exhausted = true;
            return None;
        }
        let found = library(self.data, name, &mut self.budget);
        // O contador zera ao falhar uma reserva, incluindo tentativas de blocos
        // sobrepostos que não podem ser resolvidas dentro do orçamento restante.
        if self.budget == 0 {
            self.exhausted = true;
        }
        self.cache.insert(name.to_owned(), found);
        found
    }
}
fn marker_point(data: &[u8], marker: &[u8]) -> Option<DsnPoint> {
    let offset = occurrences(data, marker).next()?;
    point(data, offset.checked_sub(12)?)
}

fn library_pins(libraries: &mut Libraries<'_>, name: &str) -> (Vec<DsnPin>, Vec<String>) {
    let mut pins = Vec::new();
    let mut warnings = Vec::new();
    let Some(blob) = libraries.get(name) else {
        return (
            pins,
            vec![format!("Biblioteca embutida de {name} não localizada.")],
        );
    };
    let Some(symbol_origin) = marker_point(blob, b"$MKRORIGIN\0") else {
        return (pins, vec![format!("Origem de {name} ausente ou truncada.")]);
    };
    for (index, off) in occurrences(blob, b"$PIN").enumerate() {
        if index >= 64 {
            warnings.push(format!("Símbolo {name} excede o limite de 64 pinos."));
            pins.clear();
            break;
        }
        let Some((template, after_template)) = cstring(blob, off) else {
            warnings.push(format!("Template de pino truncado em {name}, 0x{off:X}."));
            continue;
        };
        if !printable(template) || template.len() > 64 {
            warnings.push(format!("Template de pino inválido em {name}, 0x{off:X}."));
            continue;
        }
        let template_name = text(template);
        let decoded = (|| {
            let (pin_name, after_name) = cstring(blob, after_template)?;
            let (pin_number, _) = cstring(blob, after_name)?;
            if pin_name.len() > 80
                || pin_number.len() > 80
                || !printable(pin_name)
                || !printable(pin_number)
            {
                return None;
            }
            let template_blob = libraries.get(&template_name)?;
            let node = marker_point(template_blob, b"$MKRNODE\0")?;
            let template_origin = marker_point(template_blob, b"$MKRORIGIN\0")?;
            let relative = DsnPoint {
                x: node.x.checked_sub(template_origin.x)?,
                y: node.y.checked_sub(template_origin.y)?,
            };
            let position = point(blob, off.checked_sub(12)?)?;
            let angle = word(blob, off.checked_sub(4)?)? as i16;
            let flags = word(blob, off.checked_sub(2)?)?;
            let anchor = transform(relative, position, angle, flags)?;
            Some(DsnPin {
                name: text(pin_name),
                number: if pin_number.is_empty() {
                    text(pin_name)
                } else {
                    text(pin_number)
                },
                position: DsnPoint {
                    x: anchor.x.checked_sub(symbol_origin.x)?,
                    y: anchor.y.checked_sub(symbol_origin.y)?,
                },
            })
        })();
        match decoded {
            Some(pin) => pins.push(pin),
            None => warnings.push(format!(
                "Pino {template_name} de {name} em 0x{off:X} não recuperado."
            )),
        }
    }
    if pins.is_empty() {
        warnings.push(format!("Nenhum pino recuperado para {name}."));
    }
    (pins, warnings)
}

/// O texto de atributo precede sua posição/estilo e o nome da fonte. A distância
/// de 34 bytes é validada pelos marcadores FF e pelo limite do texto contado.
fn attribute_text(data: &[u8], label: usize, lower_bound: usize) -> Option<String> {
    if label > data.len() || lower_bound > label {
        return None;
    }
    let start = label.saturating_sub(512).max(lower_bound);
    let font_tag = (start..label).rev().find(|i| data[*i] == 0xff)?;
    let font_start = font_tag.checked_add(2)?;
    let (font, font_end) = cstring(data, font_start)?;
    if font_end != label || !printable(font) {
        return None;
    }
    let after_text = font_start.checked_sub(34)?;
    for off in (after_text.saturating_sub(257).max(lower_bound)..after_text).rev() {
        if data[off] != 0xff {
            continue;
        }
        let len = usize::from(*data.get(off + 1)?);
        if off + 2 + len == after_text && printable(&data[off + 2..after_text]) {
            return Some(text(&data[off + 2..after_text]));
        }
    }
    None
}

fn component_header(data: &[u8], off: usize) -> Option<(usize, String)> {
    if !matches!(word(data, off)?, 2 | 3) {
        return None;
    }
    let len = usize::from(*data.get(off + 2)?);
    if !(1..=80).contains(&len) {
        return None;
    }
    let name = data.get(off + 3..off + 3 + len)?;
    if !name.iter().all(|b| (32..127).contains(b)) {
        return None;
    }
    data.get(off + 3 + len..off + 3 + len + 16)?;
    Some((off + 3 + len, text(name)))
}

pub fn parse_dsn(data: &[u8]) -> Result<DsnDocument, DsnError> {
    if data.len() > 4 * 1024 * 1024 {
        return Err(error(0, "arquivo excede o limite de 4 MiB"));
    }
    if !data.starts_with(b"ISIS SCHEMATIC FILE\x1a") {
        return Err(error(0, "assinatura ISIS SCHEMATIC FILE ausente"));
    }
    let circuit = occurrences(data, b"ISIS CIRCUIT FILE\x1a")
        .next()
        .ok_or_else(|| error(0, "seção de circuito não localizada"))?;
    let end = occurrences(&data[circuit + 1..], b"ISIS CIRCUIT FILE\x1a")
        .next()
        .map_or(data.len(), |o| circuit + 1 + o);
    let section = &data[circuit..end];
    let library_data = &data[..circuit];
    let bounded_data = &data[..end];
    let mut libraries = Libraries::new(library_data);
    if occurrences(section, b"OBJECT DATA\0").next().is_none() {
        return Err(error(circuit, "seção OBJECT DATA ausente"));
    }
    let mut document = DsnDocument::default();
    let mut wire_starts = BTreeMap::new();
    let mut vertex_total = 0usize;
    for rel in occurrences(section, b"\x02\x7fWIRE\0") {
        let off = circuit + rel;
        if document.wires.len() >= 20_000 {
            return Err(error(off, "limite de 20 mil fios excedido"));
        }
        let attr_count = word(bounded_data, off + 7)
            .ok_or_else(|| error(off, "contagem de atributos truncada"))?
            as usize;
        if attr_count > 32 {
            return Err(error(off, "mais de 32 atributos em um fio"));
        }
        let mut count_offset = off + 9;
        let mut labels = Vec::new();
        for _ in 0..attr_count {
            let search_end = count_offset.saturating_add(1024).min(end);
            let attr_data = data
                .get(count_offset..search_end)
                .ok_or_else(|| error(count_offset, "atributo de fio ultrapassa a seção"))?;
            let label = occurrences(attr_data, b"WIRE LABEL\0")
                .next()
                .map(|rel| count_offset + rel)
                .ok_or_else(|| error(count_offset, "atributo de fio não reconhecido"))?;
            let value = attribute_text(bounded_data, label, count_offset)
                .ok_or_else(|| error(label, "texto de rede não reconhecido"))?;
            labels.push(value);
            count_offset = label + 15;
        }
        let count = word(bounded_data, count_offset)
            .ok_or_else(|| error(off, "contagem de vértices truncada"))?
            as usize;
        if count < 2 {
            return Err(error(off, "fio com menos de dois vértices"));
        }
        vertex_total += count;
        if vertex_total > 100_000 {
            return Err(error(off, "limite de 100 mil vértices excedido"));
        }
        let mut points = Vec::with_capacity(count);
        for index in 0..count {
            points.push(
                point(
                    data.get(..end).unwrap_or(data),
                    count_offset + 2 + index * 8,
                )
                .ok_or_else(|| error(off, "vértices de fio truncados"))?,
            );
        }
        // Os vínculos de junção apontam ao início do estilo, 23 bytes antes.
        wire_starts.insert(off.saturating_sub(23), document.wires.len());
        for name in labels {
            document.labels.push(DsnLabel {
                name,
                position: points[0],
                is_ground: false,
            });
        }
        document.wires.push(DsnWire {
            offset: off,
            points,
        });
    }
    let mut lower_bound = circuit;
    let mut pin_cache: BTreeMap<String, (Vec<DsnPin>, Vec<String>)> = BTreeMap::new();
    for rel in occurrences(section, b"PROPERTIES\0\0\0\0\0") {
        let label = circuit + rel;
        let count_pos = label + 15;
        if label < lower_bound {
            continue;
        }
        let Some(count) = dword(bounded_data, count_pos).map(|v| v as usize) else {
            continue;
        };
        let immediate = count_pos + 4;
        let mut header = component_header(bounded_data, immediate).map(|h| (immediate, h, false));
        if header.is_none() {
            header = immediate
                .checked_add(count)
                .filter(|p| *p < end)
                .and_then(|p| component_header(bounded_data, p).map(|h| (p, h, true)));
        }
        let Some((off, (transform_pos, symbol_name), inline)) = header else {
            lower_bound = label + 11;
            continue;
        };
        if document.components.len() >= 20_000 {
            return Err(error(off, "limite de 20 mil componentes excedido"));
        }
        let preceding = &data[lower_bound..label];
        let field = |marker: &[u8]| {
            occurrences(preceding, marker)
                .last()
                .and_then(|rel| attribute_text(data, lower_bound + rel, lower_bound))
                .unwrap_or_default()
        };
        let reference = field(b"COMPONENT ID\0");
        let value = field(b"COMPONENT VALUE\0");
        if count > 64 * 1024 {
            return Err(error(count_pos, "propriedades excedem 64 KiB"));
        }
        let properties_raw = if inline {
            text(&data[immediate..off])
        } else {
            attribute_text(data, label, lower_bound).unwrap_or_default()
        };
        let position = point(data, transform_pos)
            .ok_or_else(|| error(off, "posição de componente truncada"))?;
        let angle_tenths =
            word(data, transform_pos + 8).ok_or_else(|| error(off, "rotação truncada"))? as i16;
        let mirror_flags =
            word(data, transform_pos + 10).ok_or_else(|| error(off, "espelho truncado"))?;
        let source_id =
            dword(data, transform_pos + 12).ok_or_else(|| error(off, "identificador truncado"))?;
        let (mut pins, mut pin_warnings) = pin_cache
            .entry(symbol_name.clone())
            .or_insert_with(|| library_pins(&mut libraries, &symbol_name))
            .clone();
        if libraries.exhausted {
            return Err(error(
                off,
                "limite de 128 bibliotecas ou orçamento de varredura de 64 MiB excedido",
            ));
        }
        let mut pins_complete = pin_warnings.is_empty();
        pins.retain_mut(|pin| {
            if let Some(p) = transform(pin.position, position, angle_tenths, mirror_flags) {
                pin.position = p;
                true
            } else {
                pins_complete = false;
                false
            }
        });
        if !pins_complete {
            pin_warnings.push(format!(
                "Geometria elétrica incompleta para {reference} ({symbol_name}) em 0x{off:X}."
            ));
        }
        document.warnings.extend(pin_warnings);
        document.components.push(DsnComponent {
            offset: off,
            source_id,
            reference,
            value,
            symbol_name,
            position,
            angle_tenths,
            mirror_flags,
            properties_raw,
            pins,
            pins_complete,
        });
        lower_bound = transform_pos + 16;
    }
    // O terra tem semântica própria; o marcador da biblioteca, e não a posição
    // do desenho do símbolo, define sua conexão com a rede.
    for rel in occurrences(section, b"$TERGROUND") {
        let off = circuit + rel;
        if off < 14 || data[off - 14] != 0x10 || data[off - 1] != 10 {
            continue;
        }
        let decoded = (|| {
            let blob = libraries.get("$TERGROUND")?;
            let node = marker_point(blob, b"$MKRNODE\0")?;
            let origin = marker_point(blob, b"$MKRORIGIN\0")?;
            let relative = DsnPoint {
                x: node.x.checked_sub(origin.x)?,
                y: node.y.checked_sub(origin.y)?,
            };
            transform(
                relative,
                point(data, off - 13)?,
                word(data, off - 5)? as i16,
                word(data, off - 3)?,
            )
        })();
        if libraries.exhausted {
            return Err(error(off, "orçamento de bibliotecas excedido"));
        }
        if let Some(position) = decoded {
            document.labels.push(DsnLabel {
                name: "0".to_owned(),
                position,
                is_ground: true,
            });
        } else {
            document
                .warnings
                .push(format!("Terminal de terra em 0x{off:X} não recuperado."));
        }
    }
    // Junções são registros explícitos com três ou mais referências a fios.
    // A coordenada precisa existir em cada polilinha referenciada; proximidade
    // e cruzamento visual não constituem vínculo elétrico.
    for off in circuit..end.saturating_sub(25) {
        if data[off] != 1 {
            continue;
        }
        let Some(position) = point(data, off + 1) else {
            continue;
        };
        let mut cursor = off + 9;
        let mut linked = Vec::new();
        let mut terminated = false;
        while let Some(link) = dword(bounded_data, cursor) {
            cursor += 4;
            if link == 0 {
                terminated = true;
                break;
            }
            let Some(&index) = wire_starts.get(&(link as usize)) else {
                linked.clear();
                break;
            };
            if !document.wires[index].points.contains(&position) || linked.contains(&index) {
                linked.clear();
                break;
            }
            linked.push(index);
        }
        if terminated && linked.len() >= 3 && !document.junctions.contains(&position) {
            document.junctions.push(position);
        }
    }
    document.warnings.push("Leitor experimental ISIS 8: folhas adicionais, barramentos, terminais além do terra, geradores e gráficos ainda não são integralmente recuperados; compatibilidade completa não é garantida.".to_owned());
    if document.components.is_empty() && document.wires.is_empty() {
        return Err(error(circuit, "nenhum componente ou fio reconhecido"));
    }
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_preserves_exact_mirror_before_rotation() {
        let p = DsnPoint { x: -254_000, y: 0 };
        assert_eq!(
            transform(
                p,
                DsnPoint {
                    x: 508_000,
                    y: 508_000
                },
                -900,
                2
            ),
            Some(DsnPoint {
                x: 508_000,
                y: 762_000
            })
        );
        assert_eq!(
            transform(p, DsnPoint::default(), 0, 1),
            Some(DsnPoint { x: 254_000, y: 0 })
        );
        assert_eq!(transform(p, DsnPoint::default(), 15, 0), None);
    }

    #[test]
    fn synthetic_wire_is_read_without_external_fixture() {
        let mut bytes =
            b"ISIS SCHEMATIC FILE\x1aISIS CIRCUIT FILE\x1aOBJECT DATA\0\x02\x7fWIRE\0\0\0".to_vec();
        bytes.extend(2u16.to_le_bytes());
        for value in [-300i32, 400, 500, 400] {
            bytes.extend(value.to_le_bytes());
        }
        let parsed = parse_dsn(&bytes).expect("DSN sintético");
        assert_eq!(
            parsed.wires[0].points,
            vec![DsnPoint { x: -300, y: 400 }, DsnPoint { x: 500, y: 400 }]
        );
        bytes.pop();
        assert!(parse_dsn(&bytes).is_err());
    }

    fn marker(blob: &mut Vec<u8>, name: &[u8], p: DsnPoint) {
        blob.extend(p.x.to_le_bytes());
        blob.extend(p.y.to_le_bytes());
        blob.extend([0; 4]);
        blob.extend(name);
        blob.push(0);
    }

    fn definition(out: &mut Vec<u8>, name: &[u8], blob: &[u8]) {
        out.push(name.len() as u8);
        out.extend(name);
        out.extend(0u32.to_le_bytes());
        out.extend((blob.len() as u32 + 4).to_le_bytes());
        out.extend(blob);
    }

    #[test]
    fn synthetic_component_uses_embedded_node_and_instance_transform() {
        let mut bytes = b"ISIS SCHEMATIC FILE\x1a".to_vec();
        let mut template = Vec::new();
        marker(&mut template, b"$MKRORIGIN", DsnPoint::default());
        marker(&mut template, b"$MKRNODE", DsnPoint { x: -10, y: 0 });
        definition(&mut bytes, b"$PINSHORT", &template);
        let mut symbol = Vec::new();
        marker(&mut symbol, b"$MKRORIGIN", DsnPoint::default());
        marker(&mut symbol, b"$PINSHORT", DsnPoint { x: 10, y: 0 });
        symbol.extend(b"A\x001\0");
        definition(&mut bytes, b"SYNTHETIC", &symbol);
        bytes.extend(b"ISIS CIRCUIT FILE\x1aOBJECT DATA\0PROPERTIES\0");
        bytes.extend([0; 8]);
        bytes.extend(2u16.to_le_bytes());
        bytes.push(9);
        bytes.extend(b"SYNTHETIC");
        bytes.extend(40i32.to_le_bytes());
        bytes.extend(20i32.to_le_bytes());
        bytes.extend((-900i16).to_le_bytes());
        bytes.extend(1u16.to_le_bytes());
        bytes.extend(7u32.to_le_bytes());
        let parsed = parse_dsn(&bytes).expect("componente sintético");
        let component = &parsed.components[0];
        assert_eq!(component.source_id, 7);
        assert!(component.pins_complete);
        assert_eq!(component.pins[0].name, "A");
        assert_eq!(component.pins[0].number, "1");
        assert_eq!(component.pins[0].position, DsnPoint { x: 40, y: 20 });
    }

    fn synthetic_header() -> Vec<u8> {
        b"ISIS SCHEMATIC FILE\x1aISIS CIRCUIT FILE\x1aOBJECT DATA\0".to_vec()
    }

    fn append_wire(out: &mut Vec<u8>, count: u16) {
        out.extend(b"\x02\x7fWIRE\0\0\0");
        out.extend(count.to_le_bytes());
        out.resize(out.len() + usize::from(count) * 8, 0);
    }

    #[test]
    fn input_and_wire_count_limits_are_enforced() {
        let oversized = vec![0; 4 * 1024 * 1024 + 1];
        assert!(parse_dsn(&oversized).unwrap_err().message.contains("4 MiB"));
        let mut vertices = synthetic_header();
        append_wire(&mut vertices, 60_000);
        append_wire(&mut vertices, 60_000);
        assert!(
            parse_dsn(&vertices)
                .unwrap_err()
                .message
                .contains("100 mil vértices")
        );
        let mut wires = synthetic_header();
        for _ in 0..20_001 {
            append_wire(&mut wires, 2);
        }
        assert!(
            parse_dsn(&wires)
                .unwrap_err()
                .message
                .contains("20 mil fios")
        );
        let mut components = synthetic_header();
        for _ in 0..20_001 {
            components.extend(b"PROPERTIES\0");
            components.extend([0; 8]);
            components.extend([2, 0, 1, b'X']);
            components.extend([0; 16]);
        }
        assert!(
            parse_dsn(&components)
                .unwrap_err()
                .message
                .contains("20 mil componentes")
        );
    }

    #[test]
    fn library_count_and_scan_budget_are_enforced() {
        let mut libraries = Libraries::new(&[]);
        for index in 0..128 {
            assert!(libraries.get(&format!("ABSENT{index}")).is_none());
        }
        assert!(!libraries.exhausted);
        assert!(libraries.get("ONE_TOO_MANY").is_none());
        assert!(libraries.exhausted);
        let data = [0; 128];
        let mut libraries = Libraries::new(&data);
        libraries.budget = 127;
        assert!(libraries.get("ABSENT").is_none());
        assert!(libraries.exhausted);

        let mut data = Vec::new();
        definition(&mut data, b"NAME", &[0; 32]);
        let mut budget = data.len() + 5;
        assert!(library(&data, "NAME", &mut budget).is_none());
        assert_eq!(
            budget, 0,
            "candidatos sobrepostos também consomem orçamento"
        );
    }

    #[test]
    fn symbol_with_more_than_64_pins_is_not_partially_accepted() {
        let mut data = Vec::new();
        let mut template = Vec::new();
        marker(&mut template, b"$MKRORIGIN", DsnPoint::default());
        marker(&mut template, b"$MKRNODE", DsnPoint::default());
        definition(&mut data, b"$PINSHORT", &template);
        let mut symbol = Vec::new();
        marker(&mut symbol, b"$MKRORIGIN", DsnPoint::default());
        for _ in 0..65 {
            marker(&mut symbol, b"$PINSHORT", DsnPoint::default());
            symbol.extend(b"A\0\0");
        }
        definition(&mut data, b"SYNTHETIC", &symbol);
        let mut libraries = Libraries::new(&data);
        let (pins, warnings) = library_pins(&mut libraries, "SYNTHETIC");
        assert!(pins.is_empty());
        assert!(warnings.iter().any(|warning| warning.contains("64 pinos")));
    }

    #[test]
    fn attribute_and_next_section_bounds_are_checked() {
        assert!(attribute_text(&[], 1, 0).is_none());
        assert!(attribute_text(&[0; 10], 5, 6).is_none());
        let mut bytes = synthetic_header();
        bytes.extend(b"\x02\x7fWIRE\0\x02\0\xff\0");
        bytes.extend([0; 32]);
        bytes.extend(b"\xff\x01Font\0WIRE LABEL\0");
        assert!(parse_dsn(&bytes).is_err());
        // Uma segunda seção nunca pode completar o contador truncado da primeira.
        let mut bytes = synthetic_header();
        bytes.extend(b"\x02\x7fWIRE\0");
        bytes.extend(b"ISIS CIRCUIT FILE\x1aOBJECT DATA\0");
        assert!(parse_dsn(&bytes).is_err());
    }

    #[test]
    fn deterministic_random_and_all_truncated_headers_do_not_panic() {
        let mut valid = synthetic_header();
        append_wire(&mut valid, 2);
        for cut in 0..valid.len() {
            assert!(std::panic::catch_unwind(|| parse_dsn(&valid[..cut])).is_ok());
        }
        let markers: [&[u8]; 5] = [
            b"PROPERTIES\0\0\0\0\0",
            b"\x02\x7fWIRE\0",
            b"WIRE LABEL\0",
            b"$TERGROUND",
            b"COMPONENT ID\0",
        ];
        let mut state = 0x4a39_f05d_9e27_b813u64;
        for index in 0..2_000 {
            let mut bytes = synthetic_header();
            let len = index % 4096;
            for _ in 0..len {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                bytes.push(state as u8);
            }
            bytes.extend(markers[index % markers.len()]);
            let cut = (state as usize) % (bytes.len() + 1);
            assert!(
                std::panic::catch_unwind(|| parse_dsn(&bytes[..cut])).is_ok(),
                "caso determinístico {index}"
            );
            assert!(
                std::panic::catch_unwind(|| parse_dsn(&bytes)).is_ok(),
                "marcador truncado {index}"
            );
        }
    }
}
