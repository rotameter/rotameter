# Metric-by-Metric Comparison & Definitions

Every metric tracked or planned across **Rotameter (`0.1.0` / `0.4.0`)** compared against **GoatCounter**, **Plausible CE**, **Umami**, **Matomo**, and **GA4**, followed by exact mathematical definitions implemented in `0.1.0+`.

*See [`COMPARISON-FEATURES.md`](./COMPARISON-FEATURES.md) for runtime, privacy, and interface comparisons, and [`ROADMAP.md`](./ROADMAP.md) for milestone timelines.*

---

## 1. Metric Coverage Matrix

### Core Traffic & Sessions

| Metric | Rotameter | GoatCounter | Plausible CE | Umami | Matomo | GA4 |
| --- | --- | --- | --- | --- | --- | --- |
| **Pageviews** | `0.1.0` | Yes | Yes | Yes | Yes | Yes ("Views") |
| **Unique Visitors** | `0.1.0` (daily salt default; `weekly`/`monthly` option) | Yes (8h session hash) | Yes (daily salt) | Yes (monthly/salt) | Yes (cookie / fingerprint) | Yes ("Active/Total Users" via cookie) |
| **Visits / Sessions** | `0.1.0` (30m sliding inactivity) | No (only 8h unique count) | Yes (30m sliding) | Yes | Yes (30m timeout) | Yes (30m timeout) |
| **Views per Visit** | `0.1.0` | No | Yes | Yes | Yes ("Actions/Visit") | Yes ("Views/Session") |
| **Bounce Rate** | `0.1.0` (1-event visit ratio) | No | Yes (single-page visit) | Yes | Yes | Inverted ("Engaged Sessions") |
| **Avg. Visit Duration** | `0.1.0` | No | Yes | Yes | Yes | Avg. Engagement Time |
| **Engaged Time / Active Time** | `0.4.0` (opt-in heartbeat module, `RM-21`) | No | No | No | Heartbeat timer | Yes (native `user_engagement`) |
| **Live / Realtime Visitors** | `0.1.0` (active in last 5 min) | No (recent view log) | Yes (5 min window) | Yes (5 min window) | Yes (real-time widget) | Yes (30 min window) |
| **Bot / Crawler Hits Excluded** | `0.1.0` (counted separately in health/diag option) | Excluded | Excluded | Excluded | Excluded | Excluded |

### Content & Navigation

| Metric | Rotameter | GoatCounter | Plausible CE | Umami | Matomo | GA4 |
| --- | --- | --- | --- | --- | --- | --- |
| **Top Pages (normalized path)** | `0.1.0` | Yes | Yes | Yes | Yes | Yes |
| **Entry Pages** | `0.1.0` | No | Yes | Entry URL | Yes | Landing Page |
| **Exit Pages** | `0.1.0` | No | Yes | Exit URL | Yes | Exit Reports |
| **404 Error Paths** | `0.1.0` | Custom convention | `404` event goal | Custom event | Page title / custom | Custom event |
| **Page Scroll Depth & Click Heatmaps** | `0.4.0` (opt-in privacy-safe module, `RM-21`) | No | Optional script / custom | Custom event | Plugin / custom | Enhanced (`90%` default) |
| **User Flows / Path Transitions** | `0.4.0` (`RM-21`) | No | No | Journeys | Transitions | Path Exploration |
| **Time on Specific Page** | `0.1.0` (approx via next event) / `0.4.0` (heartbeat) | No | Entry/step delta | No | Yes | Avg. engagement/page |

### Acquisition & Attribution

