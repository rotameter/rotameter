import { Select as BaseSelect } from "@base-ui/react/select";
import { cx } from "../utils";

export interface SelectOption<T extends string = string> {
  value: T;
  label: string;
}

export interface SelectProps<T extends string = string> {
  value: T;
  onValueChange: (value: T) => void;
  options: ReadonlyArray<SelectOption<T>>;
  ariaLabel: string;
  className?: string;
}

export function Select<T extends string>({
  value,
  onValueChange,
  options,
  ariaLabel,
  className,
}: SelectProps<T>) {
  const current = options.find((opt) => opt.value === value)?.label ?? value;

  return (
    <BaseSelect.Root
      value={value}
      onValueChange={(next) => {
        if (typeof next === "string") {
          onValueChange(next as T);
        }
      }}
    >
      <BaseSelect.Trigger
        aria-label={ariaLabel}
        className={cx(
          "inline-flex items-center justify-between gap-2 border border-[var(--border-strong)]",
          "bg-[var(--bg-surface)] text-[var(--fg-primary)] px-2.5 min-h-[max(24px,var(--density-control-h))]",
          "rounded-[var(--radius-xs)] text-[length:var(--text-sm)] cursor-pointer hover:bg-[var(--bg-elevated)]",
          className,
        )}
      >
        <BaseSelect.Value>{current}</BaseSelect.Value>
        <BaseSelect.Icon aria-hidden className="text-[var(--fg-muted)] text-[length:var(--text-xs)]">
          ▾
        </BaseSelect.Icon>
      </BaseSelect.Trigger>
      <BaseSelect.Portal>
        <BaseSelect.Positioner sideOffset={4} className="z-50">
          <BaseSelect.Popup className="min-w-[140px] border border-[var(--border-strong)] bg-[var(--bg-surface)] rounded-[var(--radius-sm)] p-1">
            {options.map((opt) => (
              <BaseSelect.Item
                key={opt.value}
                value={opt.value}
                className="flex items-center justify-between px-2.5 min-h-[24px] py-1 text-[length:var(--text-sm)] text-[var(--fg-primary)] rounded-[var(--radius-xs)] cursor-pointer data-[highlighted]:bg-[var(--bg-elevated)] data-[selected]:font-semibold"
              >
                <BaseSelect.ItemText>{opt.label}</BaseSelect.ItemText>
                <BaseSelect.ItemIndicator className="text-[var(--accent-primary)]">✓</BaseSelect.ItemIndicator>
              </BaseSelect.Item>
            ))}
          </BaseSelect.Popup>
        </BaseSelect.Positioner>
      </BaseSelect.Portal>
    </BaseSelect.Root>
  );
}
