ALTER TABLE agents ADD COLUMN role TEXT NOT NULL DEFAULT 'coder';
ALTER TABLE tasks ADD COLUMN preset TEXT;

INSERT OR REPLACE INTO schema_meta(key, value) VALUES ('schema', '8');
