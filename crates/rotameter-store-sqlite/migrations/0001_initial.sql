CREATE TABLE IF NOT EXISTS schema_migrations (
  version       INTEGER PRIMARY KEY,
  name          TEXT NOT NULL,
  applied_at_ms INTEGER NOT NULL
);

CREATE TABLE orgs (
  org_id        BLOB PRIMARY KEY,
  slug          TEXT NOT NULL UNIQUE,
  display_name  TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE sites (
  site_id            BLOB PRIMARY KEY,
  org_id             BLOB NOT NULL REFERENCES orgs(org_id) ON DELETE CASCADE,
  slug               TEXT NOT NULL,
  display_name       TEXT NOT NULL,
  timezone           TEXT NOT NULL DEFAULT 'UTC',
  salt_rotation      TEXT NOT NULL DEFAULT 'daily',
  respect_gpc        INTEGER NOT NULL DEFAULT 0,
  is_self_site       INTEGER NOT NULL DEFAULT 0,
  allowed_origins    TEXT NOT NULL DEFAULT '[]',
  allowed_query_keys TEXT NOT NULL DEFAULT '[]',
  raw_retention_days INTEGER,
  created_at_ms      INTEGER NOT NULL,
  updated_at_ms      INTEGER NOT NULL,
  UNIQUE(org_id, slug)
);

CREATE TABLE api_keys (
  key_id        BLOB PRIMARY KEY,
  org_id        BLOB NOT NULL REFERENCES orgs(org_id) ON DELETE CASCADE,
  key_prefix    TEXT NOT NULL,
  key_hash      BLOB NOT NULL UNIQUE,
  scope         TEXT NOT NULL,
  site_id       BLOB REFERENCES sites(site_id) ON DELETE CASCADE,
  label         TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL,
  last_used_ms  INTEGER,
  revoked_at_ms INTEGER
);

CREATE TABLE dim_strings (
  id      INTEGER PRIMARY KEY,
  site_id BLOB NOT NULL REFERENCES sites(site_id) ON DELETE CASCADE,
  kind    INTEGER NOT NULL,
  value   TEXT NOT NULL,
  UNIQUE(site_id, kind, value)
);

CREATE TABLE sessions (
  site_id         BLOB NOT NULL,
  session_id      BLOB NOT NULL,
  visitor_id      BLOB NOT NULL,
  started_at_ms   INTEGER NOT NULL,
  last_event_ms   INTEGER NOT NULL,
  event_count     INTEGER NOT NULL,
  pageview_count  INTEGER NOT NULL,
  entry_path_id   INTEGER NOT NULL,
  exit_path_id    INTEGER NOT NULL,
  ref_host_id     INTEGER NOT NULL,
  channel_id      INTEGER NOT NULL,
  utm_source_id   INTEGER NOT NULL,
  utm_medium_id   INTEGER NOT NULL,
  utm_campaign_id INTEGER NOT NULL,
  country_id      INTEGER NOT NULL,
  region_id       INTEGER NOT NULL,
  browser_id      INTEGER NOT NULL,
  os_id           INTEGER NOT NULL,
  device_type     INTEGER NOT NULL,
  viewport_class  INTEGER NOT NULL,
  PRIMARY KEY (site_id, session_id)
) WITHOUT ROWID;

CREATE INDEX idx_sessions_range ON sessions(site_id, started_at_ms, visitor_id);
CREATE INDEX idx_sessions_realtime ON sessions(site_id, last_event_ms);
CREATE INDEX idx_sessions_visitor_active ON sessions(site_id, visitor_id, last_event_ms);

CREATE TABLE events (
  site_id        BLOB NOT NULL,
  ts_ms          INTEGER NOT NULL,
  session_id     BLOB NOT NULL,
  seq_in_session INTEGER NOT NULL,
  visitor_id     BLOB NOT NULL,
  event_name_id  INTEGER NOT NULL,
  path_id        INTEGER NOT NULL,
  props_json     TEXT,
  PRIMARY KEY (site_id, ts_ms, session_id, seq_in_session)
) WITHOUT ROWID;

CREATE INDEX idx_events_session_seq ON events(site_id, session_id, seq_in_session);
CREATE INDEX idx_events_path_ts ON events(site_id, path_id, ts_ms);

CREATE TABLE rollups_hourly (
  site_id           BLOB NOT NULL,
  bucket_ts_ms      INTEGER NOT NULL,
  pageviews         INTEGER NOT NULL,
  events_total      INTEGER NOT NULL,
  bounced_visits    INTEGER NOT NULL,
  visits_started    INTEGER NOT NULL,
  total_duration_ms INTEGER NOT NULL,
  PRIMARY KEY (site_id, bucket_ts_ms)
) WITHOUT ROWID;

CREATE TABLE rollups_daily (
  site_id           BLOB NOT NULL,
  day_utc           INTEGER NOT NULL,
  pageviews         INTEGER NOT NULL,
  events_total      INTEGER NOT NULL,
  bounced_visits    INTEGER NOT NULL,
  visits_started    INTEGER NOT NULL,
  total_duration_ms INTEGER NOT NULL,
  PRIMARY KEY (site_id, day_utc)
) WITHOUT ROWID;
