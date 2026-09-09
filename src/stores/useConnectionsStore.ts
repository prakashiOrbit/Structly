import { create } from "zustand";
import { api } from "@/lib/tauri";
import type { ConnectionConfig, Engine, SavedConnection, SshTunnelConfig } from "@/lib/types";

export interface NewSshTunnelInput {
  host: string;
  port: number;
  username: string;
  /** Only password auth is exposed in the UI today; SshAuthMethod::PrivateKey still
   * works at the driver level (crates/ssh-tunnel) for anyone constructing it directly. */
  password: string;
}

export interface NewConnectionInput {
  name: string;
  engine: Engine;
  host: string | null;
  port: number | null;
  database: string;
  username: string | null;
  password: string | null;
  sshTunnel: NewSshTunnelInput | null;
  readOnly: boolean;
}

/** SavedConnection carries no secrets — the live connect step looks them up from the
 * keychain by id (see src-tauri/src/commands/query.rs::connect). */
function toConnectionConfig(c: SavedConnection): ConnectionConfig {
  const sshTunnel: SshTunnelConfig | null = c.ssh_tunnel_json ? JSON.parse(c.ssh_tunnel_json) : null;
  return {
    id: c.id,
    name: c.name,
    engine: c.engine as Engine,
    host: c.host,
    port: c.port,
    database: c.database,
    username: c.username,
    ssh_tunnel: sshTunnel,
    read_only: c.read_only,
  };
}

interface ConnectionsState {
  connections: SavedConnection[];
  activeConnectionId: string | null;
  connectingId: string | null;
  loading: boolean;
  error: string | null;
  connectError: string | null;
  load: () => Promise<void>;
  select: (id: string | null) => void;
  remove: (id: string) => Promise<void>;
  create: (input: NewConnectionInput) => Promise<string>;
  connect: (id: string) => Promise<boolean>;
}

export const useConnectionsStore = create<ConnectionsState>((set, get) => ({
  connections: [],
  activeConnectionId: null,
  connectingId: null,
  loading: false,
  error: null,
  connectError: null,

  load: async () => {
    set({ loading: true, error: null });
    try {
      const connections = await api.listSavedConnections();
      set({ connections, loading: false });
    } catch (err) {
      set({ error: String(err), loading: false });
    }
  },

  select: (id) => set({ activeConnectionId: id }),

  remove: async (id) => {
    await api.deleteConnection(id);
    const isActive = get().activeConnectionId === id;
    set((state) => ({
      connections: state.connections.filter((c) => c.id !== id),
      activeConnectionId: isActive ? null : state.activeConnectionId,
    }));
  },

  create: async (input) => {
    const id = crypto.randomUUID();
    const sshTunnel: SshTunnelConfig | null = input.sshTunnel
      ? {
          host: input.sshTunnel.host,
          port: input.sshTunnel.port,
          username: input.sshTunnel.username,
          auth: { method: "password" },
        }
      : null;
    const saved: SavedConnection = {
      id,
      name: input.name,
      engine: input.engine,
      host: input.host,
      port: input.port,
      database: input.database,
      username: input.username,
      ssh_tunnel_json: sshTunnel ? JSON.stringify(sshTunnel) : null,
      read_only: input.readOnly,
    };
    await api.saveConnection(saved, input.password ?? undefined, input.sshTunnel?.password);
    set((state) => ({ connections: [...state.connections, saved] }));
    return id;
  },

  connect: async (id) => {
    const saved = get().connections.find((c) => c.id === id);
    if (!saved) {
      set({ connectError: `Unknown connection: ${id}` });
      return false;
    }
    set({ connectingId: id, connectError: null });
    try {
      await api.connect(toConnectionConfig(saved));
      set({ connectingId: null, activeConnectionId: id });
      return true;
    } catch (err) {
      set({ connectingId: null, connectError: String(err) });
      return false;
    }
  },
}));
