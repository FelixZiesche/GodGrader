//! Dünne Tauri-Schicht: Commands und Zustand. Die Fachlogik liegt in
//! `godgrader-core`.

mod commands;
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
        ])
        .run(tauri::generate_context!())
        .expect("GodGrader konnte nicht gestartet werden");
}
