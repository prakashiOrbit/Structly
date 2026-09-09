use async_trait::async_trait;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use db_core::{
    ColumnMeta, ConnectSecrets, ConnectionConfig, DatabaseDriver, DbConnection, DbError, DbResult,
    Engine, ForeignKeyMeta, IndexMeta, QueryResult, SchemaMeta, TableMeta,
};
use futures_util::TryStreamExt;
use serde_json::Value as JsonValue;
use sqlx::sqlite::{SqlitePoolOptions, SqliteRow};
use sqlx::{Column, Either, Row, SqlitePool, TypeInfo};
use std::ptr::NonNull;
use std::time::Instant;

/// `sqlite3_interrupt` is documented by SQLite as safe to call from any thread, at any
/// time, including concurrently with another thread executing a statement on this exact
/// connection handle — that's the API's whole purpose, so wrapping the otherwise-non-Send
/// raw pointer this way is sound.
struct InterruptHandle(NonNull<libsqlite3_sys::sqlite3>);
unsafe impl Send for InterruptHandle {}
unsafe impl Sync for InterruptHandle {}

/// Decode one column of a `SqliteRow` into JSON. SQLite is dynamically typed per-value
/// (manifest typing), not per-column, so `type_name` here is really "what the *first* row
/// happened to store" — a column with no declared affinity could hold a different storage
/// class on a later row, and this function has no way to see that from the first row alone.
fn sqlite_value_to_json(row: &SqliteRow, index: usize, type_name: &str) -> JsonValue {
    macro_rules! decode {
        ($t:ty) => {
            row.try_get::<Option<$t>, _>(index)
                .ok()
                .flatten()
                .map(|v| serde_json::to_value(v).unwrap_or(JsonValue::Null))
        };
    }

    let decoded = match type_name {
        "NULL" => Some(JsonValue::Null),
        "BOOLEAN" => decode!(bool),
        "INTEGER" => decode!(i64),
        "REAL" => decode!(f64),
        "TEXT" => decode!(String),
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
        "DATETIME" => row
            .try_get::<Option<NaiveDateTime>, _>(index)
            .ok()
            .flatten()
            .map(|v| JsonValue::String(v.to_string())),
        // NUMERIC affinity can hold an integer, a real, or text — try each in turn.
        "NUMERIC" => decode!(i64)
            .or_else(|| decode!(f64))
            .or_else(|| decode!(String)),
        _ => None,
    };

    decoded.unwrap_or_else(|| {
        // Best-effort fallback (mainly BLOB): only succeeds for text-compatible values.
        row.try_get::<Option<String>, _>(index)
            .ok()
            .flatten()
            .map(JsonValue::String)
            .unwrap_or(JsonValue::Null)
    })
}

pub struct SqliteDriver;

#[async_trait]
impl DatabaseDriver for SqliteDriver {
    fn engine(&self) -> Engine {
        Engine::Sqlite
    }

    async fn connect(
        &self,
        config: &ConnectionConfig,
        _secrets: ConnectSecrets<'_>,
    ) -> DbResult<Box<dyn DbConnection>> {
        // No SSH tunnel support here: SQLite is a local file, not a network service, so
        // `config.ssh_tunnel` (if set) is silently ignored rather than an error — matches
        // how the UI should just not offer that field for this engine.
        // `database` holds the file path for SQLite; there's no host/port/user. `mode=rwc`
        // creates the file if it doesn't exist yet — without it, connecting to a brand new
        // database path fails instead of creating one.
        let url = format!("sqlite://{}?mode=rwc", config.database);
        let pool = SqlitePoolOptions::new()
            .max_connections(1) // SQLite serializes writers anyway; one connection avoids lock contention.
            .connect(&url)
            .await
            .map_err(|e| DbError::Connection(e.to_string()))?;

        // Grab the raw C handle once, up front. `pool.acquire()` returns the pool's one
        // physical connection; dropping it afterwards just returns that same connection to
        // the (still-open) pool rather than closing it, so the pointer stays valid for as
        // long as this `SqliteConnection` is — it's what `cancel()` calls
        // `sqlite3_interrupt` on, entirely bypassing sqlx's async connection machinery (the
        // whole point: it has to work even while that machinery is busy running a query).
        let mut conn = pool
            .acquire()
            .await
            .map_err(|e| DbError::Connection(e.to_string()))?;
        let raw_handle = conn
            .lock_handle()
            .await
            .map_err(|e| DbError::Connection(e.to_string()))?
            .as_raw_handle();
        drop(conn);

        Ok(Box::new(SqliteConnection {
            pool,
            interrupt_handle: InterruptHandle(raw_handle),
        }))
    }
}

pub struct SqliteConnection {
    pool: SqlitePool,
    interrupt_handle: InterruptHandle,
}

