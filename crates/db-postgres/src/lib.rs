use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use db_core::{
    ColumnMeta, ConnectSecrets, ConnectionConfig, DatabaseDriver, DbConnection, DbError, DbResult,
    Engine, ForeignKeyMeta, IndexMeta, QueryResult, SchemaMeta, TableMeta,
};
use futures_util::TryStreamExt;
use rust_decimal::Decimal;
use serde_json::Value as JsonValue;
use sqlx::postgres::{PgPoolOptions, PgRow};
use sqlx::{Column, Either, PgPool, Row, TypeInfo};
use std::time::Instant;
use uuid::Uuid;

/// Decode one column of a `PgRow` into JSON, dispatching on the Postgres type name.
/// Covers the primitive/common types; anything else (arrays, custom/composite types,
/// network types, intervals, ...) falls back to a best-effort text decode, or NULL if
/// even that fails — a real client would keep extending this match as gaps are found.
fn pg_value_to_json(row: &PgRow, index: usize, type_name: &str) -> JsonValue {
    macro_rules! decode {
        ($t:ty) => {
            row.try_get::<Option<$t>, _>(index)
                .ok()
                .flatten()
                .map(|v| serde_json::to_value(v).unwrap_or(JsonValue::Null))
        };
    }

    let decoded = match type_name {
        "BOOL" => decode!(bool),
        "INT2" => decode!(i16),
        "INT4" => decode!(i32),
        "INT8" => decode!(i64),
        "FLOAT4" => decode!(f32),
        "FLOAT8" => decode!(f64),
        "NUMERIC" => decode!(Decimal),
        "TEXT" | "VARCHAR" | "BPCHAR" | "NAME" | "CITEXT" => decode!(String),
        "UUID" => decode!(Uuid),
        "JSON" | "JSONB" => decode!(JsonValue),
        "TIMESTAMPTZ" => row
            .try_get::<Option<DateTime<Utc>>, _>(index)
            .ok()
            .flatten()
            .map(|v| JsonValue::String(v.to_rfc3339())),
        "TIMESTAMP" => row
            .try_get::<Option<NaiveDateTime>, _>(index)
            .ok()
            .flatten()
            .map(|v| JsonValue::String(v.to_string())),
        "DATE" => row
            .try_get::<Option<NaiveDate>, _>(index)
            .ok()
            .flatten()
            .map(|v| JsonValue::String(v.to_string())),
        "TIME" => row
            .try_get::<Option<NaiveTime>, _>(index)
            .ok()
            .flatten()
            .map(|v| JsonValue::String(v.to_string())),
        _ => None,
    };

    decoded.unwrap_or_else(|| {
        // Best-effort fallback for types we don't decode explicitly above: this only
        // succeeds for text-protocol-compatible values, so most binary types will just
        // come back as JSON null rather than panicking or failing the whole query.
        row.try_get::<Option<String>, _>(index)
            .ok()
            .flatten()
            .map(JsonValue::String)
            .unwrap_or(JsonValue::Null)
    })
}

pub struct PostgresDriver;

#[async_trait]
impl DatabaseDriver for PostgresDriver {
    fn engine(&self) -> Engine {
        Engine::Postgres
    }

    async fn connect(
        &self,
        config: &ConnectionConfig,
        secrets: ConnectSecrets<'_>,
    ) -> DbResult<Box<dyn DbConnection>> {
        let real_host = config.host.as_deref().unwrap_or("localhost");
        let real_port = config.port.unwrap_or(5432);

        let tunnel = ssh_tunnel::maybe_open_tunnel(
            config.ssh_tunnel.as_ref(),
            secrets.ssh_secret,
            real_host,
            real_port,
        )
        .await
        .map_err(|e| DbError::Connection(format!("ssh tunnel: {e}")))?;

        // With a tunnel, the driver talks to our local end of it instead of the real
        // host:port — the tunnel is what actually reaches the real database from there.
        let (connect_host, connect_port) = tunnel
            .as_ref()
            .map(|t| ("127.0.0.1", t.local_port))
            .unwrap_or((real_host, real_port));

        let url = format!(
            "postgres://{}:{}@{}:{}/{}",
            config.username.as_deref().unwrap_or(""),
            secrets.db_password.unwrap_or(""),
            connect_host,
            connect_port,
            config.database
        );
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .map_err(|e| DbError::Connection(e.to_string()))?;
        Ok(Box::new(PostgresConnection {
            pool,
            current_pid: tokio::sync::Mutex::new(None),
            _tunnel: tunnel,
        }))
    }
}

