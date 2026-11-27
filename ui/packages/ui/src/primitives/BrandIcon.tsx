import { cx } from "../utils";

export const BRAND_ICON_NAMES = [
  "rm-logo",
  "rm-stream",
  "rm-threshold",
  "rm-range",
  "rm-throughput",
  "rm-segments",
  "rm-equilibrium",
] as const;

export type BrandIconName = (typeof BRAND_ICON_NAMES)[number];

export interface BrandIconProps {
  name: BrandIconName;
  label?: string;
  className?: string;
}

export function BrandIcon({ name, label, className }: BrandIconProps) {
  return (
    <svg
      aria-hidden={label ? undefined : true}
      aria-label={label}
      role={label ? "img" : undefined}
      className={cx(
        "w-[var(--density-icon-size)] h-[var(--density-icon-size)] shrink-0",
        className,
      )}
    >
      <use href={`/icons-sprite.svg#${name}`} />
    </svg>
  );
}
