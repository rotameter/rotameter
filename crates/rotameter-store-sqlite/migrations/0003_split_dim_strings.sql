CREATE TABLE dim_strings_new (
  id      INTEGER PRIMARY KEY,
  site_id BLOB NOT NULL,
  kind    INTEGER NOT NULL,
  value   TEXT NOT NULL,
  UNIQUE(site_id, kind, value)
);

INSERT INTO dim_strings_new (id, site_id, kind, value)
SELECT id, site_id, kind, value FROM dim_strings;

DROP TABLE dim_strings;

ALTER TABLE dim_strings_new RENAME TO dim_strings;
