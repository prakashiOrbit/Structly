use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use db_core::{
    ColumnMeta, ConnectSecrets, ConnectionConfig, DatabaseDriver, DbConnection, DbError, DbResult,
    Engine, ForeignKeyMeta, IndexMeta, QueryResult, SchemaMeta, TableMeta,
};
use futures_util::TryStreamExt;
use rust_decimal::Decimal;
use serde_json::Value as JsonValue;
use sqlx::mysql::{MySqlPoolOptions, MySqlRow};
use sqlx::{Column, Either, MySqlPool, Row, TypeInfo};
use std::time::Instant;

/// Decode one column of a `MySqlRow` into JSON, dispatching on the MySQL type name.
/// Covers the primitive/common types; anything else (BLOB, SET, ENUM-as-raw, spatial
/// types, ...) falls back to a best-effort text decode, or NULL if even that fails.
fn mysql_value_to_json(row: &MySqlRow, index: usize, type_name: &str) -> JsonValue {
    macro_rules! decode {
        ($t:ty) => {
            row.try_get::<Option<$t>, _>(index)
                .ok()
                .flatten()
                .map(|v| serde_json::to_value(v).unwrap_or(JsonValue::Null))
        };
    }

    let decoded = match type_name {
        "BOOLEAN" => decode!(bool),
        "TINYINT" | "TINYINT UNSIGNED" | "SMALLINT" | "SMALLINT UNSIGNED" | "MEDIUMINT"
        | "MEDIUMINT UNSIGNED" | "INT" | "INT UNSIGNED" | "YEAR" => decode!(i32),
        "BIGINT" | "BIGINT UNSIGNED" => decode!(i64),
        "FLOAT" => decode!(f32),
        "DOUBLE" => decode!(f64),
        "DECIMAL" => decode!(Decimal),
        "VARCHAR" | "CHAR" | "TEXT" | "TINYTEXT" | "MEDIUMTEXT" | "LONGTEXT" | "ENUM" => {
            decode!(String)
        }
        "JSON" => decode!(JsonValue),
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
        // DATETIME carries no timezone, so it decodes as a naive value. TIMESTAMP is
        // stored as UTC internally — sqlx only implements that decode for `DateTime<Utc>`,
        // not `NaiveDateTime`, so the two need separate Rust types here.
        "DATETIME" => row
            .try_get::<Option<NaiveDateTime>, _>(index)
            .ok()
            .flatten()
            .map(|v| JsonValue::String(v.to_string())),
        "TIMESTAMP" => row
            .try_get::<Option<DateTime<Utc>>, _>(index)
            .ok()
            .flatten()
            .map(|v| JsonValue::String(v.to_rfc3339())),
        _ => None,
    };

    decoded.unwrap_or_else(|| {
        // Best-effort fallback for types we don't decode explicitly above (BLOB, SET,
        // spatial types, ...): only succeeds for text-compatible values, so most binary
        // types come back as JSON null rather than failing the whole query.
        row.try_get::<Option<String>, _>(index)
            .ok()
            .flatten()
            .map(JsonValue::String)
            .unwrap_or(JsonValue::Null)
    })
}

pub struct MySqlDriver;

#[async_trait]
impl DatabaseDriver for MySqlDriver {
    fn engine(&self) -> Engine {
        Engine::MySql
    }

    async fn connect(
        &self,
        config: &ConnectionConfig,
        secrets: ConnectSecrets<'_>,
    ) -> DbResult<Box<dyn DbConnection>> {
        let real_host = config.host.as_deref().unwrap_or("localhost");
        let real_port = config.port.unwrap_or(3306);

        let tunnel = ssh_tunnel::maybe_open_tunnel(
            config.ssh_tunnel.as_ref(),
            secrets.ssh_secret,
            real_host,
            real_port,
        )
        .await
        .map_err(|e| DbError::Connection(format!("ssh tunnel: {e}")))?;

        let (connect_host, connect_port) = tunnel
            .as_ref()
            .map(|t| ("127.0.0.1", t.local_port))
            .unwrap_or((real_host, real_port));

        let url = format!(
            "mysql://{}:{}@{}:{}/{}",
            config.username.as_deref().unwrap_or(""),
            secrets.db_password.unwrap_or(""),
            connect_host,
            connect_port,
            config.database
        );
        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .map_err(|e| DbError::Connection(e.to_string()))?;
        Ok(Box::new(MySqlConnection {
            pool,
            current_connection_id: tokio::sync::Mutex::new(None),
            _tunnel: tunnel,
        }))
    }
}

