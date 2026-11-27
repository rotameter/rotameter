import type { RealtimeResponse } from "@rotameter/plugin-sdk";

export interface PulseBar {
  minute: string;
  label: string;
  events: number;
  ratio: number;
}

export function formatMinuteLabel(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return `${String(d.getUTCHours()).padStart(2, "0")}:${String(d.getUTCMinutes()).padStart(2, "0")}`;
}

export function normalizePulseBars(pulse: RealtimeResponse["pulse"]): {
  bars: PulseBar[];
  maxEvents: number;
  totalEvents: number;
} {
  let maxEvents = 0;
  let totalEvents = 0;
  for (const pt of pulse) {
    if (pt.events > maxEvents) maxEvents = pt.events;
    totalEvents += pt.events;
  }
  const bars: PulseBar[] = pulse.map((pt) => ({
    minute: pt.minute,
    label: formatMinuteLabel(pt.minute),
    events: pt.events,
    ratio: maxEvents > 0 ? pt.events / maxEvents : 0,
  }));
  return { bars, maxEvents, totalEvents };
}
