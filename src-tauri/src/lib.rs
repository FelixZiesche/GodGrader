//! Dünne Tauri-Schicht: Commands und Zustand. Die Fachlogik liegt in
//! `godgrader-core`.

mod commands;
mod ki;
mod pruefen;
mod state;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = state::AppState::init(app.handle())?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            pruefen::pruefdaten,
            pruefen::be_setzen,
            pruefen::bestaetigung_aufheben,
            pruefen::freigeben,
            pruefen::freigabe_zuruecknehmen,
            pruefen::abschrift_speichern,
            pruefen::protokoll,
            pruefen::seitenbild,
            pruefen::aufgabe_neu_bewerten,
            ki::pipeline_uebersicht,
            ki::kosten_schaetzen,
            ki::pipeline_starten,
            ki::pipeline_anhalten,
            ki::pipeline_abbrechen,
            ki::aufrufe,
            ki::prompts_lesen,
            ki::prompt_speichern,
            ki::prompt_zuruecksetzen,
            ki::ki_hilfe_ewh,
            commands::get_settings,
            commands::save_settings,
            commands::ack_hinweis,
            commands::api_key_status,
            commands::set_api_key,
            commands::delete_api_key,
            commands::check_connection,
            commands::get_models,
            commands::refresh_models,
            commands::check_slots,
            commands::validate_slot_drop,
            commands::reset_slots,
            commands::list_projects,
            commands::create_project,
            commands::update_project,
            commands::delete_project,
            commands::get_ewh,
            commands::import_ewh,
            commands::save_ewh,
            commands::validate_ewh,
            commands::berechne_notenschluessel,
            commands::pdf_info,
            commands::pdf_miniaturen,
            commands::pdf_seitenvorschau,
            commands::klausuren,
            commands::scans_importieren,
            commands::seiten_neu_erzeugen,
            commands::klausur_umbenennen,
            commands::klausur_loeschen,
            commands::seiten_miniatur,
            commands::beispielseite,
            commands::get_schwaerzung,
            commands::set_schwaerzung,
            commands::get_pseudonyme,
            commands::set_pseudonyme,
            commands::pseudonyme_importieren,
            commands::bezeichnung_vorschlaege,
        ])
        .run(tauri::generate_context!())
        .expect("GodGrader konnte nicht gestartet werden");
}
