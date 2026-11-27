import type {
  AuthMeResponse,
  CapabilitiesResponse,
  HealthResponse,
  QueryRequest,
  QueryResponse,
  RealtimeResponse,
  SiteResponse,
} from "./types";

export class ApiClientError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = "ApiClientError";
    this.status = status;
    this.code = code;
  }
}

export interface ApiClientConfig {
  query_base_url?: string;
  stream_base_url?: string;
  fetchImpl?: typeof fetch;
  getCsrfToken?: () => string | null;
}

export interface LoginPayload {
  username?: string;
  password?: string;
  token?: string;
}

export interface LoginResponse {
  authenticated: boolean;
  method: string;
  subject: string;
  scope: "read" | "admin" | "ingest";
  org_id: string;
  site_id?: string;
  csrf_token: string;
}

export function readCookie(name: string): string | null {
  if (typeof document === "undefined") return null;
  for (const part of document.cookie.split(";")) {
    const trimmed = part.trim();
    const eqIdx = trimmed.indexOf("=");
    if (eqIdx > 0 && trimmed.slice(0, eqIdx).trim() === name) {
      return trimmed.slice(eqIdx + 1).trim();
    }
  }
  return null;
}

export class RotameterApiClient {
  readonly query_base_url: string;
  readonly stream_base_url: string;
  private readonly fetchImpl: typeof fetch;
  private readonly getCsrfToken: () => string | null;

  constructor(config: ApiClientConfig = {}) {
    this.query_base_url = (config.query_base_url ?? "").replace(/\/$/, "");
    this.stream_base_url = (config.stream_base_url ?? this.query_base_url).replace(/\/$/, "");
    this.fetchImpl = config.fetchImpl ?? globalThis.fetch.bind(globalThis);
    this.getCsrfToken = config.getCsrfToken ?? (() => readCookie("rotameter_csrf"));
  }

  private async fetchChecked(
    path: string,
    init: RequestInit = {},
    baseUrl = this.query_base_url,
  ): Promise<Response> {
    const method = (init.method ?? "GET").toUpperCase();
    const headers = new Headers(init.headers);
    if (init.body && !headers.has("Content-Type")) {
      headers.set("Content-Type", "application/json");
    }
    if (method === "POST" || method === "PUT" || method === "DELETE") {
      const csrf = this.getCsrfToken();
      if (csrf && !headers.has("X-CSRF-Token")) {
        headers.set("X-CSRF-Token", csrf);
      }
    }
    const resp = await this.fetchImpl(`${baseUrl}${path}`, {
      ...init,
      method,
      headers,
      credentials: "same-origin",
    });
    if (!resp.ok) {
      let code = "http_error";
      let msg = `Request failed (${resp.status})`;
      try {
        const parsed = (await resp.json()) as { error?: { code?: string; message?: string } };
        if (parsed.error?.code) code = parsed.error.code;
        if (parsed.error?.message) msg = parsed.error.message;
      } catch {
        // non-JSON error body
      }
      throw new ApiClientError(resp.status, code, msg);
    }
    return resp;
  }

  private async requestJson<T>(
    path: string,
    init: RequestInit = {},
    baseUrl = this.query_base_url,
  ): Promise<T> {
    const resp = await this.fetchChecked(path, init, baseUrl);
    return (await resp.json()) as T;
  }

  capabilities(signal?: AbortSignal): Promise<CapabilitiesResponse> {
    return this.requestJson("/v1/capabilities", { signal });
  }

  health(signal?: AbortSignal): Promise<HealthResponse> {
    return this.requestJson("/v1/health", { signal });
  }

  authMe(signal?: AbortSignal): Promise<AuthMeResponse> {
    return this.requestJson("/v1/auth/me", { signal });
  }

  login(payload: LoginPayload, signal?: AbortSignal): Promise<LoginResponse> {
    return this.requestJson("/v1/auth/login", {
      method: "POST",
      body: JSON.stringify(payload),
      signal,
    });
  }

  logout(signal?: AbortSignal): Promise<{ authenticated: boolean }> {
    return this.requestJson("/v1/auth/logout", { method: "POST", signal });
  }

  listSites(signal?: AbortSignal): Promise<SiteResponse[]> {
    return this.requestJson("/v1/admin/sites", { signal });
  }

  query(req: QueryRequest, signal?: AbortSignal): Promise<QueryResponse> {
    return this.requestJson("/v1/query", {
      method: "POST",
      body: JSON.stringify(req),
      signal,
    });
  }

  async queryCsv(req: QueryRequest, signal?: AbortSignal): Promise<string> {
    const resp = await this.fetchChecked("/v1/query", {
      method: "POST",
      body: JSON.stringify({ ...req, format: "csv" }),
      signal,
    });
    return resp.text();
  }

  async exportPortableArchive(signal?: AbortSignal): Promise<Blob> {
    const resp = await this.fetchChecked("/v1/admin/backup", {
      method: "POST",
      signal,
    });
    return resp.blob();
  }

  realtime(site: string, signal?: AbortSignal): Promise<RealtimeResponse> {
    return this.requestJson(`/v1/sites/${encodeURIComponent(site)}/realtime`, { signal });
  }
}
