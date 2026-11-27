import { useState, type ReactNode } from "react";
import { Button } from "@rotameter/ui";
import type { ChartHostFallbackData } from "./types";

export type WebGLChartRenderer = (props: { vizType: string; data: unknown }) => ReactNode;

const webglRenderers = new Map<string, WebGLChartRenderer>();

/** Extension seam for future deferred WebGL renderers (0.5.0+ if density requires). */
export function registerWebGLRenderer(vizType: string, renderer: WebGLChartRenderer): void {
  webglRenderers.set(vizType, renderer);
}

export function hasWebGLRenderer(vizType: string): boolean {
  return webglRenderers.has(vizType);
}

export interface ChartHostProps {
  vizType: string;
  title: string;
  data?: unknown;
  fallback: ChartHostFallbackData;
  children: ReactNode;
}

export function ChartHost({ vizType, title, data, fallback, children }: ChartHostProps) {
  const [showTable, setShowTable] = useState(false);
  const webgl = webglRenderers.get(vizType);

  return (
    <div className="flex flex-col gap-2" data-viz-type={vizType}>
      <div className="flex items-center justify-between gap-2">
        <span className="text-[length:var(--text-xs)] uppercase tracking-[0.06em] text-[var(--fg-muted)]">
          {title}
        </span>
        <Button
          variant="ghost"
          aria-pressed={showTable}
          onClick={() => setShowTable((prev) => !prev)}
        >
          {showTable ? "View Chart" : "View Table"}
        </Button>
      </div>
      {showTable ? (
        <div className="overflow-x-auto border border-[var(--border-hairline)]">
          <table className="w-full border-collapse text-[length:var(--text-sm)]" aria-label={fallback.caption ?? title}>
            <thead>
              <tr className="bg-[var(--bg-elevated)] border-b border-[var(--border-hairline)]">
                {fallback.columns.map((col) => (
                  <th
                    key={col.key}
                    className={`px-2.5 py-1.5 font-medium text-[var(--fg-secondary)] ${
                      col.align === "right" ? "text-right" : "text-left"
                    }`}
                  >
                    {col.label}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {fallback.rows.length === 0 ? (
                <tr>
                  <td
                    colSpan={fallback.columns.length}
                    className="px-2.5 py-3 text-center text-[var(--fg-muted)]"
                  >
                    No tabular rows available.
                  </td>
                </tr>
              ) : (
                fallback.rows.map((row, idx) => (
                  <tr key={idx} className="border-b border-[var(--border-hairline)] last:border-b-0">
                    {fallback.columns.map((col) => (
                      <td
                        key={col.key}
                        className={`px-2.5 py-1.5 rm-tabular ${
                          col.align === "right" ? "text-right" : "text-left"
                        }`}
                      >
                        {String(row[col.key] ?? "")}
                      </td>
                    ))}
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      ) : webgl ? (
        <>{webgl({ vizType, data })}</>
      ) : (
        <>{children}</>
      )}
    </div>
  );
}
