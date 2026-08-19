-- 0003_policies.sql
-- Agent/workspace permission ladder. Default is ask (C26: not full-access).
-- Exactly one of workspace_id / agent_id is set.

CREATE TABLE policies (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NULL REFERENCES workspaces(id),
  agent_id TEXT NULL REFERENCES agents(id),
  mode TEXT NOT NULL CHECK (mode IN ('ask', 'allow-always', 'deny')),
  scope TEXT NOT NULL CHECK (scope IN ('agent', 'workspace')),
  yolo INTEGER NOT NULL CHECK (yolo IN (0, 1)),
  updated_at TEXT NOT NULL,
  CHECK (
    (workspace_id IS NOT NULL AND agent_id IS NULL)
    OR (workspace_id IS NULL AND agent_id IS NOT NULL)
  )
);

CREATE UNIQUE INDEX policies_agent_id_unique ON policies(agent_id) WHERE agent_id IS NOT NULL;
CREATE UNIQUE INDEX policies_workspace_id_unique ON policies(workspace_id) WHERE workspace_id IS NOT NULL;

INSERT OR REPLACE INTO schema_meta(key, value) VALUES ('schema', '3');
