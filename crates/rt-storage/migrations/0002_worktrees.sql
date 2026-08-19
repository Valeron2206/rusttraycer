-- 0002_worktrees.sql
-- Expand agents.run_location CHECK to 'local' | 'worktree' and add worktrees.
-- SQLite cannot ALTER CHECK: rebuild agents (FK off).

PRAGMA foreign_keys = OFF;

CREATE TABLE agents_new (
  id           TEXT PRIMARY KEY,
  task_id      TEXT NOT NULL REFERENCES tasks(id),
  host_id      TEXT NOT NULL REFERENCES host(id),
  parent_id    TEXT REFERENCES agents_new(id),
  interface    TEXT NOT NULL CHECK (interface IN ('chat')),
  provider     TEXT NOT NULL,
  status       TEXT NOT NULL CHECK (status IN ('idle', 'running', 'error')),
  run_location TEXT NOT NULL CHECK (run_location IN ('local', 'worktree')),
  created_at   TEXT NOT NULL
);

INSERT INTO agents_new (
  id, task_id, host_id, parent_id, interface, provider, status, run_location, created_at
)
SELECT
  id, task_id, host_id, parent_id, interface, provider, status, run_location, created_at
FROM agents;

DROP TABLE agents;
ALTER TABLE agents_new RENAME TO agents;

CREATE INDEX idx_agents_task ON agents(task_id, created_at, id);
CREATE INDEX idx_agents_status ON agents(status);

CREATE TABLE worktrees (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id),
  agent_id TEXT NOT NULL UNIQUE REFERENCES agents(id),
  path TEXT NOT NULL UNIQUE,
  branch TEXT NOT NULL,
  created_at TEXT NOT NULL
);

INSERT OR REPLACE INTO schema_meta(key, value) VALUES ('schema', '2');

PRAGMA foreign_keys = ON;
