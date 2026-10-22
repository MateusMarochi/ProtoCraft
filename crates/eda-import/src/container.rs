//! Leitura limitada e sem extração do contêiner de projeto Proteus 8.
use std::{
    collections::HashSet,
    fs::File,
    io::{Cursor, Read},
    path::Path,
};

use quick_xml::{Reader, events::Event};
use thiserror::Error;

pub const MAX_ARCHIVE_BYTES: usize = 32 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 16 * 1024 * 1024;
const MAX_ROOT_BYTES: u64 = 4 * 1024 * 1024;
const MAX_XML_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProteusMetadata {
    pub name: Option<String>,
    pub file_version: Option<u32>,
    pub release: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ProteusContainer {
    pub metadata: ProteusMetadata,
    pub dsn: Option<Vec<u8>>,
    pub cdb: Option<Vec<u8>>,
    pub ignored_entries: Vec<String>,
}

#[derive(Debug, Error)]
pub enum ContainerError {
    #[error("Não foi possível ler o projeto: {0}")]
    Io(#[from] std::io::Error),
    #[error("Contêiner ZIP inválido: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("Limite de importação excedido: {0}")]
    Limit(&'static str),
    #[error("Nome de membro ZIP inseguro ou duplicado: {0}")]
    UnsafeEntry(String),
    #[error("Membro ZIP criptografado ou link simbólico não permitido: {0}")]
    ForbiddenEntry(String),
    #[error("Membro obrigatório ausente: {0}")]
    Missing(&'static str),
    #[error("PROJECT.XML inválido: {0}")]
    Xml(String),
    #[error("Versão Proteus não suportada: FILEVER={0}; perfil disponível: 800")]
    Unsupported(u32),
}

pub fn read_container(path: impl AsRef<Path>) -> Result<ProteusContainer, ContainerError> {
    let file = File::open(path)?;
    if file.metadata()?.len() > MAX_ARCHIVE_BYTES as u64 {
        return Err(ContainerError::Limit("arquivo maior que 32 MiB"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_ARCHIVE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    read_container_bytes(&bytes)
}

pub fn read_container_bytes(bytes: &[u8]) -> Result<ProteusContainer, ContainerError> {
    if bytes.len() > MAX_ARCHIVE_BYTES {
        return Err(ContainerError::Limit("arquivo maior que 32 MiB"));
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    if archive.len() > 128 {
        return Err(ContainerError::Limit("mais de 128 membros ZIP"));
    }
    let mut names = HashSet::new();
    let mut total = 0u64;
    let mut project = None;
    let mut dsn = None;
    let mut cdb = None;
    let mut ignored_entries = Vec::new();
    for index in 0..archive.len() {
        let mut member = archive.by_index(index)?;
        let name = member.name()?.into_owned();
        validate_name(&name)?;
        let canonical = name.to_ascii_uppercase();
        if !names.insert(canonical.clone()) {
            return Err(ContainerError::UnsafeEntry(name));
        }
        if member.encrypted() || member.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
            return Err(ContainerError::ForbiddenEntry(name));
        }
        total = total
            .checked_add(member.size())
            .ok_or(ContainerError::Limit("tamanho descomprimido inválido"))?;
        if total > MAX_TOTAL_BYTES {
            return Err(ContainerError::Limit(
                "conteúdo descomprimido maior que 16 MiB",
            ));
        }
        let limit = match canonical.as_str() {
            "PROJECT.XML" => MAX_XML_BYTES,
            "ROOT.DSN" | "ROOT.CDB" => MAX_ROOT_BYTES,
            _ => MAX_TOTAL_BYTES,
        };
        if member.size() > limit {
            return Err(ContainerError::Limit("membro ultrapassa limite individual"));
        }
        // Ler também os membros ignorados valida CRC e tamanho sem executar seu conteúdo.
        let mut data = Vec::new();
        (&mut member).take(limit + 1).read_to_end(&mut data)?;
        if data.len() as u64 > limit || data.len() as u64 != member.size() {
            return Err(ContainerError::Limit(
                "tamanho real do membro inconsistente",
            ));
        }
        match canonical.as_str() {
            "PROJECT.XML" => project = Some(data),
            "ROOT.DSN" => dsn = Some(data),
            "ROOT.CDB" => cdb = Some(data),
            _ => ignored_entries.push(name),
        }
    }
    let metadata = parse_metadata(&project.ok_or(ContainerError::Missing("PROJECT.XML"))?)?;
    if dsn.is_none() {
        return Err(ContainerError::Missing("ROOT.DSN"));
    }
    Ok(ProteusContainer {
        metadata,
        dsn,
        cdb,
        ignored_entries,
    })
}

fn validate_name(name: &str) -> Result<(), ContainerError> {
    if name.is_empty()
        || !name.is_ascii()
        || name.contains(['\\', ':', '\0'])
        || name.starts_with('/')
        || name
            .trim_end_matches('/')
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == ".." || s.ends_with([' ', '.']))
    {
        return Err(ContainerError::UnsafeEntry(name.to_owned()));
    }
    Ok(())
}

fn parse_metadata(bytes: &[u8]) -> Result<ProteusMetadata, ContainerError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| ContainerError::Xml("somente XML UTF-8 é suportado".into()))?;
    if text
        .chars()
        .any(|c| c < '\u{20}' && !matches!(c, '\t' | '\n' | '\r'))
    {
        return Err(ContainerError::Xml("caractere de controle inválido".into()));
    }
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().check_end_names = true;
    let mut metadata = ProteusMetadata::default();
    let mut depth = 0usize;
    let mut root_seen = false;
    let mut timestamp_seen = false;
    let mut declaration_seen = false;
    loop {
        let event = reader
            .read_event()
            .map_err(|e| ContainerError::Xml(e.to_string()))?;
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                if depth == 0 {
                    if root_seen || e.name().as_ref() != b"PROJECT" {
                        return Err(ContainerError::Xml("raiz PROJECT única obrigatória".into()));
                    }
                    root_seen = true;
                }
                if depth >= 64 {
                    return Err(ContainerError::Limit("XML com profundidade maior que 64"));
                }
                if e.name().as_ref() == b"TIMESTAMP" {
                    if depth != 1 || timestamp_seen {
                        return Err(ContainerError::Xml(
                            "TIMESTAMP deve ser único e filho de PROJECT".into(),
                        ));
                    }
                    timestamp_seen = true;
                    for attr in e.attributes() {
                        let attr = attr.map_err(|e| ContainerError::Xml(e.to_string()))?;
                        let value = attr
                            .decoded_and_normalized_value(
                                quick_xml::XmlVersion::Implicit1_0,
                                reader.decoder(),
                            )
                            .map_err(|e| ContainerError::Xml(e.to_string()))?;
                        match attr.key.as_ref() {
                            b"FILEVER" => {
                                metadata.file_version = Some(value.parse().map_err(|_| {
                                    ContainerError::Xml("FILEVER deve ser inteiro".into())
                                })?)
                            }
                            b"RELEASE" => {
                                metadata.release = Some(value.parse().map_err(|_| {
                                    ContainerError::Xml("RELEASE deve ser inteiro".into())
                                })?)
                            }
                            _ => {}
                        }
                    }
                } else {
                    for attr in e.attributes() {
                        let attr = attr.map_err(|e| ContainerError::Xml(e.to_string()))?;
                        attr.decoded_and_normalized_value(
                            quick_xml::XmlVersion::Implicit1_0,
                            reader.decoder(),
                        )
                        .map_err(|e| ContainerError::Xml(e.to_string()))?;
                    }
                }
                if matches!(event, Event::Start(_)) {
                    depth += 1;
                }
            }
            Event::End(_) => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| ContainerError::Xml("fechamento sem abertura".into()))?;
            }
            Event::DocType(_) | Event::GeneralRef(_) => {
                return Err(ContainerError::Xml(
                    "DTD e entidades não são permitidos".into(),
                ));
            }
            Event::CData(_) if depth == 0 => {
                return Err(ContainerError::Xml("CDATA fora da raiz PROJECT".into()));
            }
            Event::Decl(e) => {
                if declaration_seen
                    || root_seen
                    || e.version()
                        .map_err(|e| ContainerError::Xml(e.to_string()))?
                        .as_ref()
                        != b"1.0"
                {
                    return Err(ContainerError::Xml(
                        "declaração XML deve preceder PROJECT e usar versão 1.0".into(),
                    ));
                }
                declaration_seen = true;
                if let Some(encoding) = e.encoding() {
                    let encoding = encoding.map_err(|e| ContainerError::Xml(e.to_string()))?;
                    if !encoding.eq_ignore_ascii_case(b"UTF-8") {
                        return Err(ContainerError::Xml("somente XML UTF-8 é suportado".into()));
                    }
                }
            }
            Event::Text(e) if depth == 0 && !e.iter().all(u8::is_ascii_whitespace) => {
                return Err(ContainerError::Xml("texto fora da raiz PROJECT".into()));
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !root_seen || depth != 0 {
        return Err(ContainerError::Xml("documento incompleto".into()));
    }
    match metadata.file_version {
        Some(800) => Ok(metadata),
        Some(version) => Err(ContainerError::Unsupported(version)),
        None => Err(ContainerError::Xml("TIMESTAMP/FILEVER obrigatório".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const XML: &str = "<?xml version='1.0'?><PROJECT><TIMESTAMP FILEVER='800' RELEASE='800'/><SCRIPTS><STRING>PWRRAILS.DAT</STRING></SCRIPTS></PROJECT>";

    fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            writer
                .start_file(
                    *name,
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Stored),
                )
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn reads_profile_and_preserves_payload_without_execution() {
        let bytes = archive(&[
            ("PROJECT.XML", XML.as_bytes()),
            ("ROOT.DSN", b"test"),
            ("FIRMWARE.XML", b"firmware"),
        ]);
        let result = read_container_bytes(&bytes).unwrap();
        assert_eq!(result.metadata.file_version, Some(800));
        assert_eq!(result.metadata.release, Some(800));
        assert_eq!(result.dsn.as_deref(), Some(b"test".as_slice()));
        assert_eq!(result.ignored_entries, ["FIRMWARE.XML"]);
        assert!(result.cdb.is_none());
    }

    #[test]
    fn rejects_unsafe_paths_and_case_collisions() {
        for name in [
            "../ROOT.DSN",
            "/ROOT.DSN",
            "C:/ROOT.DSN",
            "folder\\ROOT.DSN",
            "folder/./x",
            "folder//x",
            "x./test",
        ] {
            assert!(
                matches!(
                    read_container_bytes(&archive(&[(name, b"")])),
                    Err(ContainerError::UnsafeEntry(_))
                ),
                "{name}"
            );
        }
        let bytes = archive(&[("ROOT.DSN", b""), ("root.dsn", b"")]);
        assert!(matches!(
            read_container_bytes(&bytes),
            Err(ContainerError::UnsafeEntry(_))
        ));
    }

    #[test]
    fn rejects_missing_or_unsupported_metadata() {
        for xml in [
            "<PROJECT/>",
            "<PROJECT><TIMESTAMP FILEVER='banana'/></PROJECT>",
            "<PROJECT><TIMESTAMP FILEVER='800'/>",
            "<PROJECT/><PROJECT/>",
        ] {
            assert!(parse_metadata(xml.as_bytes()).is_err(), "{xml}");
        }
        assert!(matches!(
            parse_metadata(b"<PROJECT><TIMESTAMP FILEVER='801'/></PROJECT>"),
            Err(ContainerError::Unsupported(801))
        ));
        assert!(matches!(
            read_container_bytes(&archive(&[("PROJECT.XML", XML.as_bytes())])),
            Err(ContainerError::Missing("ROOT.DSN"))
        ));
        assert!(matches!(
            read_container_bytes(&archive(&[("ROOT.DSN", b"")])),
            Err(ContainerError::Missing("PROJECT.XML"))
        ));
    }

    #[test]
    fn rejects_xml_entities_depth_and_malformed_archives() {
        assert!(parse_metadata(b"<!DOCTYPE PROJECT [<!ENTITY e 'boom'>]><PROJECT><TIMESTAMP FILEVER='800'/></PROJECT>").is_err());
        let deep = format!(
            "<PROJECT>{}<TIMESTAMP FILEVER='800'/>{}</PROJECT>",
            "<X>".repeat(64),
            "</X>".repeat(64)
        );
        assert!(parse_metadata(deep.as_bytes()).is_err());
        assert!(read_container_bytes(b"not a zip").is_err());
        let bytes = archive(&[
            ("PROJECT.XML", XML.as_bytes()),
            ("ROOT.DSN", &vec![0; MAX_ROOT_BYTES as usize + 1]),
        ]);
        assert!(matches!(
            read_container_bytes(&bytes),
            Err(ContainerError::Limit(_))
        ));
    }

    #[test]
    fn rejects_symlinks_too_many_members_and_corrupt_crc() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .add_symlink(
                "ROOT.DSN",
                "outside",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        assert!(matches!(
            read_container_bytes(&writer.finish().unwrap().into_inner()),
            Err(ContainerError::ForbiddenEntry(_))
        ));
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for index in 0..129 {
            writer
                .start_file(
                    format!("x{index}"),
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
        }
        assert!(matches!(
            read_container_bytes(&writer.finish().unwrap().into_inner()),
            Err(ContainerError::Limit(_))
        ));
        let mut bytes = archive(&[("PROJECT.XML", XML.as_bytes()), ("ROOT.DSN", b"DSNPAYLOAD")]);
        let offset = bytes.windows(10).position(|w| w == b"DSNPAYLOAD").unwrap();
        bytes[offset] ^= 1;
        assert!(read_container_bytes(&bytes).is_err());
    }

    #[test]
    fn rejects_compressed_bomb_and_total_uncompressed_limit() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file(
                "ROOT.DSN",
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated),
            )
            .unwrap();
        writer
            .write_all(&vec![0; MAX_ROOT_BYTES as usize + 1])
            .unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        assert!(bytes.len() < 100_000);
        assert!(matches!(
            read_container_bytes(&bytes),
            Err(ContainerError::Limit(_))
        ));
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for index in 0..5 {
            writer
                .start_file(
                    format!("ignored{index}"),
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated),
                )
                .unwrap();
            writer.write_all(&vec![0; MAX_ROOT_BYTES as usize]).unwrap();
        }
        assert!(matches!(
            read_container_bytes(&writer.finish().unwrap().into_inner()),
            Err(ContainerError::Limit(_))
        ));
    }
}
