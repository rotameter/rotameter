import { Button as BaseButton } from "@base-ui/react/button";
import type { ComponentPropsWithoutRef } from "react";
import { cx } from "../utils";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";

export interface ButtonProps
  extends Omit<ComponentPropsWithoutRef<typeof BaseButton>, "className"> {
  variant?: ButtonVariant;
  className?: string;
}

const VARIANT_CLASSES: Record<ButtonVariant, string> = {
  primary:
    "bg-[var(--accent-primary)] text-[var(--accent-fg)] border-[var(--accent-primary)] hover:opacity-90",
  secondary:
    "bg-[var(--bg-surface)] text-[var(--fg-primary)] border-[var(--border-strong)] hover:bg-[var(--bg-elevated)]",
  ghost:
    "bg-transparent text-[var(--fg-secondary)] border-transparent hover:bg-[var(--bg-elevated)] hover:text-[var(--fg-primary)]",
  danger:
    "bg-[var(--signal-crit)] text-[var(--accent-fg)] border-[var(--signal-crit)] hover:opacity-90",
};

export function Button({
  variant = "secondary",
  className,
  type = "button",
  ...props
}: ButtonProps) {
  return (
    <BaseButton
      type={type}
      className={cx(
        "inline-flex items-center justify-center gap-1.5 border px-2.5 font-medium transition-colors",
        "min-h-[max(24px,var(--density-control-h))] min-w-[24px] rounded-[var(--radius-xs)]",
        "text-[var(--text-sm)] leading-none cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed",
        VARIANT_CLASSES[variant],
        className,
      )}
      {...props}
    />
  );
}
