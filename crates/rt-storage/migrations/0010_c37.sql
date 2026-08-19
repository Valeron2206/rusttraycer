-- 0010_c37.sql
-- C37: agents.task_id nullable so a terminal/shell can exist without a Task.
-- workspace_id is set when task_id is NULL. Rebuild agents; FK stay restrict.

PRAGMA foreign_keys = OFF;

CREATE TABLE agents_new (
  id                   TEXT PRIMARY KEY,
  task_id              TEXT REFERENCES tasks(id),
  host_id              TEXT NOT NULL REFERENCES host(id),
  parent_id            TEXT REFERENCES agents_new(id),
  interface            TEXT NOT NULL CHECK (interface IN ('chat', 'terminal')),
  provider             TEXT NOT NULL,
  status               TEXT NOT NULL CHECK (status IN ('idle', 'running', 'error')),
  run_location         TEXT NOT NULL CHECK (run_location IN ('local', 'worktree')),
  created_at           TEXT NOT NULL,
  provider_session_id  TEXT NULL,
  model                TEXT,
  effort               TEXT,
  fast                 INTEGER NOT NULL DEFAULT 0,
  role                 TEXT NOT NULL DEFAULT 'coder',
  account_id           TEXT,
  workspace_id         TEXT REFERENCES workspaces(id),
  CHECK (interface != 'chat' OR provider_session_id IS NULL),
  CHECK (task_id IS NOT NULL OR workspace_id IS NOT NULL)
);

INSERT INTO agents_new (
  id, task_id, host_id, parent_id, interface, provider, status, run_location,
  created_at, provider_session_id, model, effort, fast, role, account_id, workspace_id
)
SELECT
  id, task_id, host_id, parent_id, interface, provider, status, run_location,
  created_at, provider_session_id, model, effort, fast, role, account_id, NULL
FROM agents;

DROP TABLE agents;
ALTER TABLE agents_new RENAME TO agents;

CREATE INDEX idx_agents_task ON agents(task_id, created_at, id);
CREATE INDEX idx_agents_status ON agents(status);
CREATE INDEX idx_agents_workspace ON agents(workspace_id, created_at, id);

INSERT OR REPLACE INTO schema_meta(key, value) VALUES ('schema', '10');

PRAGMA foreign_keys = ON;
