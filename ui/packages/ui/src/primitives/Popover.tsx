import { Popover as BasePopover } from "@base-ui/react/popover";
import type { ReactElement, ReactNode } from "react";

export interface PopoverProps {
  trigger: ReactElement;
  children: ReactNode;
}

export function Popover({ trigger, children }: PopoverProps) {
  return (
    <BasePopover.Root>
      <BasePopover.Trigger render={trigger} />
      <BasePopover.Portal>
        <BasePopover.Positioner sideOffset={6} className="z-50">
          <BasePopover.Popup className="border border-[var(--border-strong)] bg-[var(--bg-surface)] p-3 rounded-[var(--radius-sm)] text-[length:var(--text-sm)] text-[var(--fg-primary)]">
            {children}
          </BasePopover.Popup>
        </BasePopover.Positioner>
      </BasePopover.Portal>
    </BasePopover.Root>
  );
}
