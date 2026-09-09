import { useEffect, useState } from "react";
import { ReactFlow, Background, Controls, MarkerType, type Edge, type Node } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { api } from "@/lib/tauri";

/** One node per table, laid out in a simple grid, with an edge per foreign key. */
export function SchemaDiagram({ connectionId, schema }: { connectionId: string; schema: string }) {
  const [nodes, setNodes] = useState<Node[]>([]);
  const [edges, setEdges] = useState<Edge[]>([]);

  useEffect(() => {
    Promise.all([api.listTables(connectionId, schema), api.listForeignKeys(connectionId, schema)]).then(
      ([tables, foreignKeys]) => {
        const perRow = 5;
        setNodes(
          tables.map((t, i) => ({
            id: t.name,
            position: { x: (i % perRow) * 200, y: Math.floor(i / perRow) * 120 },
            data: { label: t.name },
            style: {
              fontFamily: "var(--font-mono)",
              fontSize: 12,
              border: "1px solid var(--color-divider)",
              background: "var(--color-surface)",
            },
          })),
        );

        setEdges(
          foreignKeys.map((fk, i) => ({
            id: `${fk.constraint_name}-${i}`,
            source: fk.from_table,
            target: fk.to_table,
            label: `${fk.from_column} → ${fk.to_column}`,
            labelStyle: { fontFamily: "var(--font-mono)", fontSize: 10 },
            style: { stroke: "var(--color-accent-500)" },
            markerEnd: { type: MarkerType.ArrowClosed, color: "var(--color-accent-500)" },
          })),
        );
      },
    );
  }, [connectionId, schema]);

  return (
    <div className="h-full w-full">
      <ReactFlow nodes={nodes} edges={edges} fitView>
        <Background />
        <Controls />
      </ReactFlow>
    </div>
  );
}
