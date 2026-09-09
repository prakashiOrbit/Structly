//! Exercises PostgresDriver against a real Postgres instance end-to-end: connect, schema
//! introspection, then a SELECT with a decoded result set. Point it at any Postgres via
//! env vars; defaults match the disposable `structly-smoketest-pg` container used in dev.
//!
//! Run with: cargo run -p db-postgres --example smoke_test
use db_core::{ConnectionConfig, DatabaseDriver, Engine};
use std::env;
use std::time::Instant;

#[tokio::main]
async fn main() {
    let config = ConnectionConfig {
        id: "smoketest".into(),
        name: "Smoke Test".into(),
        engine: Engine::Postgres,
        host: Some(env::var("PGHOST").unwrap_or_else(|_| "localhost".into())),
        port: Some(
            env::var("PGPORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(5433),
        ),
        database: env::var("PGDATABASE").unwrap_or_else(|_| "structly_smoketest".into()),
        username: Some(env::var("PGUSER").unwrap_or_else(|_| "structly".into())),
        ssh_tunnel: None,
        read_only: false,
    };
    let password = env::var("PGPASSWORD").unwrap_or_else(|_| "structly_dev_only".into());

    println!(
        "connecting to {}@{:?}:{:?}/{}...",
        config.username.as_deref().unwrap(),
        config.host,
        config.port,
        config.database
    );
    let conn = DatabaseDriver::connect(
        &db_postgres::PostgresDriver,
        &config,
        db_core::ConnectSecrets {
            db_password: Some(&password),
            ssh_secret: None,
        },
    )
    .await
    .expect("connect failed");
    println!("connected.\n");

    let schemas = conn.list_schemas().await.expect("list_schemas failed");
    println!(
        "schemas: {:?}\n",
        schemas.iter().map(|s| &s.name).collect::<Vec<_>>()
    );

    let tables = conn
        .list_tables("public")
        .await
        .expect("list_tables failed");
    println!(
        "tables in public: {:?}\n",
        tables.iter().map(|t| &t.name).collect::<Vec<_>>()
    );

    let columns = conn
        .table_columns("public", "users")
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
        .table_indexes("public", "users")
        .await
        .expect("table_indexes failed");
    println!("indexes on users:");
    for i in &indexes {
        println!("  {} (unique={}) -> {}", i.name, i.is_unique, i.definition);
    }
    println!();

    let result = conn
        .execute("select id, email, plan, created_at from users order by id")
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
    assert_eq!(result.columns, vec!["id", "email", "plan", "created_at"]);
    match &result.rows[0][1] {
        serde_json::Value::String(email) => assert_eq!(email, "ada@example.com"),
        other => panic!("expected email to decode as a JSON string, got {other:?}"),
    }
    assert_eq!(
        result.rows_affected,
        Some(3),
        "Postgres's SELECT command tag also reports a row count"
    );

    let update_result = conn
        .execute("update users set plan = 'pro' where plan = 'pro'")
        .await
        .expect("update failed");
    println!("\nUPDATE rows_affected: {:?}", update_result.rows_affected);
    assert_eq!(
        update_result.rows_affected,
        Some(2),
        "2 seeded rows already have plan = 'pro'"
    );

    // Idempotent so this example can run repeatedly against the same container.
    conn.execute(
        "create table if not exists orders ( \
            id serial primary key, \
            user_id integer not null references users(id), \
            amount numeric(10,2) not null \
        )",
    )
    .await
    .expect("create orders table failed");
    conn.execute("insert into orders (user_id, amount) select 1, 42.50 where not exists (select 1 from orders)")
        .await
        .expect("seed orders failed");

    let foreign_keys = conn
        .list_foreign_keys("public")
        .await
        .expect("list_foreign_keys failed");
    println!("\nforeign keys in public:");
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

    println!("\ncancelling a 5s query after 500ms...");
    let cancel_started = Instant::now();
    let (query_result, _) = tokio::join!(conn.execute("select pg_sleep(5)"), async {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        conn.cancel().await.expect("cancel failed");
    });
    let elapsed = cancel_started.elapsed();
    println!("query returned after {elapsed:?}: {query_result:?}");
    assert!(
        query_result.is_err(),
        "the cancelled query should return an error, not a result"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(3),
        "expected cancellation to interrupt the 5s sleep well before it finished naturally, took {elapsed:?}"
    );

    conn.close().await.expect("close failed");
    println!("\nsmoke test passed.");
}
