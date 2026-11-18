use godgrader_core::models::ModelCache;
use godgrader_core::openrouter::KeyInfo;
use godgrader_core::secrets::mask;
use godgrader_core::settings::AppSettings;
use godgrader_core::slots::{self, SlotCheck, SlotKind, SlotSet};
use godgrader_core::{Error, Result};
use serde::Serialize;
use tauri::State;

use crate::state::AppState;

#[derive(Serialize)]
pub struct AppInfo {
    version: &'static str,
    data_dir: String,
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        data_dir: state.paths.root().display().to_string(),
    }
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppSettings {
    state.settings()
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: AppSettings) -> Result<AppSettings> {
    settings.save(&state.paths)?;
    let saved = AppSettings::load(&state.paths)?;
    *state.settings.lock().expect("Sperre") = saved.clone();
    Ok(saved)
}

#[tauri::command]
pub fn ack_hinweis(state: State<'_, AppState>) -> Result<()> {
    let mut s = state.settings();
    s.hinweis_bestaetigt = true;
    save_settings(state, s).map(|_| ())
}

#[derive(Serialize)]
pub struct KeyStatus {
    present: bool,
    masked: Option<String>,
    error: Option<String>,
}

#[tauri::command]
pub fn api_key_status(state: State<'_, AppState>) -> KeyStatus {
    match state.secrets.get() {
        Ok(k) => KeyStatus {
            present: k.is_some(),
            masked: k.as_deref().map(mask),
            error: None,
        },
        Err(e) => KeyStatus {
            present: false,
            masked: None,
            error: Some(e.to_string()),
        },
    }
}

#[tauri::command]
pub fn set_api_key(state: State<'_, AppState>, key: String) -> Result<KeyStatus> {
    state.secrets.set(&key)?;
    Ok(api_key_status(state))
}

#[tauri::command]
pub fn delete_api_key(state: State<'_, AppState>) -> Result<KeyStatus> {
    state.secrets.delete()?;
    Ok(api_key_status(state))
}

#[tauri::command]
pub async fn check_connection(state: State<'_, AppState>) -> Result<KeyInfo> {
    let client = state.client(true)?;
    client.check_key().await
}

#[tauri::command]
pub fn get_models(state: State<'_, AppState>) -> Result<Option<ModelCache>> {
    ModelCache::load(&state.paths)
}

#[tauri::command]
pub async fn refresh_models(state: State<'_, AppState>) -> Result<ModelCache> {
    let client = state.client(false)?;
    let mut models = client.list_models().await?;
    models.sort_by_key(|m| m.name.to_lowercase());
    let cache = ModelCache {
        fetched_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
        models,
    };
    cache.save(&state.paths)?;
    Ok(cache)
}

#[tauri::command]
pub fn check_slots(state: State<'_, AppState>) -> Result<Vec<SlotCheck>> {
    let cache = ModelCache::load(&state.paths)?.unwrap_or_default();
    Ok(slots::check_slots(&state.settings().slots, &cache.models))
}

#[tauri::command]
pub fn validate_slot_drop(
    state: State<'_, AppState>,
    kind: SlotKind,
    model_id: String,
) -> Result<()> {
    let cache = ModelCache::load(&state.paths)?.unwrap_or_default();
    let model = cache
        .get(&model_id)
        .ok_or_else(|| Error::Ungueltig("Modell nicht in der Modellliste.".into()))?;
    slots::validate_drop(kind, model).map_err(Error::Ungueltig)
}

#[tauri::command]
pub fn reset_slots(state: State<'_, AppState>) -> Result<AppSettings> {
    let mut s = state.settings();
    s.slots = SlotSet::default();
    save_settings(state, s)
}

// ---------------- Projekte ----------------

use godgrader_core::ewh::{self, validate::Warnung, Erwartungshorizont, Notenschluessel, Stufe};
use godgrader_core::projekt::{Projekt, ProjektMeta};

#[tauri::command]
pub fn list_projects(state: State<'_, AppState>) -> Result<Vec<ProjektMeta>> {
    Projekt::list(&state.paths)
}

#[tauri::command]
pub fn create_project(state: State<'_, AppState>, meta: ProjektMeta) -> Result<ProjektMeta> {
    Projekt::create(&state.paths, meta)?.meta()
}

#[tauri::command]
pub fn update_project(state: State<'_, AppState>, meta: ProjektMeta) -> Result<ProjektMeta> {
    Projekt::open(&state.paths, &meta.id)?.save_meta(&meta)
}

#[tauri::command]
pub fn delete_project(state: State<'_, AppState>, id: String) -> Result<()> {
    Projekt::delete(&state.paths, &id)
}

// ---------------- Erwartungshorizont ----------------

#[tauri::command]
pub fn get_ewh(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Option<Erwartungshorizont>> {
    Projekt::open(&state.paths, &project_id)?.ewh()
}

#[tauri::command]
pub async fn import_ewh(
    state: State<'_, AppState>,
    project_id: String,
    path: String,
) -> Result<Erwartungshorizont> {
    let p = std::path::PathBuf::from(&path);
    let mut e = tauri::async_runtime::spawn_blocking(move || ewh::import_file(&p))
        .await
        .map_err(|e| Error::Ungueltig(e.to_string()))??;
    let projekt = Projekt::open(&state.paths, &project_id)?;
    // Stufe aus dem Projekt übernehmen, falls der Kopf nichts verrät
    let meta = projekt.meta()?;
    if e.kopf.lerngruppe.is_none() && !meta.lerngruppe.is_empty() {
        e.kopf.lerngruppe = Some(meta.lerngruppe.clone());
    }
    projekt.save_ewh(&e)?;
    Ok(e)
}

#[tauri::command]
pub fn save_ewh(
    state: State<'_, AppState>,
    project_id: String,
    ewh: Erwartungshorizont,
) -> Result<()> {
    let mut ewh = ewh;
    ewh.renumber();
    Projekt::open(&state.paths, &project_id)?.save_ewh(&ewh)
}

#[tauri::command]
pub fn validate_ewh(ewh: Erwartungshorizont, halbe_be: bool) -> Vec<Warnung> {
    ewh::validate::validate(&ewh, halbe_be)
}

#[tauri::command]
pub fn berechne_notenschluessel(stufe: Stufe, gesamt: f64) -> Notenschluessel {
    Notenschluessel::berechnen(stufe, gesamt)
}
