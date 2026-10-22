//! Importação independente e limitada de projetos Proteus.
pub mod cdb;
pub mod container;
mod conversion;
pub mod dsn;
pub use conversion::{convert_document, import_proteus};

use eda_core::Project;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReport {
    pub format: String,
    pub file_version: Option<u32>,
    pub components: usize,
    pub wires: usize,
    pub junctions: usize,
    pub labels: usize,
    pub unsupported_components: Vec<String>,
    pub warnings: Vec<String>,
    pub electrical_complete: bool,
}
#[derive(Debug, Clone)]
pub struct ImportResult {
    pub project: Project,
    pub report: ImportReport,
}

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("Não foi possível ler o arquivo Proteus: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Container(#[from] container::ContainerError),
    #[error(transparent)]
    Cdb(#[from] cdb::CdbError),
    #[error(transparent)]
    Dsn(#[from] dsn::DsnError),
    #[error("Formato Proteus não suportado: {0}")]
    Unsupported(String),
    #[error("Não foi possível converter o projeto: {0}")]
    Conversion(String),
}