pub struct MySqlConnection {
    pool: MySqlPool,
    /// MySQL processlist id of whichever pooled connection is currently running a query,
    /// if any — set for the duration of `execute()` so `cancel()` (running concurrently,
    /// on a different pooled connection) knows which session to `KILL QUERY`.
    current_connection_id: tokio::sync::Mutex<Option<u64>>,
    /// Kept alive for as long as this connection is — dropping it would tear down the
    /// port forward the pool's connections are actually talking through.
    _tunnel: Option<ssh_tunnel::SshTunnel>,
}

#[async_trait]
impl DbConnection for MySqlConnection {
    async fn execute(&self, sql: &str) -> DbResult<QueryResult> {
        let started = Instant::now();

        // Cancellation targets one specific server session (via KILL QUERY <id>), so the
        // query needs to run on one specific connection we can identify — not just
        // "whichever one the pool picks", which is what `&self.pool` as an executor would give us.
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| DbError::Connection(e.to_string()))?;
        let connection_id: u64 = sqlx::query_scalar("select connection_id()")
            .fetch_one(&mut *conn)
            .await
            .map_err(|e| DbError::Query(e.to_string()))?;
        *self.current_connection_id.lock().await = Some(connection_id);

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
        // here as an Err from the stream (MySQL error 1317, query execution interrupted).
        *self.current_connection_id.lock().await = None;
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
                    .map(|(i, type_name)| mysql_value_to_json(row, i, type_name))
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
        // MySQL has no separate schema layer above the database itself: "schema" and
        // "database" are the same thing, unlike Postgres. We surface the current
        // database as the one schema so the explorer tree still has a schema level.
        // MySQL 8's information_schema reports several text columns as VARBINARY at the
        // wire-protocol level (a data-dictionary quirk, not an actual binary charset), which
        // fails a plain String decode — CAST(... AS CHAR) forces MySQL to hand back real TEXT.
        let rows = sqlx::query(
            "select cast(schema_name as char) from information_schema.schemata order by schema_name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::Query(e.to_string()))?;
        Ok(rows.iter().map(|r| SchemaMeta { name: r.get(0) }).collect())
    }

    async fn list_tables(&self, schema: &str) -> DbResult<Vec<TableMeta>> {
        let rows = sqlx::query(
            "select cast(table_name as char) from information_schema.tables where table_schema = ? order by table_name",
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
            "select cast(column_name as char), cast(data_type as char), cast(is_nullable as char), \
                    cast(column_default as char), column_key = 'PRI' \
             from information_schema.columns \
             where table_schema = ? and table_name = ? order by ordinal_position",
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
                is_primary_key: r.get::<i64, _>(4) != 0,
                default: r.get(3),
            })
            .collect())
    }

    async fn table_indexes(&self, schema: &str, table: &str) -> DbResult<Vec<IndexMeta>> {
        let rows = sqlx::query(
            "select cast(index_name as char), non_unique from information_schema.statistics \
             where table_schema = ? and table_name = ? group by index_name, non_unique",
        )
        .bind(schema)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::Query(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|r| IndexMeta {
                name: r.get(0),
                is_unique: r.get::<i64, _>(1) == 0,
                definition: String::new(),
            })
            .collect())
    }

    async fn list_foreign_keys(&self, schema: &str) -> DbResult<Vec<ForeignKeyMeta>> {
        // Unlike Postgres, MySQL's key_column_usage already carries the referenced
        // table/column directly on FK rows, so no second join is needed here.
        let rows = sqlx::query(
            "select cast(constraint_name as char), cast(table_name as char), cast(column_name as char), \
                    cast(referenced_table_name as char), cast(referenced_column_name as char) \
             from information_schema.key_column_usage \
             where table_schema = ? and referenced_table_name is not null \
             order by constraint_name",
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
        let Some(connection_id) = *self.current_connection_id.lock().await else {
            // Nothing running (or it just finished) — not an error, just a no-op.
            return Ok(());
        };
        // KILL isn't supported over the prepared-statement protocol sqlx::query() uses, so
        // this has to go through raw_sql (plain text protocol) instead. Runs on a fresh
        // pooled connection, not the one `connection_id` refers to — that one is busy
        // running the statement being killed. `connection_id` is a u64 we generated
        // ourselves (not user input), so interpolating it directly is safe here.
        sqlx::raw_sql(&format!("kill query {connection_id}"))
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
