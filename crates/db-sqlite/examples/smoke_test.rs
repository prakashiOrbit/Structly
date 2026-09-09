//! Exercises SqliteDriver against a real (file-backed) SQLite database end-to-end:
//! connect (creating the file), seed a table directly, then introspect + query through
//! the driver and check the decoded result set.
//!
//! Run with: cargo run -p db-sqlite --example smoke_test
use db_core::{ConnectionConfig, DatabaseDriver, Engine};
use std::env;
use std::time::Instant;

#[tokio::main]
async fn main() {
    let path = env::temp_dir().join(format!("structly_smoketest_{}.sqlite", std::process::id()));
    let path_str = path.to_str().unwrap().to_string();
    println!("using database file: {path_str}");

    let config = ConnectionConfig {
        id: "smoketest".into(),
        name: "Smoke Test".into(),
        engine: Engine::Sqlite,
        host: None,
        port: None,
        database: path_str.clone(),
        username: None,
        ssh_tunnel: None,
        read_only: false,
    };

    let conn = DatabaseDriver::connect(
        &db_sqlite::SqliteDriver,
        &config,
        db_core::ConnectSecrets::default(),
    )
    .await
    .expect("connect (and create) failed");
    println!("connected (file created).\n");

    conn.execute(
        "create table users ( \
            id integer primary key autoincrement, \
            email text not null unique, \
            plan text not null default 'free', \
            score real, \
            created_at datetime not null default current_timestamp \
        )",
    )
    .await
    .expect("create table failed");
    conn.execute("create index idx_users_plan on users (plan)")
        .await
        .expect("create index failed");
    conn.execute(
        "insert into users (email, plan, score, created_at) values \
         ('ada@example.com', 'pro', 9.5, '2026-01-01 12:00:00'), \
         ('grace@example.com', 'free', null, '2026-01-02 08:30:00'), \
         ('linus@example.com', 'pro', 7.25, '2026-01-03 22:15:00')",
    )
    .await
    .expect("insert failed");
    println!("seeded users table.\n");

    let schemas = conn.list_schemas().await.expect("list_schemas failed");
    println!(
        "schemas: {:?}\n",
        schemas.iter().map(|s| &s.name).collect::<Vec<_>>()
    );

    let tables = conn.list_tables("main").await.expect("list_tables failed");
    println!(
        "tables: {:?}\n",
        tables.iter().map(|t| &t.name).collect::<Vec<_>>()
    );

    let columns = conn
        .table_columns("main", "users")
        .await
        .expect("table_columns failed");
    println!("columns on users:");
    for c in &columns {
        println!(
            "  {:<12} {:<10} nullable={:<5} pk={:<5} default={:?}",
            c.name, c.data_type, c.nullable, c.is_primary_key, c.default
        );
    }
    println!();

    let indexes = conn
        .table_indexes("main", "users")
        .await
        .expect("table_indexes failed");
    println!("indexes on users:");
    for i in &indexes {
        println!("  {} (unique={})", i.name, i.is_unique);
    }
    println!();

    let result = conn
        .execute("select id, email, plan, score, created_at from users order by id")
        .await
        .expect("execute failed");
    println!(
        "query result ({} rows, {}ms):",
        result.rows.len(),
        result.duration_ms
    );
    println!("  columns: {:?}", result.columns);
    for row in &result.rows {
        println!("  {:?}", row);
    }

    assert_eq!(result.rows.len(), 3, "expected the 3 seeded rows");
    assert_eq!(
        result.columns,
        vec!["id", "email", "plan", "score", "created_at"]
    );
    match &result.rows[0][1] {
        serde_json::Value::String(email) => assert_eq!(email, "ada@example.com"),
        other => panic!("expected email to decode as a JSON string, got {other:?}"),
    }
    match &result.rows[0][3] {
        serde_json::Value::Number(_) => {}
        other => panic!("expected score (REAL) to decode as a JSON number, got {other:?}"),
    }
    assert_eq!(
        result.rows[1][3],
        serde_json::Value::Null,
        "grace's score is NULL"
    );
    assert!(
        columns.iter().any(|c| c.name == "id" && c.is_primary_key),
        "id should be flagged as primary key"
    );
    assert!(
        indexes.iter().any(|i| i.name.contains("users") && i.is_unique),
        "the auto-generated unique index backing `email text not null unique` should report is_unique=true"
    );
    println!("\nSELECT rows_affected: {:?}", result.rows_affected);

    let update_result = conn
        .execute("update users set plan = 'pro' where plan = 'pro'")
        .await
        .expect("update failed");
    println!("UPDATE rows_affected: {:?}", update_result.rows_affected);
    assert_eq!(
        update_result.rows_affected,
        Some(2),
        "2 seeded rows already have plan = 'pro'"
    );

    conn.execute(
        "create table orders ( \
            id integer primary key autoincrement, \
            user_id integer not null references users(id), \
            amount real not null \
        )",
    )
    .await
    .expect("create orders table failed");
    conn.execute("insert into orders (user_id, amount) values (1, 42.5)")
        .await
        .expect("seed orders failed");

    let foreign_keys = conn
        .list_foreign_keys("main")
        .await
        .expect("list_foreign_keys failed");
    println!("\nforeign keys in main:");
    for fk in &foreign_keys {
        println!(
            "  {}.{} -> {}.{} ({})",
            fk.from_table, fk.from_column, fk.to_table, fk.to_column, fk.constraint_name
        );
    }
    assert!(
        foreign_keys.iter().any(|fk| fk.from_table == "orders"
            && fk.from_column == "user_id"
            && fk.to_table == "users"
            && fk.to_column == "id"),
        "expected orders.user_id -> users.id foreign key"
    );

    println!("\ncancelling a long-running recursive CTE after 500ms...");
    let cancel_started = Instant::now();
    let (query_result, _) = tokio::join!(
        conn.execute(
            "with recursive counter as ( \
                select 1 as n \
                union all \
                select n + 1 from counter where n < 500000000 \
             ) \
             select count(*) from counter"
        ),
        async {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            conn.cancel().await.expect("cancel failed");
        }
    );
    let elapsed = cancel_started.elapsed();
    println!("query returned after {elapsed:?}: {query_result:?}");
    assert!(
        query_result.is_err(),
        "the cancelled query should return an error, not a result"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(3),
        "expected cancellation to interrupt the recursive CTE well before it finished naturally, took {elapsed:?}"
    );

    conn.close().await.expect("close failed");
    std::fs::remove_file(&path).ok();
    println!("\nsmoke test passed.");
}
