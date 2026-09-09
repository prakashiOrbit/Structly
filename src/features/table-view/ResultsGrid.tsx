import { useRef } from "react";
import { createColumnHelper, flexRender, getCoreRowModel, useReactTable } from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { QueryResult } from "@/lib/types";

const columnHelper = createColumnHelper<unknown[]>();

/**
 * Renders a query result as a virtualized grid — needed once result sets run into
 * the thousands of rows (this is a DB client; that's the common case, not the edge case).
 */
export function ResultsGrid({ result }: { result: QueryResult }) {
  const parentRef = useRef<HTMLDivElement>(null);

  const columns = result.columns.map((name, i) =>
    columnHelper.accessor((row) => row[i], {
      id: name,
      header: name,
      cell: (info) => String(info.getValue() ?? "∅"),
    }),
  );

  const table = useReactTable({
    data: result.rows,
    columns,
    getCoreRowModel: getCoreRowModel(),
  });

  const rows = table.getRowModel().rows;
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 26,
    overscan: 12,
  });

  return (
    <div ref={parentRef} className="h-full overflow-auto font-mono text-[12px]">
      <table className="w-full border-collapse">
        <thead className="sticky top-0 z-10 bg-[var(--color-surface)]">
          {table.getHeaderGroups().map((hg) => (
            <tr key={hg.id}>
              {hg.headers.map((header) => (
                <th
                  key={header.id}
                  className="border-b border-r border-[var(--color-divider)] px-2 py-1 text-left font-semibold"
                >
                  {flexRender(header.column.columnDef.header, header.getContext())}
                </th>
              ))}
            </tr>
          ))}
        </thead>
        <tbody style={{ height: virtualizer.getTotalSize(), position: "relative", display: "block" }}>
          {virtualizer.getVirtualItems().map((virtualRow) => {
            const row = rows[virtualRow.index];
            return (
              <tr
                key={row.id}
                style={{
                  position: "absolute",
                  top: 0,
                  transform: `translateY(${virtualRow.start}px)`,
                  display: "flex",
                  width: "100%",
                }}
              >
                {row.getVisibleCells().map((cell) => (
                  <td
                    key={cell.id}
                    className="flex-1 truncate border-b border-r border-[var(--color-divider)] px-2 py-1"
                  >
                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                  </td>
                ))}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
