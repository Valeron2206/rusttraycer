-- 0006_loops.sql
-- Bounded A2A loops. FK without cascade.

CREATE TABLE loops (
  id              TEXT PRIMARY KEY,
  task_id         TEXT NOT NULL REFERENCES tasks(id),
  agent_a         TEXT NOT NULL REFERENCES agents(id),
  agent_b         TEXT NOT NULL REFERENCES agents(id),
  max_iterations  INTEGER NOT NULL,
  budget_turns    INTEGER NOT NULL,
  iteration       INTEGER NOT NULL DEFAULT 0,
  turns           INTEGER NOT NULL DEFAULT 0,
  status          TEXT NOT NULL CHECK (status IN ('running', 'stopped')),
  reason          TEXT,
  prompt          TEXT NOT NULL,
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL,
  CHECK (max_iterations BETWEEN 1 AND 32),
  CHECK (budget_turns BETWEEN 1 AND 64)
);

INSERT OR REPLACE INTO schema_meta(key, value) VALUES ('schema', '6');