pub struct PostgresConnection {
    pool: PgPool,
    /// Backend PID of whichever pooled connection is currently running a query, if any —
    /// set for the duration of `execute()` so `cancel()` (running concurrently, on a
    /// different pooled connection) knows who to tell `pg_cancel_backend` to interrupt.
    current_pid: tokio::sync::Mutex<Option<i32>>,
    /// Kept alive for as long as this connection is — dropping it would tear down the
    /// port forward the pool's connections are actually talking through.
    _tunnel: Option<ssh_tunnel::SshTunnel>,
}

#[async_trait]
impl DbConnection for PostgresConnection {
    async fn execute(&self, sql: &str) -> DbResult<QueryResult> {
        let started = Instant::now();

        // Cancellation targets one specific backend process, so the query needs to run on
        // one specific connection we can identify — not just "whichever one the pool picks",
        // which is what `&self.pool` as an executor would give us.
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| DbError::Connection(e.to_string()))?;
        let pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
            .fetch_one(&mut *conn)
            .await
            .map_err(|e| DbError::Query(e.to_string()))?;
        *self.current_pid.lock().await = Some(pid);

        // `fetch_many` (rather than `fetch_all`) is what surfaces rows_affected: Postgres
        // sends rows first, then a CommandComplete tag with the affected-row count, and
        // fetch_all's plain Vec<Row> throws that tag away. raw_sql (not query) is the
        // non-deprecated way to run unparameterized, possibly-multi-statement text —
        // exactly what the SQL editor sends.
        let outcome = async {
            let mut stream = sqlx::raw_sql(sql).fetch_many(&mut *conn);
            let mut rows = Vec::new();
            let mut rows_affected: u64 = 0;
            while let Some(item) = stream.try_next().await? {
                match item {
                    Either::Left(result) => rows_affected += result.rows_affected(),
                    Either::Right(row) => rows.push(row),
                }
            }
            Ok::<_, sqlx::Error>((rows, rows_affected))
        }
        .await;

        // Cleared regardless of outcome — including cancellation itself, which surfaces
        // here as an Err from the stream (Postgres error 57014, query_canceled).
        *self.current_pid.lock().await = None;
        let (rows, rows_affected) = outcome.map_err(|e| DbError::Query(e.to_string()))?;

        let columns: Vec<String> = rows
            .first()
            .map(|r| r.columns().iter().map(|c| c.name().to_string()).collect())
            .unwrap_or_default();

        let type_names: Vec<String> = rows
            .first()
            .map(|r| {
                r.columns()
                    .iter()
                    .map(|c| c.type_info().name().to_string())
                    .collect()
            })
            .unwrap_or_default();

        let json_rows = rows
            .iter()
            .map(|row| {
                type_names
                    .iter()
                    .enumerate()
                    .map(|(i, type_name)| pg_value_to_json(row, i, type_name))
                    .collect()
            })
            .collect();

