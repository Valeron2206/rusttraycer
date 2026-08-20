-- 0011_shells.sql
-- Persist user shells (shell.create) so list/resume survive host restart.
-- Live PTY remains process memory; this table is the session roster only.

CREATE TABLE shells (
  id            TEXT PRIMARY KEY,
  workspace_id  TEXT NOT NULL REFERENCES workspaces(id),
  task_id       TEXT REFERENCES tasks(id),
  cwd           TEXT NOT NULL,
  cols          INTEGER NOT NULL,
  rows          INTEGER NOT NULL,
  last_pty_id   TEXT NOT NULL DEFAULT '',
  created_at    TEXT NOT NULL
);

CREATE INDEX idx_shells_workspace ON shells(workspace_id, created_at, id);
CREATE INDEX idx_shells_task ON shells(task_id, created_at, id);

INSERT OR REPLACE INTO schema_meta(key, value) VALUES ('schema', '11');
