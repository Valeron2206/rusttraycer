ALTER TABLE agents ADD COLUMN model TEXT;
ALTER TABLE agents ADD COLUMN effort TEXT;
ALTER TABLE agents ADD COLUMN fast INTEGER NOT NULL DEFAULT 0;

CREATE TABLE model_profiles (
  id         TEXT PRIMARY KEY,
  name       TEXT NOT NULL UNIQUE,
  provider   TEXT NOT NULL,
  model      TEXT,
  effort     TEXT,
  fast       INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE harness_prefs (
  provider   TEXT PRIMARY KEY,
  model      TEXT,
  effort     TEXT,
  fast       INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL
);

INSERT OR REPLACE INTO schema_meta(key, value) VALUES ('schema', '7');
