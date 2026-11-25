import assert from "node:assert/strict";
import test from "node:test";
import {
  DEFAULT_DOWNLOAD_PATTERN,
  initRotameterBeacon,
  isRotameterIgnored,
  sendBeaconEvent,
  setRotameterIgnored,
} from "../src/beacon.ts";

interface SentRecord {
  url: string;
  body: Record<string, unknown>;
  via: "sendBeacon" | "fetch";
}

function createMockBrowser(initialHref = "https://colinwolff.dev/posts/hello") {
  let href = initialHref;
  const storage = new Map<string, string>();
  const windowListeners = new Map<string, Set<EventListener>>();
  const docListeners = new Map<string, Set<EventListener>>();
  const sent: SentRecord[] = [];
  let useSendBeacon = true;
  let sendBeaconResult = true;
  let metaNotFound = false;
  let bodyNotFoundAttr: string | null = null;

  const addTo = (map: Map<string, Set<EventListener>>, type: string, fn: EventListener) => {
    let set = map.get(type);
    if (!set) {
      set = new Set();
      map.set(type, set);
    }
    set.add(fn);
  };
  const removeFrom = (map: Map<string, Set<EventListener>>, type: string, fn: EventListener) => {
    map.get(type)?.delete(fn);
  };
  const emit = (map: Map<string, Set<EventListener>>, type: string, event: unknown) => {
    for (const fn of map.get(type) ?? []) {
      fn(event as Event);
    }
  };

  const mockLocation = {
    get href() {
      return href;
    },
    set href(next: string) {
      href = next;
    },
    get host() {
      return new URL(href).host;
    },
    get hostname() {
      return new URL(href).hostname;
    },
  };

  const mockHistory = {
    pushState(_data: unknown, _unused: string, url?: string | URL | null) {
      if (url != null) {
        href = new URL(String(url), href).href;
      }
    },
  };

  const localStorageObj = {
    getItem: (k: string) => storage.get(k) ?? null,
    setItem: (k: string, v: string) => storage.set(k, v),
    removeItem: (k: string) => storage.delete(k),
  };

  const mockWindow: Record<string, unknown> = {
    innerWidth: 1280,
    location: mockLocation,
    history: mockHistory,
    localStorage: localStorageObj,
    addEventListener: (type: string, fn: EventListener) => addTo(windowListeners, type, fn),
    removeEventListener: (type: string, fn: EventListener) => removeFrom(windowListeners, type, fn),
  };

  const mockDocument: Record<string, unknown> = {
    referrer: "https://news.ycombinator.com/",
    documentElement: { hasAttribute: () => false },
    body: {
      hasAttribute: (name: string) =>
        name === "data-rotameter-not-found" && bodyNotFoundAttr !== null,
      getAttribute: (name: string) =>
        name === "data-rotameter-not-found" ? bodyNotFoundAttr : null,
    },
    querySelector: (sel: string) =>
      sel === 'meta[name="rotameter-not-found"]' && metaNotFound
        ? { getAttribute: () => "true" }
        : null,
    addEventListener: (type: string, fn: EventListener) => addTo(docListeners, type, fn),
    removeEventListener: (type: string, fn: EventListener) => removeFrom(docListeners, type, fn),
  };

  const mockNavigator: Record<string, unknown> = {
    sendBeacon(url: string, data: unknown) {
      if (!useSendBeacon || !sendBeaconResult) return false;
      const blobLike = data as { _text?: string };
      sent.push({
        url,
        body: JSON.parse(blobLike._text ?? "{}") as Record<string, unknown>,
        via: "sendBeacon",
      });
      return true;
    },
  };

  const mockFetch = async (url: string | URL, init?: RequestInit) => {
    sent.push({
      url: String(url),
      body: JSON.parse(String(init?.body ?? "{}")) as Record<string, unknown>,
      via: "fetch",
    });
    return new Response(null, { status: 202 });
  };

  class MockBlob {
    _text: string;
    type: string;
    constructor(parts: BlobPart[], opts?: BlobPropertyBag) {
      this._text = parts.map((p) => String(p)).join("");
      this.type = opts?.type ?? "";
    }
  }

  const prev = new Map<string, PropertyDescriptor | undefined>();
  const setGlobal = (key: string, val: unknown) => {
    if (!prev.has(key)) prev.set(key, Object.getOwnPropertyDescriptor(globalThis, key));
    Object.defineProperty(globalThis, key, {
      value: val,
      writable: true,
      configurable: true,
    });
  };

  setGlobal("window", mockWindow);
  setGlobal("document", mockDocument);
  setGlobal("navigator", mockNavigator);
  setGlobal("fetch", mockFetch);
  setGlobal("Blob", MockBlob);

  return {
    sent,
    storage,
    mockWindow,
    mockLocation,
    mockHistory,
    setMetaNotFound(flag: boolean) {
      metaNotFound = flag;
    },
    setBodyNotFoundAttr(val: string | null) {
      bodyNotFoundAttr = val;
    },
    setSendBeacon(enabled: boolean, result = true) {
      useSendBeacon = enabled;
      sendBeaconResult = result;
    },
    emitWindow(type: string, ev: unknown = {}) {
      emit(windowListeners, type, ev);
    },
    emitDoc(type: string, ev: unknown = {}) {
      emit(docListeners, type, ev);
    },
    listenerCounts() {
      return {
        popstate: windowListeners.get("popstate")?.size ?? 0,
        astroPageLoad: docListeners.get("astro:page-load")?.size ?? 0,
        click: docListeners.get("click")?.size ?? 0,
      };
    },
    restore() {
      for (const [k, desc] of prev.entries()) {
        if (desc) Object.defineProperty(globalThis, k, desc);
        else delete (globalThis as Record<string, unknown>)[k];
      }
    },
  };
}

