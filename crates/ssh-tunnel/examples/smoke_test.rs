//! Proves the SSH tunnel does real, necessary work: connects `PostgresDriver` to a
//! Postgres instance that has NO port published to the host at all — the only path to it
//! is through an SSH bastion container on the same private Docker network. If the tunnel
//! didn't actually forward traffic, this would just time out.
//!
//! Set up the two containers first:
//!   docker network create structly-ssh-net
//!   docker run -d --name structly-tunnel-pg --network structly-ssh-net \
//!     -e POSTGRES_USER=structly -e POSTGRES_PASSWORD=structly_dev_only \
//!     -e POSTGRES_DB=structly_tunnel_test postgres:16
//!   docker run -d --name structly-tunnel-ssh --network structly-ssh-net -p 2222:2222 \
//!     -e PUID=1000 -e PGID=1000 -e PASSWORD_ACCESS=true \
//!     -e USER_NAME=tunneluser -e USER_PASSWORD=tunnel_pw_only \
//!     linuxserver/openssh-server
//!   docker exec -i structly-tunnel-pg psql -U structly -d structly_tunnel_test -c \
//!     "create table users (id serial primary key, email text not null unique); \
//!      insert into users (email) values ('behind-the-tunnel@example.com');"
//!
//! Run with: cargo run -p ssh-tunnel --example smoke_test
//!
//! Host key verification now uses trust-on-first-use against a per-app known_hosts file
//! (see `HostKeyVerifier` in `src/lib.rs`), keyed on `localhost:2222` by default. Since
//! `docker run` gives this bastion container a fresh host key every time, re-running this
//! smoke test against a *new* container on the same host:port will fail with an SSH error
//! after the first run, exactly as it should — delete the recorded entry (or the whole
//! file, printed as `default_known_hosts_path()`'s target — typically
//! `~/Library/Application Support/structly/ssh_known_hosts` on macOS,
//! `~/.local/share/structly/ssh_known_hosts` on Linux) to let it re-learn the new key.
use db_core::{
    ConnectSecrets, ConnectionConfig, DatabaseDriver, Engine, SshAuthMethod, SshTunnelConfig,
};
use std::env;

#[tokio::main]
async fn main() {
    let config = ConnectionConfig {
        id: "tunnel-smoketest".into(),
        name: "Tunnel Smoke Test".into(),
        engine: Engine::Postgres,
        // This is the database's hostname *as seen from inside the SSH container's
        // network* — resolvable only because both containers share the
        // structly-ssh-net Docker network, not from the host running this test.
        host: Some("structly-tunnel-pg".into()),
        port: Some(5432),
        database: "structly_tunnel_test".into(),
        username: Some("structly".into()),
        ssh_tunnel: Some(SshTunnelConfig {
            host: env::var("SSH_HOST").unwrap_or_else(|_| "localhost".into()),
            port: env::var("SSH_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(2222),
            username: env::var("SSH_USER").unwrap_or_else(|_| "tunneluser".into()),
            auth: SshAuthMethod::Password,
        }),
        read_only: false,
    };
    let db_password = env::var("PGPASSWORD").unwrap_or_else(|_| "structly_dev_only".into());
    let ssh_secret = env::var("SSH_PASSWORD").unwrap_or_else(|_| "tunnel_pw_only".into());

    println!(
        "connecting to {}:{} through an SSH tunnel via {:?}:{:?}...",
        config.host.as_deref().unwrap(),
        config.port.unwrap(),
        config.ssh_tunnel.as_ref().map(|t| &t.host),
        config.ssh_tunnel.as_ref().map(|t| t.port)
    );

    let conn = DatabaseDriver::connect(
        &db_postgres::PostgresDriver,
        &config,
        ConnectSecrets {
            db_password: Some(&db_password),
            ssh_secret: Some(&ssh_secret),
        },
    )
    .await
    .expect("connect through tunnel failed");
    println!("connected through the tunnel.\n");

    let result = conn
        .execute("select email from users order by id")
        .await
        .expect("execute failed");
    println!("query result: {:?}", result.rows);

    assert_eq!(result.rows.len(), 1, "expected the 1 seeded row");
    match &result.rows[0][0] {
        serde_json::Value::String(email) => assert_eq!(email, "behind-the-tunnel@example.com"),
        other => panic!("expected email to decode as a JSON string, got {other:?}"),
    }

    conn.close().await.expect("close failed");
    println!("\nsmoke test passed — the query genuinely traveled through the SSH tunnel.");
}