        Ok(QueryResult {
            columns,
            rows: json_rows,
            rows_affected: Some(rows_affected),
            duration_ms: started.elapsed().as_millis() as u64,
        })
    }

    async fn list_schemas(&self) -> DbResult<Vec<SchemaMeta>> {
        let rows =
            sqlx::query("select schema_name from information_schema.schemata order by schema_name")
                .fetch_all(&self.pool)
                .await
                .map_err(|e| DbError::Query(e.to_string()))?;
        Ok(rows.iter().map(|r| SchemaMeta { name: r.get(0) }).collect())
    }

    async fn list_tables(&self, schema: &str) -> DbResult<Vec<TableMeta>> {
        let rows = sqlx::query(
            "select table_name from information_schema.tables where table_schema = $1 order by table_name",
        )
        .bind(schema)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::Query(e.to_string()))?;
        Ok(rows
            .iter()
            .map(|r| TableMeta {
                schema: schema.to_string(),
                name: r.get(0),
                approx_row_count: None,
            })
            .collect())
    }

    async fn table_columns(&self, schema: &str, table: &str) -> DbResult<Vec<ColumnMeta>> {
        let rows = sqlx::query(
            "select c.column_name, c.data_type, c.is_nullable, c.column_default, \
                    exists ( \
                        select 1 from information_schema.table_constraints tc \
                        join information_schema.key_column_usage kcu \
                          on tc.constraint_name = kcu.constraint_name \
                         and tc.table_schema = kcu.table_schema \
                        where tc.constraint_type = 'PRIMARY KEY' \
                          and tc.table_schema = c.table_schema \
                          and tc.table_name = c.table_name \
                          and kcu.column_name = c.column_name \
                    ) as is_primary_key \
             from information_schema.columns c \
             where c.table_schema = $1 and c.table_name = $2 order by c.ordinal_position",
        )
        .bind(schema)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::Query(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|r| ColumnMeta {
                name: r.get(0),
                data_type: r.get(1),
                nullable: r.get::<String, _>(2) == "YES",
                is_primary_key: r.get(4),
                default: r.get(3),
            })
            .collect())
    }

    async fn table_indexes(&self, schema: &str, table: &str) -> DbResult<Vec<IndexMeta>> {
        let rows = sqlx::query(
            "select indexname, indexdef from pg_indexes where schemaname = $1 and tablename = $2",
        )
        .bind(schema)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::Query(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|r| {
                let definition: String = r.get(1);
                IndexMeta {
                    name: r.get(0),
                    is_unique: definition.contains("UNIQUE"),
                    definition,
                }
            })
            .collect())
    }

    async fn list_foreign_keys(&self, schema: &str) -> DbResult<Vec<ForeignKeyMeta>> {
        let rows = sqlx::query(
            "select tc.constraint_name, kcu.table_name, kcu.column_name, \
                    ccu.table_name, ccu.column_name \
             from information_schema.table_constraints tc \
             join information_schema.key_column_usage kcu \
               on tc.constraint_name = kcu.constraint_name and tc.table_schema = kcu.table_schema \
             join information_schema.constraint_column_usage ccu \
               on tc.constraint_name = ccu.constraint_name and tc.table_schema = ccu.table_schema \
             where tc.constraint_type = 'FOREIGN KEY' and tc.table_schema = $1 \
             order by tc.constraint_name",
        )
        .bind(schema)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::Query(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|r| ForeignKeyMeta {
                constraint_name: r.get(0),
                from_table: r.get(1),
                from_column: r.get(2),
                to_table: r.get(3),
                to_column: r.get(4),
            })
            .collect())
    }

    async fn cancel(&self) -> DbResult<()> {
        let Some(pid) = *self.current_pid.lock().await else {
            // Nothing running (or it just finished) — not an error, just a no-op.
            return Ok(());
        };
        // Runs on a fresh pooled connection, not the one `pid` refers to — that one is
        // busy running the statement being cancelled.
        sqlx::query("select pg_cancel_backend($1)")
            .bind(pid)
            .execute(&self.pool)
            .await
            .map_err(|e| DbError::Query(e.to_string()))?;
        Ok(())
    }

    async fn close(&self) -> DbResult<()> {
        self.pool.close().await;
        Ok(())
    }
}
