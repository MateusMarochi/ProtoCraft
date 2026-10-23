//! PersistÃªncia validada, limitada e substituÃ­da atomicamente.
use eda_core::Project;
use eda_simulation::SimulationResult;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use thiserror::Error;
pub mod fabrication;

pub const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;
pub const FORMAT_VERSION: u32 = eda_core::FORMAT_VERSION;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Falha de acesso ao arquivo: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON de projeto invÃ¡lido: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Projeto invÃ¡lido: {0}")]
    Invalid(String),
    #[error("Versão de formato {0} não suportada; esta versão lê formatos 1 a 6")]
    Version(u64),
    #[error("Arquivo excede o limite de 8 MiB")]
    TooLarge,
}

pub fn load(path: impl AsRef<Path>) -> Result<Project, StorageError> {
    let file = fs::File::open(path)?;
    if file.metadata()?.len() > MAX_FILE_BYTES {
        return Err(StorageError::TooLarge);
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(StorageError::TooLarge);
    }
    decode(&bytes)
}

fn decode(bytes: &[u8]) -> Result<Project, StorageError> {
    let value: serde_json::Value = serde_json::from_slice(bytes)?;
    let version = value
        .get("format_version")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| StorageError::Invalid("format_version ausente ou invÃ¡lido".into()))?;
    if !(1..=u64::from(FORMAT_VERSION)).contains(&version) {
        return Err(StorageError::Version(version));
    }
    let mut project: Project = serde_json::from_value(value)?;
    // O formato 2 adiciona geometria e metadados opcionais; os objetos v1 sÃ£o preservados.
    if version < u64::from(FORMAT_VERSION) {
        project.format_version = FORMAT_VERSION;
    }
    project
        .validate()
        .map_err(|e| StorageError::Invalid(e.to_string()))?;
    Ok(project)
}

/// O backup contÃ©m exatamente o arquivo vÃ¡lido que precedeu o salvamento.
pub fn backup_path(path: impl AsRef<Path>) -> PathBuf {
    let mut name = path.as_ref().as_os_str().to_owned();
    name.push(".bak");
    PathBuf::from(name)
}

pub fn save(path: impl AsRef<Path>, project: &Project) -> Result<(), StorageError> {
    let bytes = encode(project)?;
    let path = path.as_ref();
    if path.exists() {
        // NÃ£o substituÃ­mos arquivo corrompido ou de versÃ£o que este programa desconhece.
        load(path)?;
        let previous = fs::read(path)?;
        atomic_write(&backup_path(path), &previous)?;
    }
    atomic_write(path, &bytes)
}

/// Cria um projeto sem substituir um destino que jÃ¡ exista.
pub fn create_new(path: impl AsRef<Path>, project: &Project) -> Result<(), StorageError> {
    let bytes = encode(project)?;
    let path = path.as_ref();
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(&bytes)?;
    temp.as_file().sync_all()?;
    temp.persist_noclobber(path)
        .map(|_| ())
        .map_err(|error| StorageError::Io(error.error))
}

fn encode(project: &Project) -> Result<Vec<u8>, StorageError> {
    project
        .validate()
        .map_err(|e| StorageError::Invalid(e.to_string()))?;
    if project.format_version != FORMAT_VERSION {
        return Err(StorageError::Version(project.format_version.into()));
    }
    let mut bytes = serde_json::to_vec_pretty(project)?;
    bytes.push(b'\n');
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(StorageError::TooLarge);
    }
    Ok(bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), StorageError> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| StorageError::Io(e.error))?;
    Ok(())
}

