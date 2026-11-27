import { useEffect, useMemo, useRef, useState } from "react";
import uPlot from "uplot";
import "uplot/dist/uPlot.min.css";
import type { QueryResponse } from "@rotameter/plugin-sdk";
import { Button } from "@rotameter/ui";
import { buildUplotData } from "./format";

export interface TimeSeriesChartProps {
  points: QueryResponse["timeseries"];
}

interface SeriesConfig {
  idx: number;
  label: string;
  cssVar: string;
  dash: number[];
  patternLabel: string;
}

const SERIES_CONFIGS: ReadonlyArray<SeriesConfig> = [
  { idx: 1, label: "Visitors", cssVar: "--series-1", dash: [], patternLabel: "solid" },
  { idx: 2, label: "Pageviews", cssVar: "--series-2", dash: [6, 3], patternLabel: "dashed" },
  { idx: 3, label: "Visits", cssVar: "--series-3", dash: [2, 2], patternLabel: "dotted" },
];

function readCssToken(name: string): string {
  if (typeof document === "undefined") return "";
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

export function TimeSeriesChart({ points }: TimeSeriesChartProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const plotRef = useRef<uPlot | null>(null);
  const [visible, setVisible] = useState<[boolean, boolean, boolean]>([true, true, true]);
  const [themeRevision, setThemeRevision] = useState(0);

  const aligned = useMemo(() => buildUplotData(points), [points]);

  useEffect(() => {
    if (typeof MutationObserver === "undefined" || typeof document === "undefined") return;
    const observer = new MutationObserver(() => {
      setThemeRevision((n) => n + 1);
    });
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const el = containerRef.current;
    if (!el || points.length === 0) {
      plotRef.current?.destroy();
      plotRef.current = null;
      return;
    }

    const fgMuted = readCssToken("--fg-muted");
    const gridStroke = readCssToken("--border-hairline");
    const s1 = readCssToken("--series-1");
    const s2 = readCssToken("--series-2");
    const s3 = readCssToken("--series-3");
    const seriesColors = [s1, s2, s3];

    const opts: uPlot.Options = {
      width: Math.max(el.clientWidth, 320),
      height: 240,
      scales: {
        x: { time: true },
      },
      legend: { show: false },
      cursor: {
        drag: { x: false, y: false },
      },
      axes: [
        {
          stroke: fgMuted,
          grid: { stroke: gridStroke, width: 1 },
          ticks: { stroke: gridStroke, width: 1 },
        },
        {
          stroke: fgMuted,
          grid: { stroke: gridStroke, width: 1 },
          ticks: { stroke: gridStroke, width: 1 },
        },
      ],
      series: [
        { label: "Bucket" },
        ...SERIES_CONFIGS.map((cfg, idx) => ({
          label: cfg.label,
          stroke: seriesColors[idx] ?? s1,
          width: 2,
          dash: cfg.dash,
          show: visible[idx] ?? true,
        })),
      ],
    };

    const plot = new uPlot(opts, aligned, el);
    plotRef.current = plot;

    const ro =
      typeof ResizeObserver !== "undefined"
        ? new ResizeObserver((entries) => {
            const w = entries[0]?.contentRect.width ?? el.clientWidth;
            if (w > 0) {
              plot.setSize({ width: Math.floor(w), height: 240 });
            }
          })
        : null;
    ro?.observe(el);

    return () => {
      ro?.disconnect();
      plot.destroy();
      plotRef.current = null;
    };
  }, [aligned, points.length, themeRevision, visible]);

  if (points.length === 0) {
    return (
      <div className="border border-[var(--border-hairline)] bg-[var(--bg-sunken)] px-3 py-6 text-center text-[var(--fg-muted)] text-[var(--text-sm)]">
        No timeseries buckets in this window.
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap items-center gap-2" role="group" aria-label="Series visibility">
        {SERIES_CONFIGS.map((cfg, idx) => {
          const active = visible[idx] ?? true;
          return (
            <Button
              key={cfg.label}
              variant={active ? "secondary" : "ghost"}
              aria-pressed={active}
              onClick={() => {
                setVisible((prev) => {
                  const next: [boolean, boolean, boolean] = [...prev];
                  next[idx] = !next[idx];
                  return next;
                });
              }}
            >
              <svg width="18" height="8" aria-hidden className="shrink-0">
                <line
                  x1="0"
                  y1="4"
                  x2="18"
                  y2="4"
                  stroke={`var(${cfg.cssVar})`}
                  strokeWidth="2"
                  strokeDasharray={cfg.dash.join(" ") || undefined}
                />
              </svg>
              <span>{cfg.label}</span>
              <span className="text-[var(--text-xs)] text-[var(--fg-muted)]">({cfg.patternLabel})</span>
            </Button>
          );
        })}
      </div>
      <div
        ref={containerRef}
        className="w-full overflow-hidden border border-[var(--border-hairline)] bg-[var(--bg-surface)] p-2"
      />
    </div>
  );
}
