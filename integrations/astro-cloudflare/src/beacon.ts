import type { PropValue } from "./types.ts";

export type { PropValue } from "./types.ts";

export const DEFAULT_BEACON_ENDPOINT = "/api/e";
export const DEFAULT_IGNORE_KEY = "rotameter_ignore";
export const DEFAULT_DOWNLOAD_PATTERN =
  /\.(pdf|zip|gz|tar|dmg|exe|pkg|csv|xlsx)$/i;

export interface BeaconOptions {
  /** Optional site UID or slug; omit when the Worker proxy injects `ROTAMETER_SITE`. */
  site?: string;
  /** Alias for `site`. */
  siteId?: string;
  /** First-party proxy or ingest endpoint (default: `"/api/e"`). */
  endpoint?: string;
  /** Force `"not_found"` event instead of `"pageview"` on page hits. */
  notFound?: boolean;
  /** Automatically send a page hit when initialized (default: `true`). */
  autoPageview?: boolean;
  /** Track SPA route changes via `history.pushState`, `popstate`, and `astro:page-load`. */
  spa?: boolean;
  /** Track clicks on external hosts as `"outbound_click"`. */
  outbound?: boolean;
  /** Track clicks matching `downloadPattern` as `"file_download"`. */
  downloads?: boolean;
  /** Override `DEFAULT_DOWNLOAD_PATTERN` when `downloads` is enabled. */
  downloadPattern?: RegExp;
  /** LocalStorage key controlling opt-out (default: `"rotameter_ignore"`). */
  ignoreKey?: string;
  /** Suppress beacon traffic on `localhost` / `127.0.0.1` / `::1` (default: `true`). */
  ignoreLocalhost?: boolean;
  /** Attach `window.rotameter(event, { props })` helper (default: `true`). */
  exposeGlobal?: boolean;
}

export interface RotameterTrackOptions {
  props?: Record<string, PropValue>;
}

export interface RotameterBeaconHandle {
  track: (event: string, props?: Record<string, PropValue>) => void;
  isDisabled: () => boolean;
  destroy: () => void;
}

export interface SendBeaconEventOptions {
  site?: string;
  siteId?: string;
  endpoint?: string;
}

type RotameterGlobal = (event: string, options?: RotameterTrackOptions) => void;

type WindowWithRotameter = Window & {
  rotameter?: RotameterGlobal;
};

function getStorage(): Storage | undefined {
  try {
    if (typeof window !== "undefined" && window.localStorage) {
      return window.localStorage;
    }
    if (typeof localStorage !== "undefined") {
      return localStorage;
    }
  } catch {
    return undefined;
  }
  return undefined;
}

function getLocation(): Location | undefined {
  if (typeof window !== "undefined" && window.location) {
    return window.location;
  }
  if (typeof location !== "undefined") {
    return location;
  }
  return undefined;
}

function isLocalBrowserHost(hostname: string | undefined): boolean {
  if (!hostname) return true;
  const h = hostname.toLowerCase();
  return (
    h === "localhost" ||
    h === "127.0.0.1" ||
    h === "::1" ||
    h === "[::1]" ||
    h.endsWith(".localhost")
  );
}

interface AttrElementLike {
  hasAttribute?: (name: string) => boolean;
  getAttribute?: (name: string) => string | null;
}

function hasTruthyAttr(el: AttrElementLike | null | undefined, name: string): boolean {
  if (!el?.hasAttribute?.(name)) return false;
  const val = el.getAttribute?.(name);
  return val === null || val === undefined || val.trim().toLowerCase() !== "false";
}

function detectNotFound(): boolean {
  if (typeof document === "undefined") return false;
  if (
    hasTruthyAttr(document.documentElement, "data-rotameter-not-found") ||
    hasTruthyAttr(document.body, "data-rotameter-not-found")
  ) {
    return true;
  }
  const meta = document.querySelector?.('meta[name="rotameter-not-found"]');
  if (!meta) return false;
  const content = meta.getAttribute?.("content");
  return content === null || content === undefined || content.trim().toLowerCase() !== "false";
}

export function isRotameterIgnored(ignoreKey: string = DEFAULT_IGNORE_KEY): boolean {
  try {
    return getStorage()?.getItem(ignoreKey) === "true";
  } catch {
    return false;
  }
}

export function setRotameterIgnored(
  ignored: boolean,
  ignoreKey: string = DEFAULT_IGNORE_KEY,
): boolean {
  try {
    const storage = getStorage();
    if (!storage) return false;
    if (ignored) {
      storage.setItem(ignoreKey, "true");
    } else {
      storage.removeItem(ignoreKey);
    }
    return true;
  } catch {
    return false;
  }
}