test("beacon sends SSR no-op safely outside browser", () => {
  assert.equal(isRotameterIgnored(), false);
  assert.equal(sendBeaconEvent("pageview"), false);
  const handle = initRotameterBeacon();
  assert.equal(handle.isDisabled(), true);
  handle.track("custom");
  handle.destroy();
});

test("beacon tracks pageviews, custom events, not_found, opt-out, SPA dedup, outbound, downloads, and cleanup", () => {
  const browser = createMockBrowser();
  try {
    const handle = initRotameterBeacon({
      endpoint: "/api/e",
      spa: true,
      outbound: true,
      downloads: true,
    });

    assert.equal(handle.isDisabled(), false);
    assert.equal(browser.sent.length, 1);
    assert.deepEqual(browser.sent[0], {
      url: "/api/e",
      via: "sendBeacon",
      body: {
        event: "pageview",
        url: "https://colinwolff.dev/posts/hello",
        referrer: "https://news.ycombinator.com/",
        viewport_width: 1280,
      },
    });

    // window.rotameter global helper
    (browser.mockWindow.rotameter as (e: string, o?: { props?: Record<string, string> }) => void)(
      "newsletter_signup",
      { props: { source: "footer" } },
    );
    assert.equal(browser.sent.length, 2);
    assert.equal(browser.sent[1].body.event, "newsletter_signup");
    assert.deepEqual(browser.sent[1].body.props, { source: "footer" });

    // SPA navigation via pushState + deduplicated astro:page-load
    browser.mockHistory.pushState({}, "", "/projects");
    assert.equal(browser.sent.length, 3);
    assert.equal(browser.sent[2].body.url, "https://colinwolff.dev/projects");
    browser.emitDoc("astro:page-load");
    assert.equal(browser.sent.length, 3);

    // popstate navigation
    browser.mockLocation.href = "https://colinwolff.dev/about";
    browser.emitWindow("popstate");
    assert.equal(browser.sent.length, 4);
    assert.equal(browser.sent[3].body.url, "https://colinwolff.dev/about");

    // Outbound click + file download
    const fakeClick = (href: string) => ({
      target: {
        closest: () => ({
          href,
          getAttribute: () => href,
        }),
      },
    });
    browser.emitDoc("click", fakeClick("https://github.com/rotameter/rotameter"));
    assert.equal(browser.sent[4].body.event, "outbound_click");
    assert.deepEqual(browser.sent[4].body.props, {
      href: "https://github.com/rotameter/rotameter",
    });

    browser.emitDoc("click", fakeClick("/files/whitepaper.pdf"));
    assert.equal(browser.sent[5].body.event, "file_download");
    assert.deepEqual(browser.sent[5].body.props, { path: "/files/whitepaper.pdf" });
    assert.equal(DEFAULT_DOWNLOAD_PATTERN.test("/files/whitepaper.pdf"), true);

    // Non-HTTP links are ignored
    browser.emitDoc("click", fakeClick("mailto:hello@colinwolff.dev"));
    assert.equal(browser.sent.length, 6);

    // sendBeacon returning false falls back to keepalive fetch
    browser.setSendBeacon(true, false);
    handle.track("cta_click", { props: undefined as unknown as never });
    assert.equal(browser.sent[6].via, "fetch");

    // destroy cleans up listeners, history.pushState, and window.rotameter
    handle.destroy();
    assert.deepEqual(browser.listenerCounts(), { popstate: 0, astroPageLoad: 0, click: 0 });
    assert.equal(browser.mockWindow.rotameter, undefined);

    // notFound mode (via meta tag auto-detection + explicit site)
    browser.setMetaNotFound(true);
    const nf = initRotameterBeacon({ site: "site_123" });
    assert.equal(browser.sent[7].body.event, "not_found");
    assert.equal(browser.sent[7].body.site_id, "site_123");
    nf.destroy();
    browser.setMetaNotFound(false);

    // data-rotameter-not-found="false" does not trigger not_found; bare "" / "true" does
    browser.setBodyNotFoundAttr("false");
    const attrFalse = initRotameterBeacon();
    assert.equal(browser.sent[8].body.event, "pageview");
    attrFalse.destroy();

    browser.setBodyNotFoundAttr("");
    const attrBare = initRotameterBeacon();
    assert.equal(browser.sent[9].body.event, "not_found");
    attrBare.destroy();
    browser.setBodyNotFoundAttr(null);

    // localStorage.rotameter_ignore opt-out suppresses everything
    assert.equal(setRotameterIgnored(true), true);
    assert.equal(isRotameterIgnored(), true);
    const ignoredHandle = initRotameterBeacon();
    assert.equal(ignoredHandle.isDisabled(), true);
    ignoredHandle.track("should_not_send");
    assert.equal(browser.sent.length, 10);
    ignoredHandle.destroy();
    assert.equal(setRotameterIgnored(false), true);
    assert.equal(isRotameterIgnored(), false);

    // localhost is suppressed by default
    browser.mockLocation.href = "http://localhost:4321/";
    const localHandle = initRotameterBeacon();
    assert.equal(localHandle.isDisabled(), true);
    localHandle.track("local_test");
    assert.equal(browser.sent.length, 10);
    localHandle.destroy();
  } finally {
    browser.restore();
  }
});
