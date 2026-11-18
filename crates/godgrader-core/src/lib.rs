//! Fachlogik von GodGrader.
//!
//! Diese Crate kennt weder Tauri noch Windows-spezifische APIs. Die Desktop-App
//! (`src-tauri`) ist nur eine dünne Schicht darüber, damit später Linux und
//! Android ohne Umbau ergänzt werden können.

pub mod error;
pub mod ewh;
pub mod models;
pub mod openrouter;
pub mod paths;
pub mod pdfium;
pub mod projekt;
pub mod secrets;
pub mod settings;
pub mod slots;
pub mod storage;

pub use error::{Error, Result};

/// Name der Anwendung, u. a. für den Header `X-Title` und den Schlüsselbund.
pub const APP_NAME: &str = "GodGrader";