export function sendBeaconEvent(
  event: string,
  props?: Record<string, PropValue>,
  options: SendBeaconEventOptions = {},
): boolean {
  const loc = getLocation();
  if (typeof window === "undefined" || !loc) {
    return false;
  }

  const endpoint = options.endpoint || DEFAULT_BEACON_ENDPOINT;
  const siteId = options.site ?? options.siteId;
  const payload: Record<string, unknown> = {
    event,
    url: loc.href,
  };
  if (siteId) payload.site_id = siteId;
  if (typeof document !== "undefined" && document.referrer) {
    payload.referrer = document.referrer;
  }
  if (typeof window.innerWidth === "number") {
    payload.viewport_width = window.innerWidth;
  }
  if (props && Object.keys(props).length > 0) {
    payload.props = props;
  }

  const body = JSON.stringify(payload);

  if (typeof navigator !== "undefined" && typeof navigator.sendBeacon === "function") {
    try {
      const blob = new Blob([body], { type: "application/json" });
      if (navigator.sendBeacon(endpoint, blob)) return true;
    } catch {
      // Fall back to keepalive fetch if Blob/sendBeacon rejects.
    }
  }

  if (typeof fetch === "function") {
    fetch(endpoint, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body,
      keepalive: true,
    }).catch(() => {});
    return true;
  }

  return false;
}

export function initRotameterBeacon(
  options: BeaconOptions = {},
): RotameterBeaconHandle {
  const noop: RotameterBeaconHandle = {
    track: () => {},
    isDisabled: () => true,
    destroy: () => {},
  };

  const loc = getLocation();
  if (typeof window === "undefined" || typeof document === "undefined" || !loc) {
    return noop;
  }

  const ignoreKey = options.ignoreKey ?? DEFAULT_IGNORE_KEY;
  const ignoreLocalhost = options.ignoreLocalhost ?? true;
  const endpoint = options.endpoint ?? DEFAULT_BEACON_ENDPOINT;
  const autoPageview = options.autoPageview ?? true;
  const exposeGlobal = options.exposeGlobal ?? true;
  const downloadPattern = options.downloadPattern ?? DEFAULT_DOWNLOAD_PATTERN;

  let destroyed = false;

  const isDisabled = (): boolean =>
    destroyed ||
    isRotameterIgnored(ignoreKey) ||
    (ignoreLocalhost && isLocalBrowserHost(loc.hostname));

  if (isDisabled()) {
    return noop;
  }

  const track = (event: string, props?: Record<string, PropValue>): void => {
    if (isDisabled()) return;
    sendBeaconEvent(event, props, {
      site: options.site,
      siteId: options.siteId,
      endpoint,
    });
  };

  const hit = (): void => {
    const notFound = options.notFound ?? detectNotFound();
    track(notFound ? "not_found" : "pageview");
  };

  let globalFn: RotameterGlobal | undefined;
  if (exposeGlobal) {
    globalFn = (event, trackOpts) => track(event, trackOpts?.props);
    (window as WindowWithRotameter).rotameter = globalFn;
  }

  if (autoPageview) {
    hit();
  }

  let restorePushState: (() => void) | undefined;
  let onNavigate: (() => void) | undefined;
  const hist = window.history;

  if (options.spa && hist) {
    let lastUrl = loc.href;
    onNavigate = (): void => {
      if (loc.href !== lastUrl) {
        lastUrl = loc.href;
        hit();
      }
    };

    const origPushState = hist.pushState;
    const patchedPushState: typeof hist.pushState = function (
      this: History,
      ...args: Parameters<History["pushState"]>
    ): void {
      origPushState.apply(this, args);
      onNavigate?.();
    };
    hist.pushState = patchedPushState;
    restorePushState = () => {
      if (hist.pushState === patchedPushState) {
        hist.pushState = origPushState;
      }
    };

    window.addEventListener("popstate", onNavigate);
    document.addEventListener("astro:page-load", onNavigate);
  }

  let onClick: ((event: Event) => void) | undefined;
  if (options.outbound || options.downloads) {
    onClick = (event: Event): void => {
      const target = event.target as { closest?: (selector: string) => Element | null } | null;
      const anchor = target?.closest?.("a[href]") as HTMLAnchorElement | null | undefined;
      if (!anchor) return;
      const rawHref = anchor.getAttribute?.("href") ?? anchor.href;
      if (!rawHref) return;

      let resolved: URL;
      try {
        resolved = new URL(anchor.href || rawHref, loc.href);
      } catch {
        return;
      }
      if (resolved.protocol !== "http:" && resolved.protocol !== "https:") return;

      if (options.outbound && resolved.host && resolved.host !== loc.host) {
        track("outbound_click", { href: resolved.href });
      } else if (options.downloads && downloadPattern.test(resolved.pathname)) {
        track("file_download", { path: resolved.pathname });
      }
    };
    document.addEventListener("click", onClick);
  }

  const destroy = (): void => {
    if (destroyed) return;
    destroyed = true;
    restorePushState?.();
    if (onNavigate) {
      window.removeEventListener("popstate", onNavigate);
      document.removeEventListener("astro:page-load", onNavigate);
    }
    if (onClick) {
      document.removeEventListener("click", onClick);
    }
    if (globalFn && (window as WindowWithRotameter).rotameter === globalFn) {
      delete (window as WindowWithRotameter).rotameter;
    }
  };

  return { track, isDisabled, destroy };
}
