use std::sync::Mutex;

use godgrader_core::openrouter::{OpenRouter, DEFAULT_BASE_URL};
use godgrader_core::paths::AppPaths;
use godgrader_core::secrets::{KeyringStore, SecretStore};
use godgrader_core::settings::AppSettings;
use tauri::{AppHandle, Manager};

pub struct AppState {
    pub paths: AppPaths,
    pub settings: Mutex<AppSettings>,
    pub secrets: Box<dyn SecretStore>,
    pub base_url: String,
}

impl AppState {
    pub fn init(app: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        // Windows: %APPDATA%\GodGrader, Linux: ~/.local/share/GodGrader
        let root = app.path().data_dir()?.join(godgrader_core::APP_NAME);
        let paths = AppPaths::new(root);
        paths.ensure()?;
        let settings = AppSettings::load(&paths).unwrap_or_default();
        // PDFium liegt im Ressourcenordner der installierten App
        if let Ok(res) = app.path().resource_dir() {
            godgrader_core::pdfium::set_library_dir(res.join("pdfium"));
        }
        Ok(Self {
            paths,
            settings: Mutex::new(settings),
            secrets: Box::new(KeyringStore),
            // Nur für Entwicklung/Tests umstellbar, Standard ist OpenRouter.
            base_url: std::env::var("GODGRADER_OPENROUTER_URL")
                .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string()),
        })
    }

    pub fn settings(&self) -> AppSettings {
        self.settings.lock().expect("Sperre").clone()
    }

    pub fn client(&self, need_key: bool) -> godgrader_core::Result<OpenRouter> {
        let key = self.secrets.get()?;
        if need_key && key.is_none() {
            return Err(godgrader_core::Error::KeinSchluessel);
        }
        let s = self.settings();
        Ok(OpenRouter::new(&self.base_url, key)?
            .with_retries(s.max_retries, std::time::Duration::from_millis(1000)))
    }
}
