mod commands;
mod schema;
mod vault;

use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub vault: Mutex<Option<vault::Vault>>,
    pub path: std::path::PathBuf,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            app.manage(AppState { vault: Mutex::new(None), path: dir.join("cereware-finances.cwf") });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::vault_status,
            commands::vault_create,
            commands::vault_unlock,
            commands::vault_lock,
            commands::settings_get,
            commands::settings_set,
            commands::accounts_list,
            commands::categories_list,
            commands::dashboard_summary,
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar CereWare Finances");
}
