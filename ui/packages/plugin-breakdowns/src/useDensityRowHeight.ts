import { useEffect, useState } from "react";
import { parsePxValue } from "./dimensions";

function readDensityRowHeightPx(): number {
  if (typeof document === "undefined") return 34;
  const raw = getComputedStyle(document.documentElement).getPropertyValue("--density-table-row-h");
  return parsePxValue(raw, 34);
}

/**
 * Reads `--density-table-row-h` from `:root` via `getComputedStyle` and tracks changes to
 * `data-density` on `<html>` so virtualized rows keep `O(1)` scroll math across densities.
 */
export function useDensityRowHeight(): number {
  const [rowHeight, setRowHeight] = useState<number>(() => readDensityRowHeightPx());

  useEffect(() => {
    setRowHeight(readDensityRowHeightPx());
    if (typeof MutationObserver === "undefined" || typeof document === "undefined") return;
    const observer = new MutationObserver(() => {
      setRowHeight(readDensityRowHeightPx());
    });
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-density"],
    });
    return () => observer.disconnect();
  }, []);

  return rowHeight;
}
