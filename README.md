# Structly

A cross-engine database client (Postgres, MySQL, SQLite) built as a Tauri 2 desktop app.

## Layout

```
structly/
├── src/                     React + TypeScript frontend
│   ├── app/                 App shell (layout, top-level state wiring)
│   ├── components/ui/       Shared design-system primitives
│   ├── components/charts/   ECharts wrapper
│   ├── features/            One folder per feature area
│   │   ├── connections/     Connections list/sidebar
│   │   ├── explorer/        Schema tree + ERD diagram (React Flow)
│   │   ├── sql-editor/      Monaco-based SQL editor
│   │   ├── table-view/      Data grid (TanStack Table + Virtual), table structure/indexes
│   │   ├── monitoring/      Connection overview/stats
│   │   └── history/         Query history panel
│   ├── stores/               Zustand stores
│   ├── lib/                  Typed Tauri IPC client (tauri.ts) + shared types
│   └── test/                 Vitest setup
├── src-tauri/                Thin Tauri app: command handlers + app state wiring
├── crates/                   Rust workspace members
│   ├── db-core/              Engine-agnostic traits (DatabaseDriver, DbConnection) + shared types
│   ├── db-postgres/          Postgres driver (sqlx)
│   ├── db-mysql/             MySQL/MariaDB driver (sqlx)
│   ├── db-sqlite/            SQLite driver (sqlx)
│   ├── ssh-tunnel/           SSH port-forwarding for bastion-host connections (russh)
│   ├── secrets/              OS keychain wrapper (keyring) — passwords never touch app-storage
│   └── app-storage/          Local SQLite metadata store: saved connections, query history
└── e2e/                       Playwright smoke tests (run against `vite preview`)
```

## Adding a new database engine

1. Add a `crates/db-<engine>` crate implementing `db_core::DatabaseDriver` / `DbConnection`.
2. Register it in `src-tauri/src/commands/query.rs::driver_for`.
3. Add the engine id to `Engine` in `crates/db-core/src/types.rs` and mirror it in
   `src/lib/types.ts`.

## Development

```sh
npm install
npm run tauri dev       # desktop app with hot reload
```

## Testing

```sh
npm run test            # Vitest (frontend unit tests)
npm run test:e2e        # Playwright (needs `npx playwright install` once)
cargo test --workspace  # Rust unit/integration tests
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Smoke-testing a driver against a real database

Each `db-*` crate has a `smoke_test` example that connects to a real instance, seeds/reads
a `users` table, introspects schemas/tables/columns/indexes, runs a `SELECT`, and asserts
the decoded rows match what was seeded.

```sh
# Postgres — needs a running instance; defaults to a disposable local container
docker run -d --name structly-smoketest-pg \
  -e POSTGRES_USER=structly -e POSTGRES_PASSWORD=structly_dev_only \
  -e POSTGRES_DB=structly_smoketest -p 5433:5432 postgres:16
cargo run -p db-postgres --example smoke_test   # env: PGHOST/PGPORT/PGDATABASE/PGUSER/PGPASSWORD

# MySQL — same idea
docker run -d --name structly-smoketest-mysql \
  -e MYSQL_ROOT_PASSWORD=root_dev_only -e MYSQL_USER=structly \
  -e MYSQL_PASSWORD=structly_dev_only -e MYSQL_DATABASE=structly_smoketest \
  -p 3307:3306 mysql:8
cargo run -p db-mysql --example smoke_test      # env: MYSQL_HOST/MYSQL_PORT/MYSQL_DATABASE/MYSQL_USER/MYSQL_PASSWORD

# SQLite — no server needed, creates a throwaway file under the OS temp dir and deletes it after
cargo run -p db-sqlite --example smoke_test

# SSH tunnel — proves it does real work: a Postgres instance with NO port published to the
# host at all, reachable only through an SSH bastion on the same private Docker network.
docker network create structly-ssh-net
docker run -d --name structly-tunnel-pg --network structly-ssh-net \
  -e POSTGRES_USER=structly -e POSTGRES_PASSWORD=structly_dev_only \
  -e POSTGRES_DB=structly_tunnel_test postgres:16
docker run -d --name structly-tunnel-ssh --network structly-ssh-net -p 2222:2222 \
  -e PUID=1000 -e PGID=1000 -e PASSWORD_ACCESS=true \
  -e USER_NAME=tunneluser -e USER_PASSWORD=tunnel_pw_only \
  linuxserver/openssh-server
