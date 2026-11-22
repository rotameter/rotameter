import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import vm from "node:vm";

const COLLECTOR_SRC = fs.readFileSync(
  path.resolve(import.meta.dirname, "../assets/collect.js"),
  "utf8",
);

function createBrowserEnv({
  attrs = { "data-site": "colinwolff.dev" },
  href = "https://colinwolff.dev/projects/rotameter?utm_source=hn",
  referrer = "https://news.ycombinator.com/",
  ignore = null,
  sendBeaconImpl = null,
} = {}) {
  const beacons = [];
  const fetches = [];
  const storageSetCalls = [];
  const listeners = new Map();
  let cookieWrites = 0;

  const locationObj = new URL(href);
  const location = {
    get href() { return locationObj.href; },
    set href(val) { locationObj.href = val; },
    get host() { return locationObj.host; },
    get pathname() { return locationObj.pathname; },
  };
  const history = {
    pushState(_s, _t, url) {
      if (url != null) locationObj.href = new URL(url, locationObj.href).href;
    },
  };
  const document = {
    currentScript: { getAttribute: (n) => (n in attrs ? attrs[n] : null) },
    referrer,
    addEventListener(type, fn) {
      listeners.set(type, [...(listeners.get(type) ?? []), fn]);
    },
    dispatchClick(anchor) {
      for (const fn of listeners.get("click") ?? []) {
        fn({ target: { closest: (sel) => (sel === "a[href]" ? anchor : null) } });
      }
    },
  };
  Object.defineProperty(document, "cookie", {
    get: () => "",
    set: () => { cookieWrites += 1; },
  });

  const localStorage = {
    getItem: (k) => (k === "rotameter_ignore" ? ignore : null),
    setItem: (k, v) => { storageSetCalls.push([k, v]); },
  };
  const navigator = {
    sendBeacon: sendBeaconImpl === false
      ? undefined
      : sendBeaconImpl ?? ((ep, blob) => (beacons.push({ ep, body: JSON.parse(blob.text) }), true)),
  };
  class FakeBlob {
    constructor(parts, opts) {
      this.text = parts.join("");
      this.type = opts?.type ?? "";
    }
  }

  const windowObj = {
    innerWidth: 1440,
    document,
    location,
    history,
    navigator,
    localStorage,
    Blob: FakeBlob,
    URL,
    addEventListener(type, fn) {
      listeners.set(`win:${type}`, [...(listeners.get(`win:${type}`) ?? []), fn]);
    },
    dispatchPopstate(nextUrl) {
      if (nextUrl) locationObj.href = new URL(nextUrl, locationObj.href).href;
      for (const fn of listeners.get("win:popstate") ?? []) fn({});
    },
    fetch(ep, init) {
      fetches.push({ ep, init, body: JSON.parse(init.body) });
      return Promise.resolve({ ok: true, status: 202 });
    },
  };
  windowObj.window = windowObj;
  windowObj.self = windowObj;
  vm.createContext(windowObj);
  vm.runInContext(COLLECTOR_SRC, windowObj);

  return {
    window: windowObj,
    document,
    history,
    locationObj,
    beacons,
    fetches,
    get cookieWrites() { return cookieWrites; },
    storageSetCalls,
  };
}

test("collect.js tracks pageview, imperative events, and enforces privacy", () => {
  const env = createBrowserEnv();
  assert.equal(env.beacons.length, 1);
  assert.equal(env.beacons[0].ep, "/v1/events");
  assert.deepEqual(env.beacons[0].body, {
    site_id: "colinwolff.dev",
    event: "pageview",
    url: "https://colinwolff.dev/projects/rotameter?utm_source=hn",
    referrer: "https://news.ycombinator.com/",
    viewport_width: 1440,
  });

  env.window.rotameter("signup_clicked", { props: { plan: "pro" } });
  assert.equal(env.beacons.length, 2);
  assert.equal(env.beacons[1].body.event, "signup_clicked");
  assert.deepEqual(env.beacons[1].body.props, { plan: "pro" });
  assert.equal(env.cookieWrites, 0);
  assert.equal(env.storageSetCalls.length, 0);
});

test("collect.js respects rotameter_ignore, missing data-site, custom endpoint, and 404", () => {
  const ignored = createBrowserEnv({ ignore: "true" });
  assert.equal(ignored.beacons.length, 0);
  assert.equal(ignored.fetches.length, 0);

  const missingSite = createBrowserEnv({ attrs: {} });
  assert.equal(missingSite.beacons.length, 0);

  const custom = createBrowserEnv({
    attrs: { "data-site": "site_01abc", "data-endpoint": "/api/e", "data-not-found": "" },
  });
  assert.equal(custom.beacons.length, 1);
  assert.equal(custom.beacons[0].ep, "/api/e");
  assert.equal(custom.beacons[0].body.event, "not_found");
});

test("collect.js tracks SPA navigation, outbound clicks, downloads, and fetch fallback", () => {
  const spa = createBrowserEnv({
    attrs: { "data-site": "colinwolff.dev", "data-spa": "", "data-outbound": "", "data-downloads": "" },
    sendBeaconImpl: () => false,
  });
  assert.equal(spa.fetches.length, 1);
  assert.equal(spa.fetches[0].init.keepalive, true);
  assert.equal(spa.fetches[0].body.event, "pageview");

  spa.history.pushState({}, "", spa.locationObj.href);
  assert.equal(spa.fetches.length, 1);

  spa.history.pushState({}, "", "/about");
  assert.equal(spa.fetches.length, 2);
  assert.equal(spa.fetches[1].body.url, "https://colinwolff.dev/about");
  spa.window.dispatchPopstate("/contact");
  assert.equal(spa.fetches.length, 3);
  assert.equal(spa.fetches[2].body.url, "https://colinwolff.dev/contact");

  spa.document.dispatchClick({ href: "https://github.com/colinwolff" });
  spa.document.dispatchClick({ href: "https://colinwolff.dev/files/cv.pdf" });
  spa.document.dispatchClick({ href: "mailto:hi@colinwolff.dev" });
  spa.document.dispatchClick({ href: "https://colinwolff.dev/posts/hi" });

  assert.equal(spa.fetches.length, 5);
  assert.equal(spa.fetches[3].body.event, "outbound_click");
  assert.deepEqual(spa.fetches[3].body.props, { href: "https://github.com/colinwolff" });
  assert.equal(spa.fetches[4].body.event, "file_download");
  assert.deepEqual(spa.fetches[4].body.props, { path: "/files/cv.pdf" });
});
