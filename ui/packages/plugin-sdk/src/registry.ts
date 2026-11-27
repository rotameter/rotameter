import type {
  ApiScope,
  AuthMeResponse,
  CapabilitiesResponse,
  LazyPluginLoader,
  PluginManifest,
} from "./types";

const SCOPE_GRANTS: Record<AuthMeResponse["scope"], ReadonlySet<ApiScope>> = {
  ingest: new Set<ApiScope>(),
  read: new Set<ApiScope>(["read:aggregates"]),
  admin: new Set<ApiScope>([
    "read:aggregates",
    "read:events",
    "write:dashboards",
    "admin:sites",
    "admin:keys",
  ]),
};

export function isPluginEnabled(
  manifest: Pick<PluginManifest, "required_scopes" | "requires_query">,
  capabilities: CapabilitiesResponse,
  authScope: AuthMeResponse["scope"],
): boolean {
  if (manifest.requires_query && capabilities.node_role === "ingest") {
    return false;
  }
  const granted = SCOPE_GRANTS[authScope];
  return manifest.required_scopes.every((scope) => granted.has(scope));
}

export async function loadEnabledPlugins(
  loaders: ReadonlyArray<LazyPluginLoader>,
  capabilities: CapabilitiesResponse,
  authScope: AuthMeResponse["scope"],
): Promise<PluginManifest[]> {
  const loaded = await Promise.all(loaders.map((load) => load().then((m) => m.default)));
  return loaded.filter((manifest) => isPluginEnabled(manifest, capabilities, authScope));
}