| Metric | Rotameter | GoatCounter | Plausible CE | Umami | Matomo | GA4 |
| --- | --- | --- | --- | --- | --- | --- |
| **Referrer Host & Normalized Source** | `0.1.0` | Yes | Yes | Yes | Yes | Yes |
| **Referrer Full URL / Path** | `0.1.0` (capped cardinality) | Yes | Yes | Referrer domain | Yes | Page Referrer |
| **Channel Grouping (Direct/Search/Social)** | `0.1.0` (rule table) | Basic | Yes | Basic | Yes | Default Channel Groups |
| **UTM (`source/medium/campaign/content/term`)** | `0.1.0` | Campaign query param | Yes | Yes | Yes | Yes |
| **First-Touch vs. Last-Touch Multi-Day Attribution** | `0.4.0` (within salt window, `RM-21`) | No | Session-level only | Session-level only | Multi-day cookie | Data-driven / Cross-session |
| **Google Search Console Keywords/Clicks** | `0.4.0` (`RM-21`) | No | Integration | No | Paid plugin | Native integration |

### Audience & Technology

| Metric | Rotameter | GoatCounter | Plausible CE | Umami | Matomo | GA4 |
| --- | --- | --- | --- | --- | --- | --- |
| **Country** | `0.1.0` (`CF-IPCountry` or `.mmdb`) | Yes | Yes | Yes | Yes | Yes |
| **Region / Subdivision** | `0.1.0` (`CF-Region-Code` or `.mmdb`) | Yes (if DB supports) | Yes | Yes | Yes | Yes |
| **City** | `0.1.0` (`.mmdb` / optional header) | Optional | Yes | Yes | Yes | Yes |
| **Browser & Major Version** | `0.1.0` | Yes | Yes | Yes | Yes | Yes |
| **Operating System & Version** | `0.1.0` | Yes | Yes | Yes | Yes | Yes |
| **Device Class (`desktop/mobile/tablet`)** | `0.1.0` | Screen width / UA | Yes | Yes | Yes | Yes |
| **Viewport / Screen Size Bucket** | `0.1.0` (`collect.js` bucket) | Yes (exact/bucket) | Screen size | Screen resolution | Resolution | Screen resolution |
| **Browser Language** | `0.1.0` (`Accept-Language` primary tag) | Yes | No | Yes | Yes | Yes |

### Custom Events, Funnels & Performance

| Metric | Rotameter | GoatCounter | Plausible CE | Umami | Matomo | GA4 |
| --- | --- | --- | --- | --- | --- | --- |
| **Custom Named Events** | `0.1.0` | Custom path prefix | Yes | Yes | Yes | Yes |
| **Custom Key-Value Event Properties** | `0.1.0` | No | Yes | Yes | Custom Dimensions | Event Parameters |
| **Outbound Link Clicks** | `0.1.0` (`collect.js` auto) | Manual | Script extension | Manual | Built-in | Enhanced measurement |
| **File Downloads** | `0.1.0` (`collect.js` auto) | Manual | Script extension | Manual | Built-in | Enhanced measurement |
| **Conversion Rate (% of visitors / visits)** | `0.1.0` (per event) / `0.4.0` (goals, `RM-21`) | No | Yes | Goal conversion | Yes | Key Event Rate |
| **Multi-Step Funnels & Drop-off** | `0.4.0` (`RM-21`) | No | Yes | Yes | Paid plugin | Funnel Exploration |
| **Retention Cohorts (D1/D7/D30)** | `0.4.0` (`RM-21`) | No | No | Yes | Paid plugin | Cohort Exploration |
| **Revenue / Ecommerce Attribution** | `0.4.0` (`RM-21`) | No | Revenue goals | Revenue reporting | Ecommerce | Ecommerce |
| **Core Web Vitals (LCP/INP/CLS/TTFB)** | Out of scope (use `/status` or RUM) | No | No | No | No | No (separate CrUX) |

---

## 2. Exact Rotameter `0.1.0+` Mathematical Definitions

All core metrics are derived cleanly from the canonical `sessions` and `events` tables so API, CLI, MCP, TUI, and Web UI always agree to the decimal.

