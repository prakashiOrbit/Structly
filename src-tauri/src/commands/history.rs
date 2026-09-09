use crate::state::AppState;
use app_storage::HistoryEntry;
use tauri::State;

#[tauri::command]
pub async fn list_history(
    state: State<'_, AppState>,
    connection_id: String,
    limit: i64,
) -> Result<Vec<HistoryEntry>, String> {
    app_storage::list_history(&state.storage, &connection_id, limit)
        .await
        .map_err(|e| e.to_string())
}
