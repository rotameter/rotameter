import { Dialog as BaseDialog } from "@base-ui/react/dialog";
import type { ReactNode } from "react";
import { Button } from "./Button";

export interface DialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: string;
  children: ReactNode;
}

export function Dialog({ open, onOpenChange, title, description, children }: DialogProps) {
  return (
    <BaseDialog.Root open={open} onOpenChange={onOpenChange}>
      <BaseDialog.Portal>
        <BaseDialog.Backdrop className="fixed inset-0 z-40 bg-[var(--bg-sunken)]/80" />
        <BaseDialog.Popup className="fixed left-1/2 top-1/2 z-50 w-[min(92vw,480px)] -translate-x-1/2 -translate-y-1/2 border border-[var(--border-strong)] bg-[var(--bg-surface)] p-4 rounded-[var(--radius-sm)]">
          <div className="flex items-start justify-between gap-2 border-b border-[var(--border-hairline)] pb-2 mb-3">
            <div>
              <BaseDialog.Title className="text-[length:var(--text-md)] font-semibold text-[var(--fg-primary)]">
                {title}
              </BaseDialog.Title>
              {description ? (
                <BaseDialog.Description className="text-[length:var(--text-sm)] text-[var(--fg-secondary)] mt-0.5">
                  {description}
                </BaseDialog.Description>
              ) : null}
            </div>
            <BaseDialog.Close render={<Button variant="ghost" aria-label="Close dialog">✕</Button>} />
          </div>
          {children}
        </BaseDialog.Popup>
      </BaseDialog.Portal>
    </BaseDialog.Root>
  );
}
