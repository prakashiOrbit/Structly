import { useEffect } from "react";
import { useConnectionsStore } from "@/stores/useConnectionsStore";
import { useTabsStore } from "@/stores/useTabsStore";
import { EngineBadge } from "@/components/ui/EngineBadge";

export function ConnectionsPanel() {
  const { connections, loading, error, connectingId, connectError, load, connect, remove } = useConnectionsStore();
  const openTab = useTabsStore((s) => s.openTab);

  useEffect(() => {
    load();
  }, [load]);

  const openOverview = async (connectionId: string, name: string) => {
    const connected = await connect(connectionId);
    if (connected) {
      openTab({ id: `overview:${connectionId}`, kind: "overview", label: name, connectionId });
    }
  };

  if (loading) {
    return <p className="px-2 py-3 text-xs text-[var(--color-neutral-600)]">Loading connections…</p>;
  }

  if (error) {
    return <p className="px-2 py-3 text-xs text-[var(--color-accent-2-700)]">{error}</p>;
  }

  if (connections.length === 0) {
    return (
      <div className="px-2 py-6 text-center text-xs text-[var(--color-neutral-600)]">
        No connections yet. Add one to get started.
      </div>
    );
  }

  return (
    <div className="space-y-1">
      {connectError && <p className="px-1.5 py-1 text-[11px] text-[var(--color-accent-2-700)]">{connectError}</p>}
      {connections.map((c) => {
        const isConnecting = connectingId === c.id;
        return (
          <div
            key={c.id}
            onClick={() => !isConnecting && openOverview(c.id, c.name)}
            className="flex cursor-pointer items-start gap-2 rounded-sm px-1.5 py-1 hover:bg-[var(--color-neutral-200)]"
            style={{ opacity: isConnecting ? 0.6 : 1 }}
          >
            <span className="mt-1.5 h-1.5 w-1.5 flex-none rounded-full bg-[var(--color-accent-500)]" />
            <span className="min-w-0 flex-1">
              <span className="block truncate font-semibold text-[12.5px]">{c.name}</span>
              <span className="block truncate font-mono text-[10px] text-[var(--color-neutral-600)]">
                {isConnecting ? "Connecting…" : (c.host ?? c.database)}
              </span>
            </span>
            <EngineBadge engine={c.engine} />
            <span
              onClick={(e) => {
                e.stopPropagation();
                remove(c.id);
              }}
              title="Remove connection"
              className="grid h-4 w-4 flex-none place-items-center rounded-sm text-[11px] text-[var(--color-neutral-500)] hover:bg-[var(--color-neutral-300)] hover:text-[var(--color-text)]"
            >
              ✕
            </span>
          </div>
        );
      })}
    </div>
  );
}
