import { Tabs as BaseTabs } from "@base-ui/react/tabs";
import type { ReactNode } from "react";
import { cx } from "../utils";

export interface TabItem {
  value: string;
  label: string;
  content: ReactNode;
}

export interface TabsProps {
  value: string;
  onValueChange: (value: string) => void;
  items: ReadonlyArray<TabItem>;
  className?: string;
}

export function Tabs({ value, onValueChange, items, className }: TabsProps) {
  return (
    <BaseTabs.Root value={value} onValueChange={onValueChange} className={cx("flex flex-col", className)}>
      <BaseTabs.List className="flex flex-wrap gap-1 border-b border-[var(--border-hairline)] bg-[var(--bg-elevated)] px-2 py-1">
        {items.map((item) => (
          <BaseTabs.Tab
            key={item.value}
            value={item.value}
            className={cx(
              "px-2.5 min-h-[24px] py-1 text-[length:var(--text-sm)] font-medium rounded-[var(--radius-xs)] cursor-pointer",
              "text-[var(--fg-secondary)] hover:text-[var(--fg-primary)]",
              "data-[active]:bg-[var(--bg-surface)] data-[active]:text-[var(--fg-primary)] data-[active]:border data-[active]:border-[var(--border-strong)]",
            )}
          >
            {item.label}
          </BaseTabs.Tab>
        ))}
      </BaseTabs.List>
      {items.map((item) => (
        <BaseTabs.Panel key={item.value} value={item.value} className="pt-2">
          {item.content}
        </BaseTabs.Panel>
      ))}
    </BaseTabs.Root>
  );
}
