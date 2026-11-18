//! Ablage im App-Datenverzeichnis. Die Tauri-Schicht ermittelt den
//! plattformüblichen Ordner (Windows: `%APPDATA%\GodGrader`) und übergibt ihn.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppPaths {
    root: PathBuf,
}

impl AppPaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn settings_file(&self) -> PathBuf {
        self.root.join("einstellungen.json")
    }

    pub fn models_cache(&self) -> PathBuf {
        self.root.join("cache").join("modelle.json")
    }

    pub fn projects_dir(&self) -> PathBuf {
        self.root.join("projekte")
    }

    pub fn prompts_dir(&self) -> PathBuf {
        self.root.join("prompts")
    }

    pub fn ensure(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.projects_dir())?;
        std::fs::create_dir_all(self.prompts_dir())?;
        std::fs::create_dir_all(self.root.join("cache"))?;
        Ok(())
    }
}
