//! Commands für Feedback, Statistik und Exporte (Meilenstein 6).

use godgrader_core::export::ausgabe::{self, ExportBericht, ExportOptionen};
use godgrader_core::export::statistik::{self, KlausurZeile, Statistik};
use godgrader_core::ki::feedback::{self, FeedbackErgebnis, FeedbackZeile};
use godgrader_core::projekt::Projekt;
use godgrader_core::scans::KlausurStatus;
use godgrader_core::{Error, Result};
use serde::Serialize;
use tauri::State;

use crate::state::AppState;

#[derive(Serialize)]
pub struct StatistikAntwort {
    zeilen: Vec<KlausurZeile>,
    statistik: Statistik,
    label: &'static str,
    aufgaben: Vec<String>,
}

#[tauri::command]
pub fn statistik(state: State<'_, AppState>, project_id: String) -> Result<StatistikAntwort> {
    let p = Projekt::open(&state.paths, &project_id)?;
    let ewh = p
        .ewh()?
        .ok_or_else(|| Error::Ungueltig("Kein Erwartungshorizont im Projekt.".into()))?;
    let (zeilen, statistik) = statistik::berechnen(&p, &ewh)?;
    Ok(StatistikAntwort {
        zeilen,
        statistik,
        label: ausgabe::bezeichnungs_label(&p)?,
        aufgaben: ewh.aufgabennummern(),
    })
}

#[derive(Serialize)]
pub struct FeedbackEintrag {
    klausur_id: String,
    bezeichnung: String,
    status: KlausurStatus,
    feedback: Option<FeedbackZeile>,
}

#[tauri::command]
pub fn feedback_liste(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<FeedbackEintrag>> {
    let p = Projekt::open(&state.paths, &project_id)?;
    p.klausuren()?
        .into_iter()
        .map(|k| {
            Ok(FeedbackEintrag {
                feedback: p.feedback(&k.id)?,
                klausur_id: k.id,
                bezeichnung: k.bezeichnung,
                status: k.status,
            })
        })
        .collect()
}

#[tauri::command]
pub async fn feedback_erzeugen(
    state: State<'_, AppState>,
    project_id: String,
    klausur_ids: Vec<String>,
) -> Result<FeedbackErgebnis> {
    let konfig = crate::ki::konfig(&state)?;
    let client = state.client(true)?;
    let p = Projekt::open(&state.paths, &project_id)?;
    feedback::erzeugen(p, client, konfig, klausur_ids).await
}

#[tauri::command]
pub fn feedback_speichern(
    state: State<'_, AppState>,
    project_id: String,
    klausur_id: String,
    text: String,
) -> Result<()> {
    Projekt::open(&state.paths, &project_id)?.feedback_speichern(&klausur_id, &text, false)
}

#[tauri::command]
pub async fn exportieren(
    state: State<'_, AppState>,
    project_id: String,
    ordner: String,
    optionen: ExportOptionen,
) -> Result<ExportBericht> {
    let paths = state.paths.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let p = Projekt::open(&paths, &project_id)?;
        ausgabe::exportieren(&p, std::path::Path::new(&ordner), &optionen)
    })
    .await
    .map_err(|e| Error::Ungueltig(e.to_string()))?
}

/// Öffnet den Exportordner im Dateimanager. Nur Ordner, keine Dateien.
#[tauri::command]
pub fn ordner_oeffnen(app: tauri::AppHandle, ordner: String) -> Result<()> {
    use tauri_plugin_opener::OpenerExt;
    if !std::path::Path::new(&ordner).is_dir() {
        return Err(Error::Ungueltig("Der Ordner existiert nicht.".into()));
    }
    app.opener()
        .open_path(ordner, None::<&str>)
        .map_err(|e| Error::Ungueltig(e.to_string()))
}
