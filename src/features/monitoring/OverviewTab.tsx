import { useEffect, useState } from "react";
import { api } from "@/lib/tauri";
import { useConnectionsStore } from "@/stores/useConnectionsStore";
import { EChart } from "@/components/charts/EChart";
import type { EChartsOption } from "echarts";

const latencyPlaceholder: EChartsOption = {
  grid: { left: 32, right: 12, top: 12, bottom: 24 },
  xAxis: { type: "category", data: ["-50m", "-40m", "-30m", "-20m", "-10m", "now"], axisLine: { lineStyle: { color: "#9a9690" } } },
  yAxis: { type: "value", axisLine: { show: false }, splitLine: { lineStyle: { color: "#e0ddd6" } } },
  series: [
    {
      type: "line",
      smooth: true,
      showSymbol: false,
      data: [12, 14, 11, 40, 22, 15],
      lineStyle: { color: "#0088b0", width: 2 },
      areaStyle: { color: "rgba(0,136,176,0.1)" },
    },
  ],
};

export function OverviewTab({ connectionId }: { connectionId: string }) {
  const connection = useConnectionsStore((s) => s.connections.find((c) => c.id === connectionId));
  const [tableCount, setTableCount] = useState<number | null>(null);
  const [schemaCount, setSchemaCount] = useState<number | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const schemas = await api.listSchemas(connectionId);
      if (cancelled) return;
      setSchemaCount(schemas.length);
      const tablesPerSchema = await Promise.all(schemas.map((s) => api.listTables(connectionId, s.name)));
      if (cancelled) return;
      setTableCount(tablesPerSchema.reduce((sum, t) => sum + t.length, 0));
    })();
    return () => {
      cancelled = true;
    };
  }, [connectionId]);

  return (
    <div className="p-8">
      <div className="flex items-end gap-4">
        <div>
          <h3 className="text-2xl font-semibold">{connection?.name ?? connectionId}</h3>
          <p className="font-mono text-[11.5px] text-[var(--color-neutral-600)]">
            {connection?.engine} · {connection?.host ?? connection?.database}
          </p>
        </div>
      </div>

      <div className="mt-6 grid grid-cols-3 gap-px bg-[var(--color-divider)]">
        <div className="bg-[var(--color-neutral-100)] p-3">
          <div className="font-mono text-[9.5px] tracking-wider text-[var(--color-neutral-600)]">SCHEMAS</div>
          <div className="mt-1 text-2xl font-semibold">{schemaCount ?? "—"}</div>
        </div>
        <div className="bg-[var(--color-neutral-100)] p-3">
          <div className="font-mono text-[9.5px] tracking-wider text-[var(--color-neutral-600)]">TABLES</div>
          <div className="mt-1 text-2xl font-semibold">{tableCount ?? "—"}</div>
        </div>
        <div className="bg-[var(--color-neutral-100)] p-3">
          <div className="font-mono text-[9.5px] tracking-wider text-[var(--color-neutral-600)]">STATUS</div>
          <div className="mt-1 text-2xl font-semibold text-[var(--color-accent-600)]">Connected</div>
        </div>
      </div>

      <div className="mt-8">
        <h5 className="mb-2 font-semibold">Query latency</h5>
        <p className="mb-2 text-[12px] text-[var(--color-neutral-600)]">
          Placeholder series — wire this up to real per-engine stats (pg_stat_statements for
          Postgres, performance_schema for MySQL) once that plumbing exists.
        </p>
        <EChart option={latencyPlaceholder} className="h-48 w-full" />
      </div>
    </div>
  );
}