### 2.1 Visitor (`visitor_id`)
$$\text{visitor\_id} = \text{HMAC-SHA256}(\text{salt}_{\text{window}}, \text{site\_id} \,\|\, \text{client\_ip} \,\|\, \text{user\_agent})[0..16]$$
- **`visitors`** over range $[T_0, T_1)$: `COUNT(DISTINCT visitor_id)` across events/sessions in $[T_0, T_1)$.
- **Multi-day caveat under `daily` salt:** Because $\text{salt}_{\text{window}}$ rotates at `00:00 UTC` (with a 30-minute active-session grace carryover; see [`SPEC-PRIVACY.md`](./SPEC-PRIVACY.md)), a human visiting on Monday and again on Wednesday receives two distinct `visitor_id` hashes and counts as `2` in a 7-day `visitors` query—matching Plausible's strict cookieless privacy model. Sites needing multi-day uniqueness within a single week or month can opt into `salt_rotation = "weekly"` or `"monthly"`.

### 2.2 Session (`session_id`)
- A session groups sequential non-bot events sharing `(site_id, visitor_id)` separated by **$\le 30\text{ minutes}$** (`1,800,000 ms`) of inactivity, regardless of midnight UTC rollover (via the active-session cache and 30-minute grace check).
- **`visits` (sessions)**: `COUNT(DISTINCT session_id)` whose `started_at` falls in $[T_0, T_1)$.

### 2.3 Pageviews & Views per Visit
- **`pageviews`**: `COUNT(*)` of events in $[T_0, T_1)$ where `event_name = 'pageview'`. (Custom events such as `outbound_click`, `file_download`, `not_found`, and app-defined events do **not** inflate `pageviews`.)
- **`views_per_visit`**:
  $$\text{views\_per\_visit} = \frac{\text{pageviews}}{\max(\text{visits}, 1)} \quad (\text{returns } 0.0 \text{ when } \text{visits} = 0)$$

### 2.4 Bounce Rate
- A session is a **bounce** when its total **interaction event count** is `1` (i.e., `event_count == 1` — a visitor loaded one page and triggered zero subsequent pageviews or tracked custom events before the 30-minute session timeout).
- **`bounce_rate`** (returned as fraction `[0.0, 1.0]` in API JSON; formatted as `%` in CLI/TUI):
  $$\text{bounce\_rate} = \frac{\text{bounced\_visits}}{\text{visits}}$$

### 2.5 Visit Duration (`visit_duration_ms`)
- For each session $s$: $\text{duration}(s) = \text{last\_event\_at}(s) - \text{started\_at}(s)$.
- Single-event bounced sessions therefore have $\text{duration}(s) = 0\text{ ms}$ (identical to Plausible CE; no inflated fake timeout padding).
- **`avg_visit_duration_ms`**: arithmetic mean across all sessions starting in $[T_0, T_1)$ (and **`avg_engaged_visit_duration_ms`** excluding `0 ms` single-hit sessions is optionally inspectable in query extensions, never mixed into the primary `avg_visit_duration_ms`).

### 2.6 Entry Page & Exit Page
- **`entry_page`**: `path` of the first `pageview` event (`seq_in_session = min`) in a session.
- **`exit_page`**: `path` of the last `pageview` event (`seq_in_session = max`) in a session.

### 2.7 Realtime Visitors (`realtime_visitors`)
- `COUNT(DISTINCT visitor_id)` with at least one non-bot event in `[now - 300_000 ms, now]`.

### 2.8 Special Built-In Events (`not_found`, `outbound_click`, `file_download`)
- **`not_found`**: Emitted when `collect.js` runs with `data-not-found` / `rotameter('not_found')` or when Astro/CF Workers SSR emits a `404` status hit. Stored with `event_name = 'not_found'` and normalized `path`, so 404 tracking works cleanly without corrupting valid `pageviews`.
- **`outbound_click`**: Emitted on external `<a href>` click with `props = { "url": "...", "host": "..." }`.
- **`file_download`**: Emitted on matching asset extensions (`.pdf`, `.zip`, `.tar.gz`, `.csv`, `.bin`, etc.) with `props = { "url": "...", "ext": "..." }`.