/// CSV longo: uma linha por amostra, permitindo sÃ©ries de comprimentos diferentes.
pub fn export_csv(path: impl AsRef<Path>, result: &SimulationResult) -> Result<(), StorageError> {
    let mut csv = String::from("serie,unidade,tempo_s,valor\n");
    for trace in &result.traces {
        for sample in &trace.samples {
            if !sample[0].is_finite() || !sample[1].is_finite() {
                return Err(StorageError::Invalid(
                    "Resultado contÃ©m amostra nÃ£o finita".into(),
                ));
            }
            csv.push_str(&format!(
                "{},{},{},{}\n",
                quote(&trace.name),
                quote(&trace.unit),
                sample[0],
                sample[1]
            ));
        }
    }
    atomic_write(path.as_ref(), csv.as_bytes())
}
fn quote(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migrates_v1_without_changing_ids_and_keeps_original_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("legado.protocraft");
        let current = eda_core::examples::voltage_divider();
        let mut legacy = serde_json::to_value(&current).unwrap();
        legacy["format_version"] = 1.into();
        let bytes = serde_json::to_vec(&legacy).unwrap();
        fs::write(&path, &bytes).unwrap();
        let migrated = load(&path).unwrap();
        assert_eq!(migrated, current);
        assert_eq!(migrated.format_version, FORMAT_VERSION);
        save(&path, &migrated).unwrap();
        assert_eq!(fs::read(backup_path(&path)).unwrap(), bytes);
    }
    #[test]
    fn round_trip_and_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("projeto.protocraft");
        let first = eda_core::examples::voltage_divider();
        save(&path, &first).unwrap();
        assert_eq!(
            serde_json::to_value(load(&path).unwrap()).unwrap(),
            serde_json::to_value(&first).unwrap()
        );
        let previous = fs::read(&path).unwrap();
        let mut second = first;
        second.name = "Alterado".into();
        save(&path, &second).unwrap();
        assert_eq!(fs::read(backup_path(&path)).unwrap(), previous);
        assert_eq!(load(&path).unwrap().name, "Alterado");
    }
    #[test]
    fn refuses_future_and_corrupt_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("futuro.protocraft");
        for contents in [r#"{"format_version":99}"#, "nÃ£o Ã© JSON"] {
            fs::write(&path, contents).unwrap();
            assert!(load(&path).is_err());
            assert!(save(&path, &eda_core::examples::voltage_divider()).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), contents);
        }
    }
    #[test]
    fn rejects_invalid_project_and_large_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("invalido.protocraft");
        let mut project = eda_core::examples::voltage_divider();
        project
            .components
            .values_mut()
            .find(|c| c.kind == eda_core::ComponentKind::Resistor)
            .unwrap()
            .value = -1.0;
        assert!(save(&path, &project).is_err());
        assert!(!path.exists());
        let file = fs::File::create(&path).unwrap();
        file.set_len(MAX_FILE_BYTES + 1).unwrap();
        assert!(matches!(load(&path), Err(StorageError::TooLarge)));
    }
    #[test]
    fn old_versions_migrate_and_current_model_parameters_round_trip() {
        let p = eda_core::examples::rc_filter();
        let mut value = serde_json::to_value(&p).unwrap();
        value["format_version"] = 2.into();
        let migrated = decode(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(migrated.format_version, FORMAT_VERSION);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("onda.protocraft");
        let mut p = migrated;
        p.components
            .get_mut(&eda_core::ComponentId(1))
            .unwrap()
            .parameters
            .frequency_hz = 1234.0;
        save(&path, &p).unwrap();
        assert_eq!(load(&path).unwrap(), p);
    }

    #[test]
    fn v3_models_receive_mixed_defaults_and_new_chips_round_trip() {
        let mut value = serde_json::to_value(eda_core::examples::transistor_bias()).unwrap();
        value["format_version"] = 3.into();
        for component in value["components"].as_object_mut().unwrap().values_mut() {
            if let Some(parameters) = component
                .get_mut("parameters")
                .and_then(|v| v.as_object_mut())
            {
                for key in [
                    "mos_threshold_v",
                    "channel_modulation",
                    "hysteresis_v",
                    "propagation_delay_s",
                    "initial_state",
                ] {
                    parameters.remove(key);
                }
            }
        }
        let migrated = decode(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(migrated.format_version, FORMAT_VERSION);
        for component in migrated.components.values() {
            assert_eq!(component.parameters.propagation_delay_s, 1e-7);
        }
        let mut chip = eda_core::examples::decade_counter();
        let model = chip
            .components
            .values_mut()
            .find(|c| c.kind == eda_core::ComponentKind::Counter10)
            .unwrap();
        model.parameters.initial_state = 9;
        model.parameters.propagation_delay_s = 2e-6;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("contador.protocraft");
        save(&path, &chip).unwrap();
        assert_eq!(load(&path).unwrap(), chip);
    }
}
