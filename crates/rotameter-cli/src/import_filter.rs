#![forbid(unsafe_code)]
//! Proxy-aware noise filtering for access logs (`GET`/`HEAD`, `200`/`404`, HTML & static assets).

const STATIC_EXTENSIONS: &[&str] = &[
    "js", "mjs", "cjs", "css", "map", "png", "jpg", "jpeg", "gif", "svg", "ico", "webp", "avif",
    "woff", "woff2", "ttf", "otf", "eot", "xml", "txt", "json", "wasm", "pdf", "zip", "gz", "br",
    "mp4", "webm", "mp3",
];

const NOISE_PATHS: &[&str] = &["/favicon.ico", "/robots.txt", "/sitemap.xml"];

/// Classifies an HTTP access log hit into a Rotameter event name, or `None` to skip noise.
pub(crate) fn classify_proxy_hit(
    method: &str,
    status: u16,
    target: &str,
    content_type: Option<&str>,
) -> Option<&'static str> {
    if !matches!(method.to_ascii_uppercase().as_str(), "GET" | "HEAD") {
        return None;
    }

    let event = match status {
        200 => "pageview",
        404 => "not_found",
        _ => return None,
    };

    if let Some(ct) = content_type {
        let media = ct
            .split(';')
            .next()
            .map_or("", str::trim)
            .to_ascii_lowercase();
        if !media.is_empty() && media != "text/html" && media != "application/xhtml+xml" {
            return None;
        }
    }

    let path = strip_query_and_fragment(target);
    if is_noise_path(path) {
        return None;
    }

    Some(event)
}

fn strip_query_and_fragment(target: &str) -> &str {
    let without_scheme = target
        .strip_prefix("https://")
        .or_else(|| target.strip_prefix("http://"))
        .and_then(|rest| rest.find('/').map(|idx| &rest[idx..]))
        .unwrap_or(target);
    let no_frag = without_scheme.split('#').next().unwrap_or(without_scheme);
    no_frag.split('?').next().unwrap_or(no_frag)
}

fn is_noise_path(path: &str) -> bool {
    if path.starts_with("/.well-known/") || NOISE_PATHS.iter().any(|p| path.eq_ignore_ascii_case(p))
    {
        return true;
    }
    let last_segment = path.rsplit('/').next().unwrap_or(path);
    let Some((_, ext)) = last_segment.rsplit_once('.') else {
        return false;
    };
    STATIC_EXTENSIONS
        .iter()
        .any(|known| ext.eq_ignore_ascii_case(known))
}

#[cfg(test)]
mod tests {
    use super::classify_proxy_hit;

    #[test]
    fn filters_methods_statuses_content_types_and_static_assets() {
        assert_eq!(
            classify_proxy_hit(
                "GET",
                200,
                "/docs/intro?ref=hn",
                Some("text/html; charset=utf-8")
            ),
            Some("pageview")
        );
        assert_eq!(
            classify_proxy_hit("HEAD", 404, "/missing-page", None),
            Some("not_found")
        );
        assert_eq!(classify_proxy_hit("POST", 200, "/api/submit", None), None);
        assert_eq!(classify_proxy_hit("GET", 301, "/old", None), None);
        assert_eq!(classify_proxy_hit("GET", 500, "/boom", None), None);
        assert_eq!(
            classify_proxy_hit("GET", 200, "/api/data", Some("application/json")),
            None
        );
        assert_eq!(
            classify_proxy_hit("GET", 200, "/assets/app.js?v=123", None),
            None
        );
        assert_eq!(classify_proxy_hit("GET", 200, "/robots.txt", None), None);
        assert_eq!(
            classify_proxy_hit("GET", 200, "/.well-known/acme-challenge/abc", None),
            None
        );
        // Dotted non-asset page paths remain tracked.
        assert_eq!(
            classify_proxy_hit("GET", 200, "/docs/v1.2", Some("text/html")),
            Some("pageview")
        );
    }
}
