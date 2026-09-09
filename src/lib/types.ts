// Mirrors the shapes serialized by db-core and app-storage on the Rust side
// (crates/db-core/src/types.rs, crates/app-storage/src/lib.rs). Keep in sync by hand for now;
// worth generating from Rust via ts-rs once the shapes stabilize.

export type Engine = "postgres" | "my_sql" | "sqlite";

// Internally-tagged enum (#[serde(tag = "method")] on the Rust side): a unit variant
// serializes as just `{ method: "password" }`, a struct variant merges its fields in
// alongside the tag, e.g. `{ method: "private_key", path: "..." }`.
export type SshAuthMethod = { method: "password" } | { method: "private_key"; path: string };

export interface SshTunnelConfig {
  host: string;
  port: number;
  username: string;
  auth: SshAuthMethod;
}

export interface ConnectionConfig {
  id: string;
  name: string;
  engine: Engine;
  host: string | null;
  port: number | null;
  database: string;
  username: string | null;
  // Rust's ConnectionConfig has no #[serde(rename_all)], so these are the literal
  // (snake_case) field names — Tauri's camelCase auto-conversion only applies to
  // top-level command argument names, not to fields inside a serialized struct.
  ssh_tunnel: SshTunnelConfig | null;
  read_only: boolean;
}

export interface SavedConnection {
  id: string;
  name: string;
  engine: string;
  host: string | null;
  port: number | null;
  database: string;
  username: string | null;
  ssh_tunnel_json: string | null;
  read_only: boolean;
}

export interface SchemaMeta {
  name: string;
}

export interface TableMeta {
  schema: string;
  name: string;
  approx_row_count: number | null;
}

export interface ColumnMeta {
  name: string;
  data_type: string;
  nullable: boolean;
  is_primary_key: boolean;
  default: string | null;
}

export interface IndexMeta {
  name: string;
  definition: string;
  is_unique: boolean;
}

export interface ForeignKeyMeta {
  constraint_name: string;
  from_table: string;
  from_column: string;
  to_table: string;
  to_column: string;
}

export interface HistoryEntry {
  id: number;
  connection_id: string;
  sql: string;
  duration_ms: number;
  status: string;
  executed_at: string;
}

export interface QueryResult {
  columns: string[];
  rows: unknown[][];
  rows_affected: number | null;
  duration_ms: number;
}
