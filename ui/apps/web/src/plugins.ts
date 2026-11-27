import type { LazyPluginLoader } from "@rotameter/plugin-sdk";

export const CORE_PLUGIN_LOADERS: ReadonlyArray<LazyPluginLoader> = [
  () => import("@rotameter/plugin-overview"),
  () => import("@rotameter/plugin-breakdowns"),
  () => import("@rotameter/plugin-realtime"),
  () => import("@rotameter/plugin-admin"),
  () => import("@rotameter/plugin-cluster"),
];
