-- 0009_v21.sql
-- v2.1 accounts (labels only), user presets, stash, worktree prefix.
-- Labels only; no credential columns. agents.account_id is nullable TEXT, no FK.

CREATE TABLE provider_accounts (
  id TEXT PRIMARY KEY,
  provider TEXT NOT NULL,
  label TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE (provider, label)
);

CREATE TABLE user_presets (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  default_role TEXT NOT NULL,
  title_hint TEXT,
  prompt TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE prompt_stash (
  id TEXT PRIMARY KEY,
  body TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE worktree_settings (
  workspace_id TEXT PRIMARY KEY REFERENCES workspaces(id),
  branch_prefix TEXT NOT NULL DEFAULT 'rt/'
);

ALTER TABLE agents ADD COLUMN account_id TEXT;

INSERT OR REPLACE INTO schema_meta(key, value) VALUES ('schema', '9');
