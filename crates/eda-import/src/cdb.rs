//! Leitura independente do perfil CDB observado em projetos ISIS de uma folha.
//! Os campos ainda sem semântica confirmada são preservados, sem inferir redes.
use std::collections::BTreeSet;
use std::fmt;

const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_RECORDS: usize = 20_000;
const MAX_PINS: usize = 1024;
const MAX_PROPERTIES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdbDocument {
    pub root_name: String,
    pub sheet_name: String,
    pub instances: Vec<CdbInstance>,
    pub parts: Vec<CdbPart>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdbInstance {
    pub id: u32,
    pub field_2: u32,
    pub field_3: u32,
    pub reference: String,
    pub pins: Vec<CdbPin>,
    pub field_4: u32,
    pub part_id: Option<u32>,
    pub field_6: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdbPin {
    pub name: String,
    /// Vazio quando o CDB não fornece um número físico separado do nome.
    pub number: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdbPart {
    pub id: u32,
    pub field_2: u32,
    pub field_3: u32,
    pub reference: String,
    pub value: String,
    pub device: String,
    pub package: String,
    /// Texto inerte, preservado sem interpretar ou executar diretivas de modelo.
    pub properties: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdbError {
    pub offset: usize,
    pub message: String,
}

impl fmt::Display for CdbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CDB no byte {}: {}", self.offset, self.message)
    }
}
impl std::error::Error for CdbError {}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn error(&self, message: impl Into<String>) -> CdbError {
        CdbError {
            offset: self.pos,
            message: message.into(),
        }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], CdbError> {
        let end = self
            .pos
            .checked_add(count)
            .ok_or_else(|| self.error("tamanho inválido"))?;
        let result = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| self.error("arquivo truncado"))?;
        self.pos = end;
        Ok(result)
    }

    fn u32(&mut self) -> Result<u32, CdbError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn words(&mut self, expected: &[u32]) -> Result<(), CdbError> {
        for &word in expected {
            let offset = self.pos;
            if self.u32()? != word {
                return Err(CdbError {
                    offset,
                    message: "perfil CDB não suportado (cabeçalho ou hierarquia)".into(),
                });
            }
        }
        Ok(())
    }

    fn zero(&mut self) -> Result<(), CdbError> {
        if self.take(1)?[0] != 0 {
            return Err(self.error("perfil CDB não suportado (marcador)"));
        }
        Ok(())
    }

    fn text(&mut self, length: usize) -> Result<String, CdbError> {
        let offset = self.pos;
        let bytes = self.take(length)?;
        if bytes.iter().any(|&b| b == 0 || !b.is_ascii()) {
            return Err(CdbError {
                offset,
                message: "texto CDB fora do perfil ASCII suportado".into(),
            });
        }
        Ok(bytes.iter().map(|&b| char::from(b)).collect())
    }

    fn string(&mut self) -> Result<String, CdbError> {
        let length = usize::from(self.take(1)?[0]);
        self.text(length)
    }

    fn count(&mut self, max: usize, minimum_bytes: usize) -> Result<usize, CdbError> {
        let count = self.u32()? as usize;
        if count > max || count > self.bytes.len().saturating_sub(self.pos) / minimum_bytes {
            return Err(self.error("contagem excede os limites ou o tamanho disponível"));
        }
        Ok(count)
    }

    fn properties(&mut self) -> Result<String, CdbError> {
        // O tamanho inclui o próprio u32 e o terminador NUL (não apenas o texto).
        let inclusive_length = self.u32()? as usize;
        if !(5..=MAX_PROPERTIES).contains(&inclusive_length) {
            return Err(self.error("tamanho do bloco de propriedades inválido"));
        }
        let text = self.text(inclusive_length - 5)?;
        self.zero()?;
        Ok(text)
    }
}

