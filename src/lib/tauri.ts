// Typed wrappers around the Tauri commands registered in src-tauri/src/commands/.
// Nothing here should reach for `invoke` directly outside this file — it's the one
// place that has to stay in sync with the Rust side.
import { invoke } from "@tauri-apps/api/core";
import type {
  ColumnMeta,
  ConnectionConfig,
  ForeignKeyMeta,
  HistoryEntry,
  IndexMeta,
  QueryResult,
  SavedConnection,
  SchemaMeta,
  TableMeta,
} from "./types";

export const api = {
  listEngines: () => invoke<string[]>("list_engines"),

  listSavedConnections: () => invoke<SavedConnection[]>("list_saved_connections"),
  saveConnection: (connection: SavedConnection, password?: string, sshSecret?: string) =>
    invoke<void>("save_connection", { connection, password, sshSecret }),
  deleteConnection: (id: string) => invoke<void>("delete_connection", { id }),

  connect: (config: ConnectionConfig) => invoke<void>("connect", { config }),
  disconnect: (connectionId: string) => invoke<void>("disconnect", { connectionId }),
  executeQuery: (connectionId: string, sql: string) =>
    invoke<QueryResult>("execute_query", { connectionId, sql }),
  cancelQuery: (connectionId: string) => invoke<void>("cancel_query", { connectionId }),

  listSchemas: (connectionId: string) => invoke<SchemaMeta[]>("list_schemas", { connectionId }),
  listTables: (connectionId: string, schema: string) =>
    invoke<TableMeta[]>("list_tables", { connectionId, schema }),
  tableColumns: (connectionId: string, schema: string, table: string) =>
    invoke<ColumnMeta[]>("table_columns", { connectionId, schema, table }),
  tableIndexes: (connectionId: string, schema: string, table: string) =>
    invoke<IndexMeta[]>("table_indexes", { connectionId, schema, table }),
  listForeignKeys: (connectionId: string, schema: string) =>
    invoke<ForeignKeyMeta[]>("list_foreign_keys", { connectionId, schema }),

  listHistory: (connectionId: string, limit = 50) =>
    invoke<HistoryEntry[]>("list_history", { connectionId, limit }),
};
