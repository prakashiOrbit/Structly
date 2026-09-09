use crate::commands::query::ssh_secret_account;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn list_engines() -> Vec<&'static str> {
    vec!["postgres", "mysql", "sqlite"]
}

#[tauri::command]
pub async fn list_saved_connections(
    state: State<'_, AppState>,
) -> Result<Vec<app_storage::SavedConnection>, String> {
    app_storage::list_connections(&state.storage)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_connection(
    state: State<'_, AppState>,
    connection: app_storage::SavedConnection,
    password: Option<String>,
    ssh_secret: Option<String>,
) -> Result<(), String> {
    if let Some(pw) = password {
        secrets::set_secret(&connection.id, &pw).map_err(|e| e.to_string())?;
    }
    if let Some(secret) = ssh_secret {
        secrets::set_secret(&ssh_secret_account(&connection.id), &secret)
            .map_err(|e| e.to_string())?;
    }
    app_storage::save_connection(&state.storage, &connection)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_connection(state: State<'_, AppState>, id: String) -> Result<(), String> {
    secrets::delete_secret(&id).map_err(|e| e.to_string())?;
    secrets::delete_secret(&ssh_secret_account(&id)).map_err(|e| e.to_string())?;
    app_storage::delete_connection(&state.storage, &id)
        .await
        .map_err(|e| e.to_string())
}
