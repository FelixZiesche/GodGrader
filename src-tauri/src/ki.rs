//! Commands für die KI-Verarbeitung (Meilenstein 4).

use std::sync::Arc;

use godgrader_core::ewh::{Element, Erwartungshorizont};
use godgrader_core::ki::ewh_hilfe;
use godgrader_core::ki::pipeline::{self, Ereignis, LaufErgebnis, PipelineKonfig, Steuerung};
use godgrader_core::ki::prompts::{self, PromptArt, PromptVorlage, Vorlagen};
use godgrader_core::ki::schaetzung::{self, Schaetzung};
use godgrader_core::ki::speicher::Aufruf;
use godgrader_core::ki::uebersicht::{self, Uebersicht};
use godgrader_core::models::ModelCache;
use godgrader_core::projekt::Projekt;
use godgrader_core::slots::SlotKind;
use godgrader_core::{Error, Result};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::state::AppState;

fn modelle(state: &AppState) -> Vec<godgrader_core::models::ModelInfo> {
    ModelCache::load(&state.paths)
        .ok()
        .flatten()
        .map(|c| c.models)
        .unwrap_or_default()
}

pub(crate) fn konfig(state: &AppState) -> Result<PipelineKonfig> {
    let s = state.settings();
    Ok(PipelineKonfig {
        slots: s.slots,
        modelle: modelle(state),
        zdr_only: s.zdr_only,
        parallel: s.parallelism as usize,
        vorlagen: Vorlagen::laden(&state.paths)?,
    })
}

fn laeuft(state: &AppState, project_id: &str) -> bool {
    state
        .laeufe
        .lock()
        .expect("Sperre")
        .contains_key(project_id)
}

#[tauri::command]
pub fn pipeline_uebersicht(state: State<'_, AppState>, project_id: String) -> Result<Uebersicht> {
    let p = Projekt::open(&state.paths, &project_id)?;
    let laeuft_jetzt = laeuft(&state, &project_id);
    if !laeuft_jetzt {
        // Reste eines abgestürzten Laufs
        p.laufende_schritte_zuruecksetzen()?;
    }
    let ewh = p.ewh()?;
    let mut u = uebersicht::uebersicht(
        &p,
        ewh.as_ref(),
        state.settings().slots.zweitmeinung.enabled,
    )?;
    u.laeuft = laeuft_jetzt;
    Ok(u)
}

#[tauri::command]
pub fn kosten_schaetzen(
    state: State<'_, AppState>,
    project_id: String,
    klausur_ids: Vec<String>,
) -> Result<Schaetzung> {
    let p = Projekt::open(&state.paths, &project_id)?;
    let ewh = p.ewh()?.ok_or_else(|| {
        Error::Ungueltig("Bitte zuerst einen Erwartungshorizont importieren.".into())
    })?;
    schaetzung::schaetzen(
        &p,
        &ewh,
        &state.settings().slots,
        &modelle(&state),
        &klausur_ids,
    )
}

#[derive(Clone, Serialize)]
struct PipelineMeldung {
    projekt_id: String,
    ereignis: Ereignis,
}

#[derive(Clone, Serialize)]
struct PipelineEnde {
    projekt_id: String,
    ergebnis: Option<LaufErgebnis>,
    fehler: Option<String>,
}

