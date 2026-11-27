import { Input as BaseInput } from "@base-ui/react/input";
import type { ComponentPropsWithoutRef } from "react";
import { cx } from "../utils";

export interface InputProps
  extends Omit<ComponentPropsWithoutRef<typeof BaseInput>, "className"> {
  className?: string;
}

export function Input({ className, ...props }: InputProps) {
  return (
    <BaseInput
      className={cx(
        "w-full border border-[var(--border-strong)] bg-[var(--bg-surface)] text-[var(--fg-primary)]",
        "placeholder:text-[var(--fg-muted)] px-2.5 min-h-[max(24px,var(--density-control-h))]",
        "rounded-[var(--radius-xs)] text-[var(--text-sm)]",
        className,
      )}
      {...props}
    />
  );
}
