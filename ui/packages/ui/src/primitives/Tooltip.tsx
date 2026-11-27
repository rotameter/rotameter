import { Tooltip as BaseTooltip } from "@base-ui/react/tooltip";
import type { ReactElement, ReactNode } from "react";

export interface TooltipProps {
  trigger: ReactElement;
  content: ReactNode;
}

export function Tooltip({ trigger, content }: TooltipProps) {
  return (
    <BaseTooltip.Provider delay={250}>
      <BaseTooltip.Root>
        <BaseTooltip.Trigger render={trigger} />
        <BaseTooltip.Portal>
          <BaseTooltip.Positioner sideOffset={6} className="z-50">
            <BaseTooltip.Popup className="border border-[var(--border-strong)] bg-[var(--bg-elevated)] px-2 py-1 rounded-[var(--radius-xs)] text-[length:var(--text-xs)] text-[var(--fg-primary)]">
              {content}
            </BaseTooltip.Popup>
          </BaseTooltip.Positioner>
        </BaseTooltip.Portal>
      </BaseTooltip.Root>
    </BaseTooltip.Provider>
  );
}