#[tauri::command]
pub fn pipeline_starten(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    klausur_ids: Vec<String>,
) -> Result<()> {
    let konfig = konfig(&state)?;
    let client = state.client(true)?;
    let p = Projekt::open(&state.paths, &project_id)?;
    pipeline::pruefen(&p, &konfig)?;
    let st = Steuerung::neu();
    {
        let mut l = state.laeufe.lock().expect("Sperre");
        if l.contains_key(&project_id) {
            return Err(Error::Ungueltig(
                "Für dieses Projekt läuft bereits eine Verarbeitung.".into(),
            ));
        }
        l.insert(project_id.clone(), st.clone());
    }
    let pid = project_id.clone();
    let app2 = app.clone();
    let beob = Arc::new(move |e: Ereignis| {
        let _ = app2.emit(
            "pipeline-ereignis",
            PipelineMeldung {
                projekt_id: pid.clone(),
                ereignis: e,
            },
        );
    });
    tauri::async_runtime::spawn(async move {
        let res = pipeline::ausfuehren(p, client, konfig, klausur_ids, st, beob).await;
        if let Some(state) = tauri::Manager::try_state::<AppState>(&app) {
            state.laeufe.lock().expect("Sperre").remove(&project_id);
        }
        let (ergebnis, fehler) = match res {
            Ok(e) => (Some(e), None),
            Err(e) => (None, Some(e.to_string())),
        };
        let _ = app.emit(
            "pipeline-ende",
            PipelineEnde {
                projekt_id: project_id,
                ergebnis,
                fehler,
            },
        );
    });
    Ok(())
}

#[tauri::command]
pub fn pipeline_anhalten(state: State<'_, AppState>, project_id: String) {
    if let Some(st) = state.laeufe.lock().expect("Sperre").get(&project_id) {
        st.anhalten();
    }
}

#[tauri::command]
pub fn pipeline_abbrechen(state: State<'_, AppState>, project_id: String) {
    if let Some(st) = state.laeufe.lock().expect("Sperre").get(&project_id) {
        st.abbrechen();
    }
}

#[tauri::command]
pub fn aufrufe(
    state: State<'_, AppState>,
    project_id: String,
    klausur_id: Option<String>,
) -> Result<Vec<Aufruf>> {
    Projekt::open(&state.paths, &project_id)?.aufrufe(klausur_id.as_deref())
}

#[tauri::command]
pub fn prompts_lesen(state: State<'_, AppState>) -> Result<Vec<PromptVorlage>> {
    prompts::alle(&state.paths)
}

#[tauri::command]
pub fn prompt_speichern(state: State<'_, AppState>, art: PromptArt, text: String) -> Result<()> {
    prompts::speichern(&state.paths, art, &text)
}

#[tauri::command]
pub fn prompt_zuruecksetzen(state: State<'_, AppState>, art: PromptArt) -> Result<()> {
    prompts::zuruecksetzen(&state.paths, art)
}

/// „KI-Hilfe beim Import“ für eine Aufgabe. Die Kosten werden im Projekt verbucht.
#[tauri::command]
pub async fn ki_hilfe_ewh(
    state: State<'_, AppState>,
    project_id: String,
    ewh: Erwartungshorizont,
    nr: String,
) -> Result<Vec<Element>> {
    let aufgabe = ewh
        .aufgaben
        .iter()
        .find(|a| a.nr == nr)
        .ok_or_else(|| Error::Ungueltig(format!("Aufgabe {nr} nicht gefunden.")))?
        .clone();
    let s = state.settings();
    let modelle = modelle(&state);
    let info = modelle.iter().find(|m| m.id == s.slots.bewerten.model);
    let halbe = Projekt::open(&state.paths, &project_id)?.meta()?.halbe_be;
    let vorlage = prompts::laden(&state.paths, PromptArt::EwhHilfe)?;
    let client = state.client(true)?;
    let start = std::time::Instant::now();
    let (elemente, nutzung) = ewh_hilfe::gliedern(
        &client,
        s.slots.get(SlotKind::Bewerten),
        info,
        &vorlage,
        &aufgabe,
        halbe,
        s.zdr_only,
    )
    .await?;
    Projekt::open(&state.paths, &project_id)?.aufruf_protokollieren(&Aufruf {
        klausur_id: None,
        art: "ewh_hilfe".into(),
        aufgabe: Some(nr),
        modell: s.slots.bewerten.model.clone(),
        tokens_ein: nutzung.tokens_ein,
        tokens_aus: nutzung.tokens_aus,
        kosten: nutzung.kosten,
        geschaetzt: nutzung.geschaetzt,
        dauer_ms: start.elapsed().as_millis() as u64,
        ok: true,
    })?;
    Ok(elemente)
}
