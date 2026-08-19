-- 0004_terminal.sql
-- Expand agents.interface CHECK to 'chat' | 'terminal' and add provider_session_id.
-- SQLite cannot ALTER CHECK: rebuild agents (FK off).
-- No shells / pty_sessions table. Live PTY is process memory only.

PRAGMA foreign_keys = OFF;

CREATE TABLE agents_new (
  id                   TEXT PRIMARY KEY,
  task_id              TEXT NOT NULL REFERENCES tasks(id),
  host_id              TEXT NOT NULL REFERENCES host(id),
  parent_id            TEXT REFERENCES agents_new(id),
  interface            TEXT NOT NULL CHECK (interface IN ('chat', 'terminal')),
  provider             TEXT NOT NULL,
  status               TEXT NOT NULL CHECK (status IN ('idle', 'running', 'error')),
  run_location         TEXT NOT NULL CHECK (run_location IN ('local', 'worktree')),
  created_at           TEXT NOT NULL,
  provider_session_id  TEXT NULL,
  CHECK (interface != 'chat' OR provider_session_id IS NULL)
);

INSERT INTO agents_new (
  id, task_id, host_id, parent_id, interface, provider, status, run_location, created_at, provider_session_id
)
SELECT
  id, task_id, host_id, parent_id, interface, provider, status, run_location, created_at, NULL
FROM agents;

DROP TABLE agents;
ALTER TABLE agents_new RENAME TO agents;

CREATE INDEX idx_agents_task ON agents(task_id, created_at, id);
CREATE INDEX idx_agents_status ON agents(status);

INSERT OR REPLACE INTO schema_meta(key, value) VALUES ('schema', '4');

PRAGMA foreign_keys = ON;