#[async_trait]
impl DbConnection for SqliteConnection {
    async fn execute(&self, sql: &str) -> DbResult<QueryResult> {
        let started = Instant::now();

        let mut stream = sqlx::raw_sql(sql).fetch_many(&self.pool);
        let mut rows = Vec::new();
        let mut rows_affected: u64 = 0;
        while let Some(item) = stream
            .try_next()
            .await
            .map_err(|e| DbError::Query(e.to_string()))?
        {
            match item {
                Either::Left(result) => rows_affected += result.rows_affected(),
                Either::Right(row) => rows.push(row),
            }
        }
        drop(stream);

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
                    .map(|(i, type_name)| sqlite_value_to_json(row, i, type_name))
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
        // SQLite has no schema concept beyond the attached database itself.
        Ok(vec![SchemaMeta {
            name: "main".to_string(),
        }])
    }

    async fn list_tables(&self, _schema: &str) -> DbResult<Vec<TableMeta>> {
        let rows = sqlx::query(
            "select name from sqlite_master where type = 'table' and name not like 'sqlite_%' order by name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::Query(e.to_string()))?;
        Ok(rows
            .iter()
            .map(|r| TableMeta {
                schema: "main".to_string(),
                name: r.get(0),
                approx_row_count: None,
            })
            .collect())
    }

    async fn table_columns(&self, _schema: &str, table: &str) -> DbResult<Vec<ColumnMeta>> {
        // pragma_table_info doesn't accept a bound parameter, so the table name is
        // interpolated directly. It comes from list_tables (sqlite_master), never from
        // raw user input, so this is not an injection vector in practice.
        let query = format!(
            "select name, type, \"notnull\", dflt_value, pk from pragma_table_info('{}')",
            table.replace('\'', "''")
        );
        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DbError::Query(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|r| ColumnMeta {
                name: r.get(0),
                data_type: r.get(1),
                nullable: r.get::<i64, _>(2) == 0,
                default: r.get(3),
                is_primary_key: r.get::<i64, _>(4) != 0,
            })
            .collect())
    }

    async fn table_indexes(&self, _schema: &str, table: &str) -> DbResult<Vec<IndexMeta>> {
        // Auto-generated indexes (e.g. for a UNIQUE column constraint) have no entry in
        // sqlite_master.sql to parse "UNIQUE" out of, so uniqueness comes from
        // pragma_index_list instead, which reports it directly regardless of origin.
        // pragma_index_list doesn't accept a bound parameter, so the table name is
        // interpolated directly; it comes from list_tables (sqlite_master), never from
        // raw user input, so this is not an injection vector in practice.
        let query = format!(
            "select il.name, sm.sql, il.\"unique\" \
             from pragma_index_list('{}') il \
             left join sqlite_master sm on sm.type = 'index' and sm.name = il.name",
            table.replace('\'', "''")
        );
        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DbError::Query(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|r| {
                let definition: Option<String> = r.get(1);
                IndexMeta {
                    name: r.get(0),
                    definition: definition.unwrap_or_default(),
                    is_unique: r.get::<i64, _>(2) != 0,
                }
            })
            .collect())
    }

    async fn list_foreign_keys(&self, schema: &str) -> DbResult<Vec<ForeignKeyMeta>> {
        // SQLite has no schema-wide "list all foreign keys" pragma — pragma_foreign_key_list
        // is per-table, so this fans out over every table instead of one query.
        let tables = self.list_tables(schema).await?;
        let mut foreign_keys = Vec::new();

        for t in tables {
            // Same non-parameterizable-pragma situation as table_indexes/table_columns:
            // the table name is interpolated directly, but it comes from list_tables
            // (sqlite_master), never from raw user input.
            let query = format!(
                "select id, \"table\", \"from\", \"to\" from pragma_foreign_key_list('{}')",
                t.name.replace('\'', "''")
            );
            let rows = sqlx::query(&query)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| DbError::Query(e.to_string()))?;

            for r in rows {
                let id: i64 = r.get(0);
                foreign_keys.push(ForeignKeyMeta {
                    constraint_name: format!("fk_{}_{}", t.name, id),
                    from_table: t.name.clone(),
                    from_column: r.get(2),
                    to_table: r.get(1),
                    to_column: r.get(3),
                });
            }
        }

        Ok(foreign_keys)
    }

    async fn cancel(&self) -> DbResult<()> {
        // SAFETY: sqlite3_interrupt is documented as safe to call at any time, including
        // when no query is running (a harmless no-op) or concurrently with one that is.
        unsafe {
            libsqlite3_sys::sqlite3_interrupt(self.interrupt_handle.0.as_ptr());
        }
        Ok(())
    }

    async fn close(&self) -> DbResult<()> {
        self.pool.close().await;
        Ok(())
    }
}
