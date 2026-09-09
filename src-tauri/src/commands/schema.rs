use crate::commands::query::get_connection;
use crate::state::AppState;
use db_core::{ColumnMeta, ForeignKeyMeta, IndexMeta, SchemaMeta, TableMeta};
use tauri::State;

#[tauri::command]
pub async fn list_schemas(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<Vec<SchemaMeta>, String> {
    let conn = get_connection(&state, &connection_id).await?;
    conn.list_schemas().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_tables(
    state: State<'_, AppState>,
    connection_id: String,
    schema: String,
) -> Result<Vec<TableMeta>, String> {
    let conn = get_connection(&state, &connection_id).await?;
    conn.list_tables(&schema).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn table_columns(
    state: State<'_, AppState>,
    connection_id: String,
    schema: String,
    table: String,
) -> Result<Vec<ColumnMeta>, String> {
    let conn = get_connection(&state, &connection_id).await?;
    conn.table_columns(&schema, &table)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn table_indexes(
    state: State<'_, AppState>,
    connection_id: String,
    schema: String,
    table: String,
) -> Result<Vec<IndexMeta>, String> {
    let conn = get_connection(&state, &connection_id).await?;
    conn.table_indexes(&schema, &table)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_foreign_keys(
    state: State<'_, AppState>,
    connection_id: String,
    schema: String,
) -> Result<Vec<ForeignKeyMeta>, String> {
    let conn = get_connection(&state, &connection_id).await?;
    conn.list_foreign_keys(&schema)
        .await
        .map_err(|e| e.to_string())
}
