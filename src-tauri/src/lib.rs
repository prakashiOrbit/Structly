mod commands;
mod state;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = handle.path().app_data_dir().expect("resolve app data dir");
            std::fs::create_dir_all(&data_dir).expect("create app data dir");
            let db_path = data_dir.join("structly.sqlite");

            let storage = tauri::async_runtime::block_on(app_storage::init(
                db_path.to_str().expect("app data dir path is valid utf-8"),
            ))
            .expect("init local metadata store");

            handle.manage(AppState {
                storage,
                live_connections: Default::default(),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_engines,
            commands::list_saved_connections,
            commands::save_connection,
            commands::delete_connection,
            commands::connect,
            commands::disconnect,
            commands::execute_query,
            commands::cancel_query,
            commands::list_schemas,
            commands::list_tables,
            commands::table_columns,
            commands::table_indexes,
            commands::list_foreign_keys,
            commands::list_history,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
