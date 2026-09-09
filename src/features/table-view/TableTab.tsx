import { useEffect, useState } from "react";
import { api } from "@/lib/tauri";
import type { ColumnMeta, IndexMeta, QueryResult } from "@/lib/types";
import { ResultsGrid } from "./ResultsGrid";

type SubTab = "data" | "structure" | "indexes";

export function TableTab({ connectionId, target }: { connectionId: string; target: string }) {
  const [schema, table] = target.split(".");
  const [subTab, setSubTab] = useState<SubTab>("data");
  const [data, setData] = useState<QueryResult | null>(null);
  const [columns, setColumns] = useState<ColumnMeta[] | null>(null);
  const [indexes, setIndexes] = useState<IndexMeta[] | null>(null);

  useEffect(() => {
    setData(null);
    setColumns(null);
    setIndexes(null);
  }, [target]);

  useEffect(() => {
    if (subTab === "data" && data === null) {
      api.executeQuery(connectionId, `SELECT * FROM "${schema}"."${table}" LIMIT 500`).then(setData);
    }
    if (subTab === "structure" && columns === null) {
      api.tableColumns(connectionId, schema, table).then(setColumns);
    }
    if (subTab === "indexes" && indexes === null) {
      api.tableIndexes(connectionId, schema, table).then(setIndexes);
    }
  }, [subTab, connectionId, schema, table, data, columns, indexes]);

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex flex-none gap-4 border-b border-[var(--color-divider)] px-4 pt-2">
        {(["data", "structure", "indexes"] as const).map((tab) => (
          <span
            key={tab}
            onClick={() => setSubTab(tab)}
            className="cursor-pointer pb-1.5 text-[12.5px] font-semibold capitalize"
            style={{
              boxShadow: subTab === tab ? "inset 0 -2px 0 var(--color-accent-600)" : undefined,
              color: subTab === tab ? "var(--color-text)" : "var(--color-neutral-600)",
            }}
          >
            {tab}
          </span>
        ))}
      </div>

      <div className="min-h-0 flex-1 overflow-auto">
        {subTab === "data" && data && <ResultsGrid result={data} />}

        {subTab === "structure" && columns && (
          <table className="w-full font-mono text-[12px]">
            <thead>
              <tr className="text-left">
                <th className="px-2 py-1">Column</th>
                <th className="px-2 py-1">Type</th>
                <th className="px-2 py-1">Nullable</th>
                <th className="px-2 py-1">Default</th>
              </tr>
            </thead>
            <tbody>
              {columns.map((c) => (
                <tr key={c.name} className="border-t border-[var(--color-divider)]">
                  <td className="px-2 py-1 font-medium">
                    {c.is_primary_key ? "🔑 " : ""}
                    {c.name}
                  </td>
                  <td className="px-2 py-1 text-[var(--color-accent-800)]">{c.data_type}</td>
                  <td className="px-2 py-1">{c.nullable ? "YES" : "NO"}</td>
                  <td className="px-2 py-1 text-[var(--color-neutral-600)]">{c.default ?? "—"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}

        {subTab === "indexes" && indexes && (
          <table className="w-full font-mono text-[12px]">
            <thead>
              <tr className="text-left">
                <th className="px-2 py-1">Index</th>
                <th className="px-2 py-1">Definition</th>
                <th className="px-2 py-1">Unique</th>
              </tr>
            </thead>
            <tbody>
              {indexes.map((i) => (
                <tr key={i.name} className="border-t border-[var(--color-divider)]">
                  <td className="px-2 py-1 font-medium">{i.name}</td>
                  <td className="px-2 py-1 text-[var(--color-neutral-600)]">{i.definition}</td>
                  <td className="px-2 py-1">{i.is_unique ? "YES" : "NO"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}
