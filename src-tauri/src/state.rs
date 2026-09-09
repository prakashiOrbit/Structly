use db_core::DbConnection;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AppState {
    /// App metadata (saved connections, history, favorites) — never secrets.
    pub storage: SqlitePool,
    /// Live, already-authenticated connections, keyed by connection id.
    ///
    /// `Arc`, not `Box`: commands must clone the connection out and drop this map's
    /// lock *before* awaiting on it. A long-running `execute()` would otherwise hold
    /// the whole map locked, and a `cancel` command for that same connection could
    /// never acquire the lock to reach it.
    pub live_connections: Mutex<HashMap<String, Arc<dyn DbConnection>>>,
}
