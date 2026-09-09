use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Engine {
    Postgres,
    MySql,
    Sqlite,
}

/// How to authenticate to the SSH bastion — never the secret itself, which (like the DB
/// password) is looked up separately from the OS keychain at connect time.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "method")]
pub enum SshAuthMethod {
    Password,
    PrivateKey { path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshTunnelConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: SshAuthMethod,
}

/// Everything needed to open a connection, minus the secret (password/key),
/// which is looked up separately from the OS keychain via `secrets` at connect time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub id: String,
    pub name: String,
    pub engine: Engine,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub database: String,
    pub username: Option<String>,
    pub ssh_tunnel: Option<SshTunnelConfig>,
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaMeta {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableMeta {
    pub schema: String,
    pub name: String,
    pub approx_row_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnMeta {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub is_primary_key: bool,
    pub default: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMeta {
    pub name: String,
    pub definition: String,
    pub is_unique: bool,
}

/// One FK relationship: `from_table.from_column` references `to_table.to_column`.
/// Scoped to a single schema (cross-schema FKs aren't represented) since that's the
/// unit `SchemaDiagram` renders one ERD for.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignKeyMeta {
    pub constraint_name: String,
    pub from_table: String,
    pub from_column: String,
    pub to_table: String,
    pub to_column: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub rows_affected: Option<u64>,
    pub duration_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Locks in the exact internally-tagged shape the frontend's `SshAuthMethod` TS type
    /// (src/lib/types.ts) assumes: a unit variant serializes as just `{"method": "..."}"`,
    /// a struct variant merges its fields in alongside the tag.
    #[test]
    fn ssh_auth_method_matches_the_shape_the_frontend_assumes() {
        assert_eq!(
            serde_json::to_string(&SshAuthMethod::Password).unwrap(),
            r#"{"method":"password"}"#
        );
        assert_eq!(
            serde_json::to_string(&SshAuthMethod::PrivateKey { path: "/x".into() }).unwrap(),
            r#"{"method":"private_key","path":"/x"}"#
        );
    }

    /// Locks in the wire contract the frontend's `invoke("connect", { config })` relies on.
    /// `ConnectionConfig` has no `#[serde(rename_all)]`, so the JSON keys are the literal
    /// (snake_case) Rust field names — a frontend that sent camelCase `sshTunnel`/`readOnly`
    /// would fail to deserialize here with "missing field", not silently coerce.
    #[test]
    fn deserializes_the_exact_shape_the_frontend_sends() {
        let json = serde_json::json!({
            "id": "conn-1",
            "name": "Local Postgres",
            "engine": "postgres",
            "host": "localhost",
            "port": 5433,
            "database": "structly_smoketest",
            "username": "structly",
            "ssh_tunnel": null,
            "read_only": false,
        });

        let config: ConnectionConfig =
            serde_json::from_value(json).expect("snake_case payload should deserialize");
        assert_eq!(config.engine, Engine::Postgres);
        assert!(config.ssh_tunnel.is_none());
        assert!(!config.read_only);
    }

    #[test]
    fn rejects_camel_case_field_names() {
        let json = serde_json::json!({
            "id": "conn-1",
            "name": "Local Postgres",
            "engine": "postgres",
            "host": "localhost",
            "port": 5433,
            "database": "structly_smoketest",
            "username": "structly",
            "sshTunnel": null,
            "readOnly": false,
        });

        assert!(
            serde_json::from_value::<ConnectionConfig>(json).is_err(),
            "camelCase field names must NOT silently parse — this is the shape of the bug that broke `connect` before it was ever called from the UI"
        );
    }
}
