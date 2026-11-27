import { useEffect, useMemo, useRef, useState } from "react";
import {
  createColumnHelper,
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  useReactTable,
  type SortingState,
} from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
import { computeBarRatio, formatDurationSeconds } from "./dimensions";
import { useDensityRowHeight } from "./useDensityRowHeight";

export interface BreakdownRowItem {
  value: string;
  visitors: number;
  pageviews: number;
  visits: number;
  bounce_rate: number;
  avg_visit_duration_ms: number;
}

export interface BreakdownTableProps {
  dimensionLabel: string;
  rows: BreakdownRowItem[];
  activeFilterValue: string | null;
  onSelectValue: (value: string) => void;
}

const columnHelper = createColumnHelper<BreakdownRowItem>();

export function BreakdownTable({
  dimensionLabel,
  rows,
  activeFilterValue,
  onSelectValue,
}: BreakdownTableProps) {
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const rowHeight = useDensityRowHeight();
  const [sorting, setSorting] = useState<SortingState>([{ id: "visitors", desc: true }]);

  const maxVisitors = useMemo(
    () => rows.reduce((acc, r) => Math.max(acc, r.visitors), 0),
    [rows],
  );

  const columns = useMemo(
    () => [
      columnHelper.accessor("value", {
        header: dimensionLabel,
        cell: (info) => info.getValue(),
      }),
      columnHelper.accessor("visitors", {
        header: "Visitors",
        cell: (info) => info.getValue().toLocaleString("en-US"),
      }),
      columnHelper.accessor("pageviews", {
        header: "Pageviews",
        cell: (info) => info.getValue().toLocaleString("en-US"),
      }),
      columnHelper.accessor("visits", {
        header: "Visits",
        cell: (info) => info.getValue().toLocaleString("en-US"),
      }),
      columnHelper.accessor("bounce_rate", {
        header: "Bounce",
        cell: (info) => `${(info.getValue() * 100).toFixed(1)}%`,
      }),
      columnHelper.accessor("avg_visit_duration_ms", {
        header: "Avg Duration",
        cell: (info) => formatDurationSeconds(info.getValue()),
      }),
    ],
    [dimensionLabel],
  );

  const table = useReactTable({
    data: rows,
    columns,
    state: { sorting },
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  const sortedRows = table.getRowModel().rows;
  const virtualizer = useVirtualizer({
    count: sortedRows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => rowHeight,
    overscan: 8,
  });

  useEffect(() => {
    virtualizer.measure();
  }, [rowHeight, virtualizer]);

  if (rows.length === 0) {
    return (
      <div className="border border-[var(--border-hairline)] bg-[var(--bg-sunken)] px-3 py-6 text-center text-[var(--fg-muted)] text-[length:var(--text-sm)]">
        No breakdown rows in this range.
      </div>
    );
  }

  return (
    <div
      ref={scrollRef}
      className="max-h-[420px] overflow-auto border border-[var(--border-hairline)] bg-[var(--bg-surface)]"
    >
      <table className="w-full border-collapse grid text-[length:var(--text-sm)]">
        <thead className="sticky top-0 z-10 grid bg-[var(--bg-elevated)] border-b border-[var(--border-hairline)]">
          {table.getHeaderGroups().map((hg) => (
            <tr
              key={hg.id}
              className="grid grid-cols-[minmax(180px,1fr)_88px_92px_76px_76px_96px] items-center"
            >
              {hg.headers.map((header, colIdx) => {
                const sorted = header.column.getIsSorted();
                const isRight = colIdx > 0;
                return (
                  <th
                    key={header.id}
                    className={`px-2.5 py-1.5 font-medium text-[length:var(--text-xs)] uppercase tracking-[0.05em] text-[var(--fg-secondary)] select-none ${
                      isRight ? "text-right" : "text-left"
                    }`}
                  >
                    <button
                      type="button"
                      onClick={header.column.getToggleSortingHandler()}
                      className={`inline-flex items-center gap-1 w-full cursor-pointer hover:text-[var(--fg-primary)] ${
                        isRight ? "justify-end" : "justify-start"
                      }`}
                    >
                      <span>{flexRender(header.column.columnDef.header, header.getContext())}</span>
                      {sorted === "asc" ? "↑" : sorted === "desc" ? "↓" : ""}
                    </button>
                  </th>
                );
              })}
            </tr>
          ))}
        </thead>
        <tbody
          className="relative grid"
          style={{ height: `${virtualizer.getTotalSize()}px` }}
        >
          {virtualizer.getVirtualItems().map((vRow) => {
            const row = sortedRows[vRow.index];
            if (!row) return null;
            const item = row.original;
            const ratio = computeBarRatio(item.visitors, maxVisitors);
            const isSelected = activeFilterValue === item.value;
            return (
              <tr
                key={row.id}
                tabIndex={0}
                role="button"
                aria-pressed={isSelected}
                onClick={() => onSelectValue(item.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    onSelectValue(item.value);
                  }
                }}
                style={{ transform: `translateY(${vRow.start}px)` }}
                className={`absolute top-0 left-0 w-full h-[var(--density-table-row-h)] grid grid-cols-[minmax(180px,1fr)_88px_92px_76px_76px_96px] items-center border-b border-[var(--border-hairline)] cursor-pointer transition-colors ${
                  isSelected
                    ? "bg-[var(--bg-elevated)] font-semibold"
                    : "hover:bg-[var(--bg-elevated)]"
                }`}
              >
                {row.getVisibleCells().map((cell, colIdx) => {
                  if (colIdx === 0) {
                    return (
                      <td key={cell.id} className="relative px-2.5 h-full flex items-center overflow-hidden">
                        <span
                          aria-hidden
                          className="absolute inset-y-0.5 left-0 bg-[var(--series-1)]/15 pointer-events-none"
                          style={{ width: `${Math.round(ratio * 100)}%` }}
                        />
                        <span className="relative truncate text-[var(--fg-primary)]">
                          {flexRender(cell.column.columnDef.cell, cell.getContext())}
                        </span>
                      </td>
                    );
                  }
                  return (
                    <td
                      key={cell.id}
                      className="px-2.5 text-right rm-tabular text-[var(--fg-secondary)]"
                    >
                      {flexRender(cell.column.columnDef.cell, cell.getContext())}
                    </td>
                  );
                })}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
