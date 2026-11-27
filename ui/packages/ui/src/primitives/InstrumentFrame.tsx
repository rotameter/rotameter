import type { ReactNode } from "react";
import { cx } from "../utils";

export interface InstrumentFrameProps {
  title: string;
  subtitle?: string;
  actions?: ReactNode;
  reticles?: boolean;
  children: ReactNode;
  className?: string;
}

export function InstrumentFrame({
  title,
  subtitle,
  actions,
  reticles = false,
  children,
  className,
}: InstrumentFrameProps) {
  return (
    <section
      className={cx(
        "border border-[var(--border-hairline)] bg-[var(--bg-surface)] rounded-[var(--radius-xs)]",
        reticles && "rm-reticles",
        className,
      )}
    >
      <header className="flex flex-wrap items-center justify-between gap-2 border-b border-[var(--border-hairline)] bg-[var(--bg-elevated)] p-[var(--density-cell-pad)]">
        <div className="flex items-baseline gap-2 min-w-0">
          <h2 className="text-[length:var(--text-sm)] font-semibold uppercase tracking-[0.06em] text-[var(--fg-primary)] truncate">
            {title}
          </h2>
          {subtitle ? (
            <span className="rm-tabular rm-density-detail text-[length:var(--text-xs)] text-[var(--fg-muted)] truncate">
              {subtitle}
            </span>
          ) : null}
        </div>
        {actions ? <div className="flex items-center gap-1.5">{actions}</div> : null}
      </header>
      <div className="p-[var(--density-panel-pad)]">{children}</div>
    </section>
  );
}
