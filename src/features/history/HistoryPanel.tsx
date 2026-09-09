import { useEffect, useState } from "react";
import { api } from "@/lib/tauri";
import type { HistoryEntry } from "@/lib/types";
import { useConnectionsStore } from "@/stores/useConnectionsStore";

export function HistoryPanel() {
  const activeConnectionId = useConnectionsStore((s) => s.activeConnectionId);
  const [entries, setEntries] = useState<HistoryEntry[] | null>(null);

  useEffect(() => {
    if (!activeConnectionId) {
      setEntries(null);
      return;
    }
    api.listHistory(activeConnectionId).then(setEntries);
  }, [activeConnectionId]);

  if (!activeConnectionId) {
    return <p className="px-2 py-3 text-xs text-[var(--color-neutral-600)]">Select a connection first.</p>;
  }

  if (entries === null) {
    return <p className="px-2 py-3 text-xs text-[var(--color-neutral-600)]">Loading…</p>;
  }

  if (entries.length === 0) {
    return <p className="px-2 py-3 text-xs text-[var(--color-neutral-600)]">No queries yet.</p>;
  }

  return (
    <div className="space-y-1">
      {entries.map((entry) => (
        <div key={entry.id} className="rounded-sm border-l-2 border-[var(--color-accent-500)] px-1.5 py-1">
          <div className="line-clamp-2 font-mono text-[11px] leading-snug">{entry.sql}</div>
          <div className="mt-0.5 flex items-center gap-2 font-mono text-[9.5px] text-[var(--color-neutral-600)]">
            <span>{entry.duration_ms}ms</span>
            <span className="ml-auto">{entry.executed_at}</span>
          </div>
        </div>
      ))}
    </div>
  );
}