# AllowTcpForwarding defaults to "no" in this image — flip it on and restart:
docker exec structly-tunnel-ssh sed -i 's/^AllowTcpForwarding no/AllowTcpForwarding yes/' /config/sshd/sshd_config
docker restart structly-tunnel-ssh
docker exec -i structly-tunnel-pg psql -U structly -d structly_tunnel_test -c \
  "create table users (id serial primary key, email text not null unique); \
   insert into users (email) values ('behind-the-tunnel@example.com');"
cargo run -p ssh-tunnel --example smoke_test
```

## Known gaps (tracked as TODOs in code)

- Row decoding is implemented for all three engines (`crates/db-{postgres,mysql,sqlite}/src/lib.rs`),
  covering bool/int/float/numeric-or-decimal/text/json/date/time/datetime, with a
  best-effort text decode or NULL fallback for anything else (arrays, BLOB/BYTEA, spatial
  types, custom/composite types, ...). Each engine needed its own quirks worked out:
  - MySQL 8's `information_schema` reports several text columns as `VARBINARY` at the wire
    level, which fails a plain `String` decode — worked around with `CAST(... AS CHAR)`.
  - MySQL's `TIMESTAMP` (UTC-normalized) and `DATETIME` (naive) need different Rust decode
    types (`DateTime<Utc>` vs `NaiveDateTime`) even though both show up as similar SQL types.
  - SQLite is dynamically typed per-*value*, not per-column, so the type used to decode a
    whole result column is really just "whatever the first row happened to store" — a
    column with no declared affinity could hold a different type on a later row.
- `rows_affected` is implemented for all three engines. `execute()` now uses
  `sqlx::raw_sql(sql).fetch_many(pool)` instead of `fetch_all` — `fetch_many` streams back
  both rows *and* the command-complete metadata (`fetch_all` throws the latter away), and
  `raw_sql` (not `query`) is the non-deprecated API for unparameterized text in sqlx 0.8.
  One cross-engine quirk worth knowing: Postgres and SQLite both report a SELECT's row
  count as `rows_affected` too (matching `rows.len()`), but MySQL always reports `0` for
  SELECT — only trust `rows_affected` for statements that return no rows.
- `ssh-tunnel` is implemented: real SSH local port-forwarding (russh), wired into
  `db-postgres`/`db-mysql`'s `connect()` via a shared `ssh_tunnel::maybe_open_tunnel()`
  helper, with SSH tunnel fields exposed in the "New Connection" dialog (password auth
  only in the UI; `SshAuthMethod::PrivateKey` works at the driver level for anyone
  constructing a `ConnectionConfig` directly, e.g. in tests).
  - Each accepted local TCP connection opens its own SSH `direct-tcpip` channel (mirrors
    `ssh -L`), bridged with `tokio::io::copy_bidirectional` via `Channel::into_stream()` —
    lets several concurrent DB connections share one SSH session.
  - `connect()` on both drivers acquires the tunnel *before* building the connection URL,
    then points the driver at `127.0.0.1:<tunnel.local_port>` instead of the real
    host:port; the `SshTunnel` is kept alive as a field on the connection struct for as
    long as the DB connection is (dropping it tears down the forward).
  - Verified against a database with **no port published to the host at all** — reachable
    only by a container on the same private Docker network as an SSH bastion. If the
    tunnel didn't do real work, this would just time out; instead the full Postgres wire
    protocol (handshake, auth, query) round-trips through it correctly.
  - **Host key verification**: the SSH host key is checked against a per-app known_hosts
    file (`crates/ssh-tunnel/src/lib.rs::HostKeyVerifier`, stored under the OS data dir,
    e.g. `~/Library/Application Support/structly/ssh_known_hosts` on macOS — not the user's
    own `~/.ssh/known_hosts`), using trust-on-first-use: the first connection to a given
    host:port records its key, and every later connection must present that exact key.
    A changed key, or a second key under a different algorithm, fails the connection
    instead of being silently accepted — that mismatch is the strongest MITM signal there
    is. SSH certificate-based host auth isn't supported and is rejected outright, since
    the known_hosts store has no way to express "trust this CA". Known limitation: TOFU
    doesn't protect the very first connection to a host if an attacker is already on-path
    at that moment — same trade-off `ssh -o StrictHostKeyChecking=accept-new` makes.
  - The image used for the test SSH server (`linuxserver/openssh-server`) ships with
    `AllowTcpForwarding no` by default — has to be flipped on for tunneling to work at all;
    this is standard SSH server config, not a bug in the tunnel code, but worth knowing if
    a real bastion host similarly has forwarding disabled.
- Query cancellation (`DbConnection::cancel`) is implemented for all three engines, wired
  up to a real Cancel button in the SQL editor. Each engine needed a genuinely different
  mechanism, and getting there required an architectural fix along the way:
  - **The fix**: Tauri's `execute_query`/`disconnect` commands used to hold the
    `live_connections` map's mutex guard for the *entire* call into the driver — so a
    concurrent `cancel_query` for that same connection could never even acquire the lock
    to reach it. `AppState.live_connections` now stores `Arc<dyn DbConnection>` instead of
    `Box<dyn DbConnection>`; every command clones the `Arc` and drops the map lock before
    awaiting on the connection, so other commands (cancel included) stay unblocked.
  - **Postgres**: `execute()` now acquires one dedicated pooled connection (not just
    `&self.pool`, which could hand different calls different physical connections), reads
    its backend PID via `pg_backend_pid()`, and stashes it in a `Mutex<Option<i32>>` on the
    struct. `cancel()` reads that PID and runs `pg_cancel_backend($1)` on a *different*
    pooled connection.
  - **MySQL**: same shape, using `connection_id()` instead of a PID, and `KILL QUERY <id>`
    to cancel. `KILL` isn't supported over the prepared-statement protocol `sqlx::query()`
    uses, so it has to go through `raw_sql` (plain text protocol) — same requirement
    `execute()` already had for a different reason.
  - **SQLite**: no side-channel connection exists to send a cancel through (it's a single
    embedded connection). Instead, `connect()` grabs the raw C `sqlite3*` handle once via
    sqlx's `lock_handle()`/`as_raw_handle()` escape hatch, and `cancel()` calls
    `sqlite3_interrupt()` on it directly through `libsqlite3-sys` — bypassing sqlx's async
    connection machinery entirely, which is exactly why it works even while that machinery
    is busy running the query being cancelled. Safe by SQLite's own documented contract:
    `sqlite3_interrupt` is designed to be called from another thread while a statement is
    in flight.
  - Verified against real cancellation, not just a driver returning `Ok(())`: each smoke
    test races a genuinely long query (Postgres `pg_sleep(5)`, SQLite a big `WITH
    RECURSIVE`) against a cancel fired 500ms in, and asserts the query comes back with an
    error in well under its natural runtime. MySQL needed a specific kind of long query —
    `SLEEP()` and `BENCHMARK()` both turned out to swallow `KILL QUERY` into a benign
    return value instead of an error (documented behavior for `SLEEP()`, empirically true
    for `BENCHMARK()` too in this version) — a big recursive CTE (a real row-scan, which
    the execution engine checks for kills between rows) surfaces the expected MySQL error
    1317 instead, representative of killing an actual slow `SELECT`/`JOIN`.
  - Known limitation shared by every PID/session-id-based cancel protocol (Postgres and
    MySQL here, and this isn't unique to us — pgAdmin/DBeaver have the same edge case): if
    the target query finishes and that PID/connection-id gets reused by an unrelated
    session in the narrow window before `cancel()` runs, it could in theory cancel the
    wrong thing. Low risk in practice at human-driven "click Cancel" timescales.
- Foreign-key introspection is implemented for all three engines
  (`DbConnection::list_foreign_keys`, exposed as the `list_foreign_keys` Tauri command) and
  `SchemaDiagram` renders the edges — open it from Explorer's "ERD" link next to a schema.
  Each engine needed a different query shape:
  - Postgres needs `information_schema.table_constraints` joined through
    `key_column_usage` *and* `constraint_column_usage` to get both sides of the FK.
  - MySQL's `key_column_usage` already carries `referenced_table_name`/
    `referenced_column_name` directly — no second join needed (same `VARBINARY`
    `CAST(... AS CHAR)` workaround as the other MySQL introspection queries applies here too).
  - SQLite has no schema-wide "list all FKs" pragma — `pragma_foreign_key_list` is
    per-table, so `list_foreign_keys` fans out over every table in the schema instead of
    running one query.
- The webview's Content-Security-Policy (`src-tauri/tauri.conf.json`) is locked down —
  `default-src 'self'` with no third-party origins anywhere, including script-src. That
  required self-hosting Monaco: `@monaco-editor/react` fetches Monaco from a CDN by
  default, which would otherwise have needed a CDN origin in the policy (and made the SQL
  editor depend on network access). `src/lib/monaco-setup.ts` instead points its loader at
  the bundled `monaco-editor` package and registers a same-origin editor worker, so nothing
  in the app fetches code from outside its own bundle. `style-src` keeps `'unsafe-inline'`
  — needed for inline `style=` attributes (TanStack Virtual's row positioning, Monaco's own
  injected theme CSS), which is a much smaller relaxation than allowing inline/remote
  scripts.
