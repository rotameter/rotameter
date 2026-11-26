-- RM-32: Persistent rotating CSPRNG salts for multi-node & restart-safe deduplication.

CREATE TABLE IF NOT EXISTS rotating_salts (
    rotation TEXT NOT NULL CHECK (rotation IN ('daily', 'weekly', 'monthly')),
    window_id INTEGER NOT NULL,
    salt BLOB NOT NULL CHECK (length(salt) = 32),
    created_at_ms INTEGER NOT NULL,
    PRIMARY KEY (rotation, window_id)
);
