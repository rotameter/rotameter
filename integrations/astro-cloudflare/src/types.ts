export interface RotameterEnv {
  ROTAMETER_URL?: string;
  ROTAMETER_SITE?: string;
  ROTAMETER_INGEST_KEY?: string;
  ROTAMETER_ALLOWED_ORIGINS?: string;
  CF_ACCESS_CLIENT_ID?: string;
  CF_ACCESS_CLIENT_SECRET?: string;
  ROTAMETER_ACCESS_CLIENT_ID?: string;
  ROTAMETER_ACCESS_CLIENT_SECRET?: string;
}

export interface CloudflareRequestCf {
  country?: string;
  region?: string;
  regionCode?: string;
}

export interface ExecutionContextLike {
  waitUntil(promise: Promise<unknown>): void;
}

export type PropValue = string | number | boolean;

export interface NormalizedBeaconPayload {
  site_id: string;
  event?: string;
  url: string;
  referrer?: string;
  viewport_width?: number;
  props?: Record<string, PropValue>;
}

export interface CollectProxyOptions {
  fetcher?: typeof fetch;
  timeoutMs?: number;
  cacheControl?: string;
}

export interface EventProxyOptions {
  fetcher?: typeof fetch;
  timeoutMs?: number;
  maxBodyBytes?: number;
}

export interface ServerHitOptions {
  event?: string;
  url?: string;
  referrer?: string;
  props?: Record<string, PropValue>;
  fetcher?: typeof fetch;
  timeoutMs?: number;
}
