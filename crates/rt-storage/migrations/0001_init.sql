-- 0001_init.sql

CREATE TABLE host (
  id         TEXT PRIMARY KEY,          -- uuid v7
  name       TEXT NOT NULL,
  created_at TEXT NOT NULL              -- RFC3339 UTC
);

CREATE TABLE workspaces (
  id         TEXT PRIMARY KEY,
  host_id    TEXT NOT NULL REFERENCES host(id),
  path       TEXT NOT NULL,             -- абсолютный canonicalize
  name       TEXT NOT NULL,             -- basename в момент add
  created_at TEXT NOT NULL,
  UNIQUE (path)
);

CREATE TABLE tasks (
  id         TEXT PRIMARY KEY,
  title      TEXT NOT NULL,             -- 1..200, проверяет слой выше
  status     TEXT NOT NULL CHECK (status IN ('open', 'archived')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE task_workspaces (
  task_id      TEXT NOT NULL REFERENCES tasks(id),
  workspace_id TEXT NOT NULL REFERENCES workspaces(id),
  PRIMARY KEY (task_id, workspace_id)
);

CREATE TABLE agents (
  id           TEXT PRIMARY KEY,
  task_id      TEXT NOT NULL REFERENCES tasks(id),
  host_id      TEXT NOT NULL REFERENCES host(id),
  parent_id    TEXT REFERENCES agents(id),   -- MVP: всегда NULL
  interface    TEXT NOT NULL CHECK (interface IN ('chat')),
  provider     TEXT NOT NULL,                -- HarnessId, MVP: 'cli.generic'
  status       TEXT NOT NULL CHECK (status IN ('idle', 'running', 'error')),
  run_location TEXT NOT NULL CHECK (run_location IN ('local')),
  created_at   TEXT NOT NULL
);

CREATE TABLE messages (
  id         TEXT PRIMARY KEY,
  agent_id   TEXT NOT NULL REFERENCES agents(id),
  role       TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'system', 'tool')),
  content    TEXT NOT NULL,             -- лимит 1 MiB проверяет слой выше
  created_at TEXT NOT NULL
);

CREATE TABLE schema_meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE INDEX idx_workspaces_host ON workspaces(host_id);
CREATE INDEX idx_tasks_status_updated ON tasks(status, updated_at DESC, id DESC);
CREATE INDEX idx_task_workspaces_ws ON task_workspaces(workspace_id);
CREATE INDEX idx_agents_task ON agents(task_id, created_at, id);
CREATE INDEX idx_agents_status ON agents(status);
CREATE INDEX idx_messages_agent ON messages(agent_id, created_at, id);

INSERT INTO schema_meta(key, value) VALUES ('schema', '1');
