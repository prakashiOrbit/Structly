//! Local metadata store (saved connections, query history, favorites) — everything
//! *except* secrets, which live in the OS keychain via the `secrets` crate instead.
//! Backed by a single SQLite file in the app's data directory.
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Row, SqlitePool};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("storage error: {0}")]
    Db(#[from] sqlx::Error),
}

pub type StorageResult<T> = Result<T, StorageError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedConnection {
    pub id: String,
    pub name: String,
    pub engine: String,
    pub host: Option<String>,
    pub port: Option<i64>,
    pub database: String,
    pub username: Option<String>,
    pub ssh_tunnel_json: Option<String>,
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub connection_id: String,
    pub sql: String,
    pub duration_ms: i64,
    pub status: String,
    pub executed_at: String,
}

pub async fn init(db_path: &str) -> StorageResult<SqlitePool> {
    let url = if db_path == ":memory:" {
        "sqlite::memory:".to_string()
    } else {
        format!("sqlite://{}?mode=rwc", db_path)
    };
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await?;

    sqlx::query(
        "create table if not exists saved_connections (
            id text primary key,
            name text not null,
            engine text not null,
            host text,
            port integer,
            database text not null,
            username text,
            ssh_tunnel_json text,
            read_only integer not null default 0,
            created_at text not null default current_timestamp
        )",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "create table if not exists query_history (
            id integer primary key autoincrement,
            connection_id text not null,
            sql text not null,
            duration_ms integer not null,
            status text not null,
            executed_at text not null default current_timestamp
        )",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "create table if not exists favorites (
            id integer primary key autoincrement,
            kind text not null,
            label text not null,
            meta_json text,
            created_at text not null default current_timestamp
        )",
    )
    .execute(&pool)
    .await?;

    Ok(pool)
}

pub async fn save_connection(pool: &SqlitePool, c: &SavedConnection) -> StorageResult<()> {
    sqlx::query(
        "insert into saved_connections (id, name, engine, host, port, database, username, ssh_tunnel_json, read_only)
         values (?, ?, ?, ?, ?, ?, ?, ?, ?)
         on conflict(id) do update set
            name = excluded.name, engine = excluded.engine, host = excluded.host,
            port = excluded.port, database = excluded.database, username = excluded.username,
            ssh_tunnel_json = excluded.ssh_tunnel_json, read_only = excluded.read_only",
    )
    .bind(&c.id)
    .bind(&c.name)
    .bind(&c.engine)
    .bind(&c.host)
    .bind(c.port)
    .bind(&c.database)
    .bind(&c.username)
    .bind(&c.ssh_tunnel_json)
    .bind(c.read_only)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_connections(pool: &SqlitePool) -> StorageResult<Vec<SavedConnection>> {
    let rows = sqlx::query("select id, name, engine, host, port, database, username, ssh_tunnel_json, read_only from saved_connections order by name")
        .fetch_all(pool)
        .await?;
    Ok(rows
        .iter()
        .map(|r| SavedConnection {
            id: r.get(0),
            name: r.get(1),
            engine: r.get(2),
            host: r.get(3),
            port: r.get(4),
            database: r.get(5),
            username: r.get(6),
            ssh_tunnel_json: r.get(7),
            read_only: r.get::<i64, _>(8) != 0,
        })
        .collect())
}

pub async fn delete_connection(pool: &SqlitePool, id: &str) -> StorageResult<()> {
    sqlx::query("delete from saved_connections where id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn record_history(
    pool: &SqlitePool,
    connection_id: &str,
    sql: &str,
    duration_ms: i64,
    status: &str,
) -> StorageResult<()> {
    sqlx::query(
        "insert into query_history (connection_id, sql, duration_ms, status) values (?, ?, ?, ?)",
    )
    .bind(connection_id)
    .bind(sql)
    .bind(duration_ms)
    .bind(status)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_history(
    pool: &SqlitePool,
    connection_id: &str,
    limit: i64,
) -> StorageResult<Vec<HistoryEntry>> {
    let rows = sqlx::query(
        "select id, connection_id, sql, duration_ms, status, executed_at from query_history
         where connection_id = ? order by id desc limit ?",
    )
    .bind(connection_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| HistoryEntry {
            id: r.get(0),
            connection_id: r.get(1),
            sql: r.get(2),
            duration_ms: r.get(3),
            status: r.get(4),
            executed_at: r.get(5),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_connection() -> SavedConnection {
        SavedConnection {
            id: "conn-1".into(),
            name: "Local Postgres".into(),
            engine: "postgres".into(),
            host: Some("localhost".into()),
            port: Some(5432),
            database: "app".into(),
            username: Some("postgres".into()),
            ssh_tunnel_json: None,
            read_only: false,
        }
    }

    #[tokio::test]
    async fn save_and_list_round_trips_a_connection() {
        let pool = init(":memory:").await.unwrap();
        save_connection(&pool, &sample_connection()).await.unwrap();

        let saved = list_connections(&pool).await.unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].name, "Local Postgres");
    }

    #[tokio::test]
    async fn save_connection_upserts_on_conflicting_id() {
        let pool = init(":memory:").await.unwrap();
        save_connection(&pool, &sample_connection()).await.unwrap();

        let mut renamed = sample_connection();
        renamed.name = "Renamed".into();
        save_connection(&pool, &renamed).await.unwrap();

        let saved = list_connections(&pool).await.unwrap();
        assert_eq!(
            saved.len(),
            1,
            "same id should update, not insert a second row"
        );
        assert_eq!(saved[0].name, "Renamed");
    }

    #[tokio::test]
    async fn delete_connection_removes_it() {
        let pool = init(":memory:").await.unwrap();
        save_connection(&pool, &sample_connection()).await.unwrap();
        delete_connection(&pool, "conn-1").await.unwrap();

        assert!(list_connections(&pool).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn history_is_scoped_per_connection_and_most_recent_first() {
        let pool = init(":memory:").await.unwrap();
        record_history(&pool, "conn-1", "select 1", 5, "ok")
            .await
            .unwrap();
        record_history(&pool, "conn-1", "select 2", 7, "ok")
            .await
            .unwrap();
        record_history(&pool, "conn-2", "select 3", 9, "ok")
            .await
            .unwrap();

        let history = list_history(&pool, "conn-1", 10).await.unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(
            history[0].sql, "select 2",
            "most recent query should come first"
        );
    }
}
