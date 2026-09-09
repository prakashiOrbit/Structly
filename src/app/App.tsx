import { useState } from "react";
import { clsx } from "clsx";
import { useTabsStore } from "@/stores/useTabsStore";
import { ConnectionsPanel } from "@/features/connections/ConnectionsPanel";
import { NewConnectionDialog } from "@/features/connections/NewConnectionDialog";
import { ExplorerPanel } from "@/features/explorer/ExplorerPanel";
import { HistoryPanel } from "@/features/history/HistoryPanel";
import { OverviewTab } from "@/features/monitoring/OverviewTab";
import { TableTab } from "@/features/table-view/TableTab";
import { SqlEditor } from "@/features/sql-editor/SqlEditor";
import { SchemaDiagram } from "@/features/explorer/SchemaDiagram";
import { Button } from "@/components/ui/Button";

type SidebarPane = "connections" | "explorer" | "history";

export default function App() {
  const [pane, setPane] = useState<SidebarPane>("connections");
  const [showNewConnection, setShowNewConnection] = useState(false);
  const { tabs, activeTabId, setActiveTab, closeTab } = useTabsStore();
  const activeTab = tabs.find((t) => t.id === activeTabId);

  return (
    <div className="flex h-screen flex-col overflow-hidden bg-[var(--color-bg)] text-[var(--color-text)]">
      <div className="flex h-11 flex-none items-center gap-2 border-b border-[var(--color-divider)] bg-[var(--color-surface)] px-3">
        <img src="/logo.png" alt="" className="h-5 w-5" />
        <span className="font-semibold tracking-tight">Structly</span>
      </div>

      <div className="flex min-h-0 flex-1">
        <div className="flex w-[276px] flex-none flex-col border-r border-[var(--color-divider)] bg-[var(--color-surface)]">
          <div className="flex gap-0 p-2 pb-1.5">
            {(["connections", "explorer", "history"] as const).map((p) => (
              <button
                key={p}
                onClick={() => setPane(p)}
                className={clsx(
                  "flex-1 rounded-sm py-1 text-[11px] font-semibold uppercase tracking-wide",
                  pane === p
                    ? "bg-[var(--color-accent-100)] text-[var(--color-accent-700)]"
                    : "text-[var(--color-neutral-600)]",
                )}
              >
                {p}
              </button>
            ))}
          </div>
          <div className="min-h-0 flex-1 overflow-auto px-2 pb-2">
            {pane === "connections" && <ConnectionsPanel />}
            {pane === "explorer" && <ExplorerPanel />}
            {pane === "history" && <HistoryPanel />}
          </div>
          <div className="flex-none p-2">
            <Button variant="secondary" className="w-full justify-center" onClick={() => setShowNewConnection(true)}>
              + New Connection
            </Button>
          </div>
        </div>

        <div className="flex min-w-0 flex-1 flex-col bg-[var(--color-neutral-100)]">
          <div className="flex h-8 flex-none items-stretch overflow-x-auto bg-[var(--color-surface)]">
            {tabs.map((tab) => (
              <div
                key={tab.id}
                onClick={() => setActiveTab(tab.id)}
                className={clsx(
                  "flex max-w-[230px] cursor-pointer items-center gap-2 border-t-2 px-2.5 text-[12.5px]",
                  tab.id === activeTabId
                    ? "border-[var(--color-accent-600)] bg-[var(--color-neutral-100)]"
                    : "border-transparent",
                )}
              >
                <span className="truncate">{tab.label}</span>
                <span
                  onClick={(e) => {
                    e.stopPropagation();
                    closeTab(tab.id);
                  }}
                  className="grid h-3.5 w-3.5 place-items-center rounded-sm text-[11px] text-[var(--color-neutral-600)] hover:bg-[var(--color-neutral-200)]"
                >
                  ✕
                </span>
              </div>
            ))}
          </div>

          <div className="min-h-0 flex-1 overflow-auto">
            {!activeTab && (
              <div className="flex h-full items-center justify-center text-[var(--color-neutral-600)]">
                Select a connection to get started.
              </div>
            )}
            {activeTab?.kind === "overview" && <OverviewTab connectionId={activeTab.connectionId} />}
            {activeTab?.kind === "table" && activeTab.target && (
              <TableTab connectionId={activeTab.connectionId} target={activeTab.target} />
            )}
            {activeTab?.kind === "sql" && <SqlEditor connectionId={activeTab.connectionId} />}
            {activeTab?.kind === "erd" && activeTab.target && (
              <SchemaDiagram connectionId={activeTab.connectionId} schema={activeTab.target} />
            )}
          </div>
        </div>
      </div>

      <NewConnectionDialog open={showNewConnection} onClose={() => setShowNewConnection(false)} />
    </div>
  );
}
