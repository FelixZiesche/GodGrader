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

// ---------------- Scans ----------------

use godgrader_core::scans::{
    self, Fortschritt, ImportAuftrag, KlausurInfo, ScanOptionen, Schwaerzung,
};
use tauri::{AppHandle, Emitter};

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| Error::Ungueltig(e.to_string()))?
}

#[derive(Serialize)]
pub struct PdfInfo {
    seiten: u32,
    name: String,
}

#[tauri::command]
pub async fn pdf_info(path: String) -> Result<PdfInfo> {
    blocking(move || {
        let p = std::path::PathBuf::from(&path);
        Ok(PdfInfo {
            seiten: scans::seitenzahl(&p)?,
            name: p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        })
    })
    .await
}

#[tauri::command]
pub async fn pdf_miniaturen(path: String, breite: i32) -> Result<Vec<String>> {
    blocking(move || scans::miniaturen(std::path::Path::new(&path), breite.clamp(60, 600))).await
}

#[tauri::command]
pub async fn pdf_seitenvorschau(path: String, seite: u32, breite: i32) -> Result<String> {
    blocking(move || {
        scans::seitenvorschau(std::path::Path::new(&path), seite, breite.clamp(200, 2000))
    })
    .await
}

#[tauri::command]
pub fn klausuren(state: State<'_, AppState>, project_id: String) -> Result<Vec<KlausurInfo>> {
    Projekt::open(&state.paths, &project_id)?.klausuren()
}

#[tauri::command]
pub async fn scans_importieren(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    auftraege: Vec<ImportAuftrag>,
    optionen: ScanOptionen,
) -> Result<Vec<KlausurInfo>> {
    let paths = state.paths.clone();
    blocking(move || {
        let p = Projekt::open(&paths, &project_id)?;
        p.scans_importieren(&auftraege, &optionen, |f: Fortschritt| {
            let _ = app.emit("scan-fortschritt", f);
        })
    })
    .await
}

#[tauri::command]
pub async fn seiten_neu_erzeugen(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
) -> Result<()> {
    let paths = state.paths.clone();
    blocking(move || {
        Projekt::open(&paths, &project_id)?.seiten_neu_erzeugen(|f: Fortschritt| {
            let _ = app.emit("scan-fortschritt", f);
        })
    })
    .await
}

#[tauri::command]
pub fn klausur_umbenennen(
    state: State<'_, AppState>,
    project_id: String,
    id: String,
    bezeichnung: String,
) -> Result<()> {
    Projekt::open(&state.paths, &project_id)?.klausur_umbenennen(&id, &bezeichnung)
}

#[tauri::command]
pub fn klausur_loeschen(state: State<'_, AppState>, project_id: String, id: String) -> Result<()> {
    Projekt::open(&state.paths, &project_id)?.klausur_loeschen(&id)
}

#[tauri::command]
pub async fn seiten_miniatur(
    state: State<'_, AppState>,
    project_id: String,
    seite_id: String,
    breite: u32,
) -> Result<String> {
    let paths = state.paths.clone();
    blocking(move || {
        Projekt::open(&paths, &project_id)?.seiten_miniatur(&seite_id, breite.clamp(60, 1600))
    })
    .await
}

#[tauri::command]
pub async fn beispielseite(
    state: State<'_, AppState>,
    project_id: String,
    breite: i32,
) -> Result<Option<String>> {
    let paths = state.paths.clone();
    blocking(move || Projekt::open(&paths, &project_id)?.beispielseite(breite.clamp(200, 2000)))
        .await
}

#[tauri::command]
pub fn get_schwaerzung(state: State<'_, AppState>, project_id: String) -> Result<Vec<Schwaerzung>> {
    Projekt::open(&state.paths, &project_id)?.schwaerzung()
}

#[tauri::command]
pub fn set_schwaerzung(
    state: State<'_, AppState>,
    project_id: String,
    schwaerzung: Vec<Schwaerzung>,
) -> Result<()> {
    Projekt::open(&state.paths, &project_id)?.set_schwaerzung(&schwaerzung)
}

#[tauri::command]
pub fn get_pseudonyme(state: State<'_, AppState>, project_id: String) -> Result<Vec<String>> {
    Projekt::open(&state.paths, &project_id)?.pseudonyme()
}

#[tauri::command]
pub fn set_pseudonyme(
    state: State<'_, AppState>,
    project_id: String,
    liste: Vec<String>,
) -> Result<()> {
    Projekt::open(&state.paths, &project_id)?.set_pseudonyme(&liste)
}

#[tauri::command]
pub fn pseudonyme_importieren(
    state: State<'_, AppState>,
    project_id: String,
    path: String,
) -> Result<Vec<String>> {
    let liste = godgrader_core::pseudonyme::import(std::path::Path::new(&path))?;
    Projekt::open(&state.paths, &project_id)?.set_pseudonyme(&liste)?;
    Ok(liste)
}

#[tauri::command]
pub fn bezeichnung_vorschlaege(
    state: State<'_, AppState>,
    project_id: String,
    dateinamen: Vec<String>,
) -> Result<Vec<String>> {
    let liste = Projekt::open(&state.paths, &project_id)?.pseudonyme()?;
    Ok(dateinamen
        .iter()
        .map(|d| godgrader_core::pseudonyme::vorschlag(d, &liste))
        .collect())
}
