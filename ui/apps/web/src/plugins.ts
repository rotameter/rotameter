import type { LazyPluginLoader } from "@rotameter/plugin-sdk";

export const CORE_PLUGIN_LOADERS: ReadonlyArray<LazyPluginLoader> = [
  () => import("./slots/overview"),
  () => import("./slots/breakdowns"),
  () => import("./slots/realtime"),
  () => import("./slots/admin"),
  () => import("./slots/cluster"),
];
