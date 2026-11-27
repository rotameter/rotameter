import { useState } from "react";
import type { RealtimeResponse } from "@rotameter/plugin-sdk";
import { normalizePulseBars, type PulseBar } from "./pulse";

export interface PulseChartProps {
  pulse: RealtimeResponse["pulse"];
}

export function PulseChart({ pulse }: PulseChartProps) {
  const { bars, maxEvents, totalEvents } = normalizePulseBars(pulse);
  const [hovered, setHovered] = useState<PulseBar | null>(null);

  if (bars.length === 0) {
    return (
      <div className="border border-[var(--border-hairline)] bg-[var(--bg-sunken)] px-3 py-6 text-center text-[var(--fg-muted)] text-[length:var(--text-sm)]">
        No events in trailing 30-minute pulse window.
      </div>
    );
  }

  const width = Math.max(bars.length * 16, 300);
  const height = 110;
  const barW = width / bars.length;

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between text-[length:var(--text-xs)] text-[var(--fg-muted)]">
        <span className="rm-tabular">
          {totalEvents.toLocaleString("en-US")} events (30m) · peak {maxEvents}/min
        </span>
        <span className="rm-tabular">
          {hovered ? `${hovered.label} UTC · ${hovered.events} events` : "Hover bar for minute"}
        </span>
      </div>
      <div className="border border-[var(--border-hairline)] bg-[var(--bg-surface)] p-2">
        <svg
          viewBox={`0 0 ${width} ${height}`}
          className="w-full h-28 overflow-visible"
          role="img"
          aria-label="30-minute minute-by-minute event pulse"
        >
          <line
            x1="0"
            y1={height - 1}
            x2={width}
            y2={height - 1}
            stroke="var(--border-hairline)"
            strokeWidth="1"
          />
          {bars.map((bar, idx) => {
            const h = Math.max(bar.events > 0 ? 3 : 1, Math.round(bar.ratio * (height - 8)));
            const x = idx * barW + 2;
            const y = height - h - 1;
            const w = Math.max(2, barW - 4);
            return (
              <rect
                key={bar.minute}
                x={x}
                y={y}
                width={w}
                height={h}
                fill="var(--series-1)"
                opacity={bar.events > 0 ? 0.9 : 0.25}
                className="cursor-crosshair"
                onMouseEnter={() => setHovered(bar)}
                onMouseLeave={() => setHovered(null)}
              >
                <title>{`${bar.label} UTC: ${bar.events} events`}</title>
              </rect>
            );
          })}
        </svg>
      </div>
    </div>
  );
}
