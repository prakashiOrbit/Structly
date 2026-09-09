use crate::state::AppState;
use db_core::{
    ConnectSecrets, ConnectionConfig, DatabaseDriver, DbConnection, Engine, QueryResult,
};
use std::sync::Arc;
use tauri::State;

/// Keychain account for a connection's SSH tunnel secret (password or key passphrase) —
/// distinct from the DB password's own entry (keyed on the plain connection id) so the two
/// never collide.
pub(crate) fn ssh_secret_account(connection_id: &str) -> String {
    format!("{connection_id}:ssh")
}

fn driver_for(engine: Engine) -> Box<dyn DatabaseDriver> {
    match engine {
        Engine::Postgres => Box::new(db_postgres::PostgresDriver),
        Engine::MySql => Box::new(db_mysql::MySqlDriver),
        Engine::Sqlite => Box::new(db_sqlite::SqliteDriver),
    }
}

/// Clones the connection out of the map and drops the lock before returning, so the
/// (possibly long-running) call the caller makes on it doesn't hold up every other
/// command — most importantly, `cancel_query` for this same connection.
pub(crate) async fn get_connection(
    state: &State<'_, AppState>,
    connection_id: &str,
) -> Result<Arc<dyn DbConnection>, String> {
    state
        .live_connections
        .lock()
        .await
        .get(connection_id)
        .cloned()
        .ok_or_else(|| format!("not connected: {connection_id}"))
}

#[tauri::command]
pub async fn connect(state: State<'_, AppState>, config: ConnectionConfig) -> Result<(), String> {
    let db_password = secrets::get_secret(&config.id).map_err(|e| e.to_string())?;
    let ssh_secret =
        secrets::get_secret(&ssh_secret_account(&config.id)).map_err(|e| e.to_string())?;
    let conn = driver_for(config.engine)
        .connect(
            &config,
            ConnectSecrets {
                db_password: db_password.as_deref(),
                ssh_secret: ssh_secret.as_deref(),
            },
        )
        .await
        .map_err(|e| e.to_string())?;
    state
        .live_connections
        .lock()
        .await
        .insert(config.id.clone(), Arc::from(conn));
    Ok(())
}

#[tauri::command]
pub async fn disconnect(state: State<'_, AppState>, connection_id: String) -> Result<(), String> {
    let removed = state.live_connections.lock().await.remove(&connection_id);
    if let Some(conn) = removed {
        conn.close().await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn execute_query(
    state: State<'_, AppState>,
    connection_id: String,
    sql: String,
) -> Result<QueryResult, String> {
    let conn = get_connection(&state, &connection_id).await?;
    let result = conn.execute(&sql).await.map_err(|e| e.to_string())?;

    app_storage::record_history(
        &state.storage,
        &connection_id,
        &sql,
        result.duration_ms as i64,
        "ok",
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(result)
}

#[tauri::command]
pub async fn cancel_query(state: State<'_, AppState>, connection_id: String) -> Result<(), String> {
    let conn = get_connection(&state, &connection_id).await?;
    conn.cancel().await.map_err(|e| e.to_string())
}
