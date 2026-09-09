import { useEffect, useState } from "react";
import { api } from "@/lib/tauri";
import type { SchemaMeta, TableMeta } from "@/lib/types";
import { useConnectionsStore } from "@/stores/useConnectionsStore";
import { useTabsStore } from "@/stores/useTabsStore";

function SchemaNode({ connectionId, schema }: { connectionId: string; schema: SchemaMeta }) {
  const [open, setOpen] = useState(false);
  const [tables, setTables] = useState<TableMeta[] | null>(null);
  const openTab = useTabsStore((s) => s.openTab);

  const toggle = async () => {
    const next = !open;
    setOpen(next);
    if (next && tables === null) {
      setTables(await api.listTables(connectionId, schema.name));
    }
  };

  const openErd = (e: React.MouseEvent) => {
    e.stopPropagation();
    openTab({
      id: `erd:${connectionId}:${schema.name}`,
      kind: "erd",
      label: `${schema.name} · ERD`,
      connectionId,
      target: schema.name,
    });
  };

  return (
    <div>
      <div
        onClick={toggle}
        className="flex cursor-pointer items-center gap-1.5 px-1.5 py-0.5 font-medium"
      >
        <span className="w-2.5 text-[10px]">{open ? "▾" : "▸"}</span>
        <span className="flex-1">{schema.name}</span>
        <span
          onClick={openErd}
          title="View ERD"
          className="rounded-sm px-1 text-[10px] font-normal text-[var(--color-neutral-600)] hover:bg-[var(--color-neutral-200)] hover:text-[var(--color-accent-700)]"
        >
          ERD
        </span>
      </div>
      {open && (
        <div className="pl-5">
          {tables === null && <div className="px-1.5 py-0.5 text-[var(--color-neutral-600)]">Loading…</div>}
          {tables?.map((t) => (
            <div
              key={t.name}
              onClick={() =>
                openTab({
                  id: `table:${connectionId}:${schema.name}.${t.name}`,
                  kind: "table",
                  label: t.name,
                  connectionId,
                  target: `${schema.name}.${t.name}`,
                })
              }
              className="flex cursor-pointer items-center gap-1.5 rounded-sm px-1.5 py-0.5 hover:bg-[var(--color-neutral-200)]"
            >
              <span className="flex-1 truncate">{t.name}</span>
              {t.approx_row_count != null && (
                <span className="text-[9.5px] text-[var(--color-neutral-500)]">{t.approx_row_count}</span>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export function ExplorerPanel() {
  const activeConnectionId = useConnectionsStore((s) => s.activeConnectionId);
  const [schemas, setSchemas] = useState<SchemaMeta[] | null>(null);

  useEffect(() => {
    if (!activeConnectionId) {
      setSchemas(null);
      return;
    }
    setSchemas(null);
    api.listSchemas(activeConnectionId).then(setSchemas);
  }, [activeConnectionId]);

  if (!activeConnectionId) {
    return <p className="px-2 py-3 text-xs text-[var(--color-neutral-600)]">Select a connection first.</p>;
  }

  if (schemas === null) {
    return <p className="px-2 py-3 text-xs text-[var(--color-neutral-600)]">Loading schema…</p>;
  }

  return (
    <div className="font-mono text-[12px]">
      {schemas.map((s) => (
        <SchemaNode key={s.name} connectionId={activeConnectionId} schema={s} />
      ))}
    </div>
  );
}
