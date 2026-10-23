use eda_i18n::LanguagePreference;
use eda_simulation::SimulationEngine;
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::PathBuf,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub language: LanguagePreference,
    pub engine: SimulationEngine,
    pub recent_projects: Vec<PathBuf>,
    pub voltage_colors: bool,
    pub current_dots: bool,
    pub voltage_scale: f64,
    pub live_step: f64,
    pub steps_per_frame: usize,
    pub kicad_cli: Option<PathBuf>,
    pub fabrication_preset: eda_storage::fabrication::ManufacturingPreset,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            language: LanguagePreference::System,
            engine: SimulationEngine::Mixed,
            recent_projects: Vec::new(),
            voltage_colors: true,
            current_dots: true,
            voltage_scale: 5.0,
            live_step: 1e-5,
            steps_per_frame: 32,
            kicad_cli: None,
            fabrication_preset: Default::default(),
        }
    }
}
impl Settings {
    pub const MAX_RECENT_PROJECTS: usize = 10;

    pub fn remember_project(&mut self, path: PathBuf) {
        let path = path.canonicalize().unwrap_or(path);
        self.recent_projects
            .retain(|existing| !same_path(existing, &path));
        self.recent_projects.insert(0, path);
        self.recent_projects.truncate(Self::MAX_RECENT_PROJECTS);
    }

    pub fn forget_project(&mut self, path: &std::path::Path) {
        self.recent_projects
            .retain(|existing| !same_path(existing, path));
    }

    pub fn validate(&self) -> bool {
        (0.001..=1e6).contains(&self.voltage_scale)
            && (1e-12..=1.0).contains(&self.live_step)
            && (1..=256).contains(&self.steps_per_frame)
            && self.recent_projects.len() <= Self::MAX_RECENT_PROJECTS
    }
    pub fn path() -> Result<PathBuf, String> {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
            .ok_or("Pasta de configurações não encontrada.")?;
        Ok(base.join("ProtoCraft").join("settings.json"))
    }
    pub fn load() -> Result<Self, String> {
        Self::load_at(&Self::path()?)
    }
    fn load_at(path: &std::path::Path) -> Result<Self, String> {
        let file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e.to_string()),
        };
        let mut bytes = Vec::new();
        file.take(32769)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 32768 {
            return Err("Configurações excedem 32 KiB.".into());
        }
        let settings: Self = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if !settings.validate() {
            return Err("Valores de configuração inválidos.".into());
        }
        Ok(settings)
    }
    pub fn save(&self) -> Result<(), String> {
        self.save_at(&Self::path()?)
    }
    fn save_at(&self, path: &std::path::Path) -> Result<(), String> {
        if !self.validate() {
            return Err("Valores de configuração inválidos.".into());
        }
        let parent = path.parent().ok_or("Pasta de configurações inválida.")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        file.as_file().sync_all().map_err(|e| e.to_string())?;
        file.persist(path).map_err(|e| e.error.to_string())?;
        Ok(())
    }
}

fn same_path(left: &std::path::Path, right: &std::path::Path) -> bool {
    #[cfg(windows)]
    {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_engine_persists_and_invalid_settings_are_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let mut settings = Settings {
            engine: SimulationEngine::Dynamic,
            ..Default::default()
        };
        settings.save_at(&path).unwrap();
        assert_eq!(
            Settings::load_at(&path).unwrap().engine,
            SimulationEngine::Dynamic
        );
        settings.engine = SimulationEngine::Stable;
        settings.save_at(&path).unwrap();
        assert_eq!(
            Settings::load_at(&path).unwrap().engine,
            SimulationEngine::Stable
        );
        let before = std::fs::read(&path).unwrap();
        settings.live_step = f64::NAN;
        assert!(settings.save_at(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        std::fs::write(&path, r#"{"engine":"unknown"}"#).unwrap();
        assert!(Settings::load_at(&path).is_err());
        std::fs::write(&path, vec![b' '; 32769]).unwrap();
        assert!(Settings::load_at(&path).is_err());
    }
}
