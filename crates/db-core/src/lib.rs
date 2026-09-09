//! Engine-agnostic contract that every database driver (postgres, mysql, sqlite, ...)
//! implements. The rest of the app (Tauri commands, query engine) talks to `dyn DbConnection`
//! and never needs to know which engine it's actually pointed at.
mod types;

pub use types::*;

use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("connection error: {0}")]
    Connection(String),
    #[error("query error: {0}")]
    Query(String),
    #[error("operation not supported on this engine: {0}")]
    Unsupported(String),
}

pub type DbResult<T> = Result<T, DbError>;

/// The secrets `connect()` needs, looked up from the OS keychain by the Tauri command layer
/// and passed down — never embedded in `ConnectionConfig` itself, which travels over IPC as
/// plain JSON. Bundled into one struct (rather than two `Option<&str>` params) so call
/// sites can't accidentally swap the DB password and the SSH secret.
#[derive(Debug, Clone, Copy, Default)]
pub struct ConnectSecrets<'a> {
    pub db_password: Option<&'a str>,
    /// Password or private-key passphrase for `ConnectionConfig.ssh_tunnel`, per its
    /// `SshAuthMethod`. Ignored if `ssh_tunnel` is `None`.
    pub ssh_secret: Option<&'a str>,
}

#[async_trait]
pub trait DatabaseDriver: Send + Sync {
    fn engine(&self) -> Engine;
    async fn connect(
        &self,
        config: &ConnectionConfig,
        secrets: ConnectSecrets<'_>,
    ) -> DbResult<Box<dyn DbConnection>>;
}

#[async_trait]
pub trait DbConnection: Send + Sync {
    async fn execute(&self, sql: &str) -> DbResult<QueryResult>;
    async fn list_schemas(&self) -> DbResult<Vec<SchemaMeta>>;
    async fn list_tables(&self, schema: &str) -> DbResult<Vec<TableMeta>>;
    async fn table_columns(&self, schema: &str, table: &str) -> DbResult<Vec<ColumnMeta>>;
    async fn table_indexes(&self, schema: &str, table: &str) -> DbResult<Vec<IndexMeta>>;
    /// All foreign keys in the schema (not just one table) — SchemaDiagram wants a
    /// whole-schema ERD, and fetching them all at once avoids an N+1 query per table.
    async fn list_foreign_keys(&self, schema: &str) -> DbResult<Vec<ForeignKeyMeta>>;
    /// Cancel an in-flight query on this connection. Each engine has its own protocol for
    /// this (Postgres needs a second connection to call pg_cancel_backend, MySQL issues
    /// KILL QUERY on another session) so it cannot be a default method here.
    async fn cancel(&self) -> DbResult<()>;
    async fn close(&self) -> DbResult<()>;
}
