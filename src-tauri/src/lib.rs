mod analytics;
mod bank;
mod commands;
mod import;
mod ledger;
mod planning;
mod schema;
mod sync;
mod vault;

use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub vault: Mutex<Option<vault::Vault>>,
    pub bank_code: std::sync::Arc<Mutex<Option<String>>>,
    pub path: std::path::PathBuf,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(mobile)]
    let builder = builder.plugin(tauri_plugin_barcode_scanner::init()).plugin(tauri_plugin_keystore::init());
    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            app.manage(AppState { vault: Mutex::new(None), bank_code: Default::default(), path: dir.join("cereware-finances.cwf") });
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
            commands::account_create,
            commands::account_update,
            commands::account_delete,
            commands::import_preview,
            commands::import_commit,
            commands::tx_list,
            commands::tx_update,
            commands::tx_create,
            commands::tx_delete,
            commands::tx_splits,
            commands::tx_split,
            commands::rules_list,
            commands::rule_create,
            commands::rule_delete,
            commands::dashboard_full,
            commands::budgets_list,
            commands::budget_set,
            commands::report_categories,
            commands::report_flows,
            commands::recurring_list,
            commands::recurring_upsert,
            commands::recurring_set_active,
            commands::recurring_delete,
            commands::goals_list,
            commands::goal_upsert,
            commands::goal_contribute,
            commands::goal_delete,
            commands::forecast,
            commands::sub_alerts,
            commands::sub_alerts_dismiss,
            commands::business_year,
            commands::vault_change_password,
            commands::backup_export,
            commands::backup_restore,
            commands::export_csv,
            commands::category_upsert,
            commands::category_delete,
            commands::bank_config_get,
            commands::bank_config_set,
            commands::bank_aspsps,
            commands::bank_connect_start,
            commands::bank_connect_poll,
            commands::bank_connect_finish,
            commands::bank_sync,
            commands::bank_linked,
            commands::bank_sync_account,
            commands::uncategorized_groups,
            commands::categorize_group,
            commands::rules_import,
            commands::app_platform,
            commands::sync_status,
            commands::sync_code,
            commands::sync_now,
            commands::sync_enable,
            commands::sync_join,
            commands::sync_disable,
            commands::vault_verify,
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar CereWare Finances");
}