/// Aceita somente o perfil conhecido de uma folha; qualquer divergência falha
/// com posição, sem buscar strings soltas ou recuperar registros por heurística.
pub fn parse_cdb(bytes: &[u8]) -> Result<CdbDocument, CdbError> {
    let mut r = Reader { bytes, pos: 0 };
    if bytes.len() > MAX_BYTES {
        return Err(r.error("arquivo excede 16 MiB"));
    }
    r.words(&[2, 1, 1, 0])?;
    let root_name = r.string()?;
    r.words(&[0])?;
    r.zero()?;
    r.words(&[1, 1, 2, 1, 1, 1])?;
    r.zero()?;
    r.words(&[10, 0, 2, 2, 0])?;
    let sheet_name = r.string()?;
    r.words(&[10, 0])?;
    let count = r.count(MAX_RECORDS, 29)?;
    let mut instances = Vec::with_capacity(count);
    let mut instance_ids = BTreeSet::new();
    for _ in 0..count {
        let id = r.u32()?;
        if id == 0 || !instance_ids.insert(id) {
            return Err(r.error("ID de instância nulo ou duplicado"));
        }
        let field_2 = r.u32()?;
        let field_3 = r.u32()?;
        let reference = r.string()?;
        let pin_count = r.count(MAX_PINS, 2)?;
        let mut pins = Vec::with_capacity(pin_count);
        for _ in 0..pin_count {
            pins.push(CdbPin {
                name: r.string()?,
                number: r.string()?,
            });
        }
        let field_4 = r.u32()?;
        let part = r.u32()?;
        let field_6 = r.u32()?;
        instances.push(CdbInstance {
            id,
            field_2,
            field_3,
            reference,
            pins,
            field_4,
            part_id: (part != 0).then_some(part),
            field_6,
        });
    }
    r.words(&[1, 1, 1])?;
    let count = r.count(MAX_RECORDS, 21)?;
    let mut parts = Vec::with_capacity(count);
    let mut part_ids = BTreeSet::new();
    for _ in 0..count {
        let id = r.u32()?;
        if id == 0 || !part_ids.insert(id) {
            return Err(r.error("ID de peça nulo ou duplicado"));
        }
        parts.push(CdbPart {
            id,
            field_2: r.u32()?,
            field_3: r.u32()?,
            reference: r.string()?,
            value: r.string()?,
            device: r.string()?,
            package: r.string()?,
            properties: r.properties()?,
        });
    }
    if r.pos != bytes.len() {
        return Err(r.error("dados adicionais após a tabela de peças"));
    }
    for instance in &instances {
        if instance.part_id.is_some_and(|id| !part_ids.contains(&id)) {
            return Err(r.error(format!(
                "instância {} aponta para peça inexistente",
                instance.id
            )));
        }
    }
    Ok(CdbDocument {
        root_name,
        sheet_name,
        instances,
        parts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(out: &mut Vec<u8>, values: &[u32]) {
        for v in values {
            out.extend(v.to_le_bytes());
        }
    }
    fn string(out: &mut Vec<u8>, text: &str) {
        out.push(text.len() as u8);
        out.extend(text.as_bytes());
    }
    fn fixture() -> Vec<u8> {
        // Fixture autoral: uma peça MOS fictícia; sem dados ou símbolos do Proteus.
        let mut out = vec![];
        words(&mut out, &[2, 1, 1, 0]);
        string(&mut out, "ROOT");
        words(&mut out, &[0]);
        out.push(0);
        words(&mut out, &[1, 1, 2, 1, 1, 1]);
        out.push(0);
        words(&mut out, &[10, 0, 2, 2, 0]);
        string(&mut out, "Folha teste");
        words(&mut out, &[10, 0, 1, 7, 1, 9]);
        string(&mut out, "Q9");
        words(&mut out, &[3]);
        for (name, number) in [("D", "2"), ("G", "1"), ("S", "3")] {
            string(&mut out, name);
            string(&mut out, number);
        }
        words(&mut out, &[0, 42, u32::MAX, 1, 1, 1, 1, 42, 1, 0]);
        for s in ["Q9", "MOS_TEST", "GENERIC_MOS", ""] {
            string(&mut out, s);
        }
        let props = "{TEST=ORIGINAL}\n";
        words(&mut out, &[props.len() as u32 + 5]);
        out.extend(props.as_bytes());
        out.push(0);
        out
    }

    #[test]
    fn preserves_pin_mapping_and_metadata() {
        let document = parse_cdb(&fixture()).unwrap();
        assert_eq!(document.sheet_name, "Folha teste");
        assert_eq!(document.instances[0].part_id, Some(42));
        assert_eq!(document.instances[0].field_3, 9);
        assert_eq!(
            document.instances[0].pins[0],
            CdbPin {
                name: "D".into(),
                number: "2".into()
            }
        );
        assert_eq!(document.parts[0].device, "GENERIC_MOS");
        assert_eq!(document.parts[0].properties, "{TEST=ORIGINAL}\n");
    }

    #[test]
    fn every_truncation_is_rejected() {
        let bytes = fixture();
        for end in 0..bytes.len() {
            assert!(parse_cdb(&bytes[..end]).is_err(), "corte {end}");
        }
    }

    #[test]
    fn rejects_header_count_encoding_and_trailing_data() {
        let mut bytes = fixture();
        bytes[0] = 3;
        assert!(parse_cdb(&bytes).is_err());
        let mut bytes = fixture();
        bytes[17] = 255;
        assert!(parse_cdb(&bytes).is_err());
        let mut bytes = fixture();
        bytes.push(0);
        assert!(parse_cdb(&bytes).is_err());
        let mut bytes = fixture();
        // O prefixo variável termina após os dois u32 seguintes ao nome da folha.
        let count_offset = 16 + 5 + 4 + 1 + 24 + 1 + 20 + 12 + 8;
        bytes[count_offset..count_offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse_cdb(&bytes).is_err());
    }

    #[test]
    fn rejects_duplicate_instance_and_part_ids() {
        let original = fixture();
        let instance_start = 95;
        let instance_end = instance_start + 43;
        let mut bytes = original.clone();
        bytes[instance_start - 4..instance_start].copy_from_slice(&2u32.to_le_bytes());
        bytes.splice(
            instance_end..instance_end,
            original[instance_start..instance_end].iter().copied(),
        );
        assert!(parse_cdb(&bytes).unwrap_err().message.contains("duplicado"));
        let part_start = instance_end + 16;
        let mut bytes = original.clone();
        bytes[part_start - 4..part_start].copy_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&original[part_start..]);
        assert!(parse_cdb(&bytes).unwrap_err().message.contains("duplicado"));
    }

    #[test]
    fn rejects_dangling_part_and_invalid_property_size() {
        let mut bytes = fixture();
        // Troca o ID da única peça sem alterar o vínculo na instância.
        let pattern = [42u32.to_le_bytes(), 1u32.to_le_bytes(), 0u32.to_le_bytes()].concat();
        let index = bytes
            .windows(pattern.len())
            .position(|b| b == pattern)
            .unwrap();
        bytes[index..index + 4].copy_from_slice(&43u32.to_le_bytes());
        assert!(parse_cdb(&bytes).is_err());
        let mut bytes = fixture();
        let index = bytes.len() - "{TEST=ORIGINAL}\n".len() - 5;
        bytes[index..index + 4].copy_from_slice(&3u32.to_le_bytes());
        assert!(parse_cdb(&bytes).is_err());
    }

    #[test]
    fn external_samples_opt_in() {
        let Some(root) = std::env::var_os("PROTOCRAFT_PROTEUS_SAMPLES") else {
            return;
        };
        let cases = [
            ("01_Circuito_RC", 5, 3),
            ("02_Portas_Logicas", 18, 14),
            ("03_Multiplexador", 17, 12),
            ("04_FlipFlop_RS", 12, 8),
            ("05_Inversor_CMOS", 9, 3),
            ("06_Somador_Completo", 48, 42),
        ];
        for (folder, instances, parts) in cases {
            let path = std::path::Path::new(&root).join(folder).join("ROOT.CDB");
            let bytes = std::fs::read(path).unwrap();
            let document = parse_cdb(&bytes).unwrap();
            assert_eq!(document.instances.len(), instances, "{folder}");
            assert_eq!(document.parts.len(), parts, "{folder}");
        }
    }
}
