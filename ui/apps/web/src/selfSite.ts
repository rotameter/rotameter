const SELF_SITE_ATTR = "data-rotameter-self-site";

/**
 * Injects the same-origin `/v1/collect.js` beacon once when `self_site_id` is enabled
 * in `/v1/capabilities`. Respects `--no-self-site` / `ROTAMETER_SELF_SITE=false` at runtime.
 */
export function mountSelfSiteCollector(
  selfSiteId: string | undefined,
  doc: Document | undefined = typeof document !== "undefined" ? document : undefined,
): HTMLScriptElement | null {
  if (!selfSiteId || !doc) return null;
  if (doc.querySelector(`script[${SELF_SITE_ATTR}]`)) return null;

  const script = doc.createElement("script");
  script.defer = true;
  script.src = "/v1/collect.js";
  script.setAttribute("data-site", selfSiteId);
  script.setAttribute("data-endpoint", "/v1/events");
  script.setAttribute("data-spa", "true");
  script.setAttribute(SELF_SITE_ATTR, "true");
  doc.head.appendChild(script);
  return script;
}
