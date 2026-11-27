import { useState, type FormEvent } from "react";
import { ApiClientError, type AuthMeResponse, type CapabilitiesResponse, type RotameterApiClient } from "@rotameter/plugin-sdk";
import { Button, Input, InstrumentFrame } from "@rotameter/ui";

export interface AuthGateProps {
  api: RotameterApiClient;
  capabilities: CapabilitiesResponse;
  onAuthenticated: (auth: AuthMeResponse) => void;
}

export function AuthGate({ api, capabilities, onAuthenticated }: AuthGateProps) {
  const [username, setUsername] = useState("admin");
  const [password, setPassword] = useState("");
  const [token, setToken] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const supportsSession = capabilities.auth_methods.includes("session");
  const supportsOidc = capabilities.features.oidc;

  async function handlePasswordLogin(e: FormEvent) {
    e.preventDefault();
    setError(null);
    setBusy(true);
    try {
      const res = await api.login({ username, password });
      onAuthenticated({
        authenticated: true,
        method: "session",
        subject: res.subject,
        scope: res.scope,
        org_id: res.org_id,
        site_id: res.site_id,
      });
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : "Authentication failed");
    } finally {
      setBusy(false);
    }
  }

  async function handleTokenLogin(e: FormEvent) {
    e.preventDefault();
    setError(null);
    setBusy(true);
    try {
      const res = await api.login({ token });
      onAuthenticated({
        authenticated: true,
        method: "session",
        subject: res.subject,
        scope: res.scope,
        org_id: res.org_id,
        site_id: res.site_id,
      });
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : "Invalid API token");
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="min-h-screen flex items-center justify-center p-4 bg-[var(--bg-canvas)]">
      <div className="w-full max-w-md flex flex-col gap-4">
        <InstrumentFrame
          title="ROTAMETER // OPERATOR AUTH"
          subtitle={`${capabilities.topology.toUpperCase()} · ${capabilities.node_role}`}
          reticles
        >
          <div className="flex flex-col gap-4">
            {error ? (
              <div
                role="alert"
                className="border border-[var(--signal-crit)] bg-[var(--bg-elevated)] px-3 py-2 text-[var(--text-sm)] text-[var(--signal-crit)]"
              >
                {error}
              </div>
            ) : null}

            {supportsSession ? (
              <form onSubmit={handlePasswordLogin} className="flex flex-col gap-2.5">
                <div className="text-[var(--text-xs)] uppercase tracking-[0.06em] text-[var(--fg-muted)]">
                  Operator Credentials
                </div>
                <label className="flex flex-col gap-1 text-[var(--text-sm)]">
                  <span>Username</span>
                  <Input
                    value={username}
                    onChange={(e) => setUsername(e.target.value)}
                    autoComplete="username"
                    required
                  />
                </label>
                <label className="flex flex-col gap-1 text-[var(--text-sm)]">
                  <span>Password</span>
                  <Input
                    type="password"
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    autoComplete="current-password"
                    required
                  />
                </label>
                <Button type="submit" variant="primary" disabled={busy}>
                  {busy ? "Signing in…" : "Sign in"}
                </Button>
              </form>
            ) : null}

            <form
              onSubmit={handleTokenLogin}
              className="flex flex-col gap-2.5 border-t border-[var(--border-hairline)] pt-3"
            >
              <div className="text-[var(--text-xs)] uppercase tracking-[0.06em] text-[var(--fg-muted)]">
                API Key Session (`rm_adm_` or `rm_rd_`)
              </div>
              <Input
                type="password"
                placeholder="rm_adm_... or rm_rd_..."
                aria-label="API token"
                value={token}
                onChange={(e) => setToken(e.target.value)}
                required
              />
              <Button type="submit" variant="secondary" disabled={busy}>
                Authenticate via HttpOnly Cookie
              </Button>
            </form>

            {supportsOidc ? (
              <div className="border-t border-[var(--border-hairline)] pt-3">
                <a
                  href={`/v1/auth/oidc/start?return_to=${encodeURIComponent(window.location.pathname || "/")}`}
                  className="inline-flex w-full items-center justify-center border border-[var(--border-strong)] bg-[var(--bg-elevated)] px-3 min-h-[max(24px,var(--density-control-h))] rounded-[var(--radius-xs)] text-[var(--text-sm)] font-medium text-[var(--fg-primary)] hover:bg-[var(--bg-sunken)]"
                >
                  Continue with OIDC SSO
                </a>
              </div>
            ) : null}
          </div>
        </InstrumentFrame>
      </div>
    </main>
  );
}
