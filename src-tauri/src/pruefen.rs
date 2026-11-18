//! Commands der Prüfansicht (Meilenstein 5).

use std::sync::Arc;

use godgrader_core::ewh::Erwartungshorizont;
use godgrader_core::ki::pipeline::{self, Ereignis, Steuerung};
use godgrader_core::ki::speicher::{SchrittArt, SchrittStatus};
use godgrader_core::projekt::Projekt;
use godgrader_core::pruefung::{ProtokollEintrag, PruefDaten};
use godgrader_core::{Error, Result};
use tauri::ipc::Response;
use tauri::State;

use crate::state::AppState;

fn mit_ewh(state: &AppState, project_id: &str) -> Result<(Projekt, Erwartungshorizont)> {
    let p = Projekt::open(&state.paths, project_id)?;
    let ewh = p
        .ewh()?
        .ok_or_else(|| Error::Ungueltig("Kein Erwartungshorizont im Projekt.".into()))?;
    Ok((p, ewh))
}

#[tauri::command]
pub fn pruefdaten(
    state: State<'_, AppState>,
    project_id: String,
    klausur_id: String,
) -> Result<PruefDaten> {
    let (p, ewh) = mit_ewh(&state, &project_id)?;
    p.pruefdaten(&klausur_id, &ewh)
}

#[tauri::command]
pub fn be_setzen(
    state: State<'_, AppState>,
    project_id: String,
    klausur_id: String,
    element_id: String,
    be: f64,
    bestaetigt: bool,
) -> Result<PruefDaten> {
    let (p, ewh) = mit_ewh(&state, &project_id)?;
    p.be_setzen(&klausur_id, &ewh, &element_id, be, bestaetigt)?;
    p.pruefdaten(&klausur_id, &ewh)
}

#[tauri::command]
pub fn bestaetigung_aufheben(
    state: State<'_, AppState>,
    project_id: String,
    klausur_id: String,
    element_id: String,
) -> Result<PruefDaten> {
    let (p, ewh) = mit_ewh(&state, &project_id)?;
    p.bestaetigung_aufheben(&klausur_id, &element_id)?;
    p.pruefdaten(&klausur_id, &ewh)
}

#[tauri::command]
pub fn freigeben(
    state: State<'_, AppState>,
    project_id: String,
    klausur_id: String,
) -> Result<PruefDaten> {
    let (p, ewh) = mit_ewh(&state, &project_id)?;
    p.freigeben(&klausur_id, &ewh)?;
    p.pruefdaten(&klausur_id, &ewh)
}

#[tauri::command]
pub fn freigabe_zuruecknehmen(
    state: State<'_, AppState>,
    project_id: String,
    klausur_id: String,
) -> Result<PruefDaten> {
    let (p, ewh) = mit_ewh(&state, &project_id)?;
    p.freigabe_zuruecknehmen(&klausur_id)?;
    p.pruefdaten(&klausur_id, &ewh)
}

#[tauri::command]
pub fn abschrift_speichern(
    state: State<'_, AppState>,
    project_id: String,
    klausur_id: String,
    aufgabe: String,
    text: String,
) -> Result<()> {
    Projekt::open(&state.paths, &project_id)?.abschrift_aendern(&klausur_id, &aufgabe, &text)
}

#[tauri::command]
pub fn protokoll(
    state: State<'_, AppState>,
    project_id: String,
    klausur_id: String,
) -> Result<Vec<ProtokollEintrag>> {
    Projekt::open(&state.paths, &project_id)?.protokoll(&klausur_id)
}

/// Seitenbild als rohe JPEG-Bytes (kommt im Browser als ArrayBuffer an).
#[tauri::command]
pub fn seitenbild(
    state: State<'_, AppState>,
    project_id: String,
    seite_id: String,
) -> Result<Response> {
    let p = Projekt::open(&state.paths, &project_id)?;
    Ok(Response::new(std::fs::read(p.seitenbild(&seite_id)?)?))
}

/// Bewertet eine Aufgabe neu (nach Änderung der Abschrift). Setzt die
/// Bestätigungen dieser Aufgabe zurück.
#[tauri::command]
pub async fn aufgabe_neu_bewerten(
    state: State<'_, AppState>,
    project_id: String,
    klausur_id: String,
    aufgabe: String,
) -> Result<PruefDaten> {
    let konfig = crate::ki::konfig(&state)?;
    let client = state.client(true)?;
    let (p, _) = mit_ewh(&state, &project_id)?;
    if p.klausur_status(&klausur_id)? == godgrader_core::scans::KlausurStatus::Freigegeben {
        return Err(Error::Ungueltig(
            "Diese Klausur ist freigegeben. Bitte zuerst die Freigabe zurücknehmen.".into(),
        ));
    }
    let st = Steuerung::neu();
    {
        let mut l = state.laeufe.lock().expect("Sperre");
        if l.contains_key(&project_id) {
            return Err(Error::Ungueltig(
                "Gerade läuft eine Verarbeitung. Bitte warten oder sie unter „Verarbeitung“ anhalten.".into(),
            ));
        }
        l.insert(project_id.clone(), st.clone());
    }
    p.schritte_loeschen(&klausur_id, SchrittArt::Bewerten, Some(&aufgabe))?;
    if konfig.slots.zweitmeinung.enabled {
        p.schritte_loeschen(&klausur_id, SchrittArt::Zweitmeinung, Some(&aufgabe))?;
    }
    let res = pipeline::ausfuehren(
        p,
        client,
        konfig,
        vec![klausur_id.clone()],
        st,
        Arc::new(|_e: Ereignis| {}),
    )
    .await;
    state.laeufe.lock().expect("Sperre").remove(&project_id);
    res?;
    let (p, ewh) = mit_ewh(&state, &project_id)?;
    if let Some(s) = p.schritt(&klausur_id, SchrittArt::Bewerten, &aufgabe)? {
        if s.status == SchrittStatus::Fehler {
            return Err(Error::Ungueltig(
                s.fehler
                    .unwrap_or_else(|| "Bewertung fehlgeschlagen.".into()),
            ));
        }
    }
    p.pruefdaten(&klausur_id, &ewh)
}
