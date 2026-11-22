ALTER TABLE rollups_daily ADD COLUMN visitors INTEGER NOT NULL DEFAULT 0;

CREATE TABLE seen_visitors_daily (
  site_id    BLOB NOT NULL,
  day_utc    INTEGER NOT NULL,
  visitor_id BLOB NOT NULL,
  PRIMARY KEY (site_id, day_utc, visitor_id)
) WITHOUT ROWID;

CREATE TABLE rollups_path_daily (
  site_id           BLOB NOT NULL,
  day_utc           INTEGER NOT NULL,
  path_id           INTEGER NOT NULL,
  pageviews         INTEGER NOT NULL,
  events_total      INTEGER NOT NULL,
  visits            INTEGER NOT NULL,
  visitors          INTEGER NOT NULL,
  bounced_visits    INTEGER NOT NULL,
  total_duration_ms INTEGER NOT NULL,
  PRIMARY KEY (site_id, day_utc, path_id)
) WITHOUT ROWID;
