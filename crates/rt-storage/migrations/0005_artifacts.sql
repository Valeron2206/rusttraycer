-- 0005_artifacts.sql
-- Artifacts + comment threads. Body is markdown TEXT in host.db.
-- FK without cascade. source_message_id has no FK on messages.

CREATE TABLE artifacts (
  id               TEXT PRIMARY KEY,
  task_id          TEXT NOT NULL REFERENCES tasks(id),
  parent_id        TEXT REFERENCES artifacts(id),
  kind             TEXT NOT NULL CHECK (kind IN ('spec', 'ticket', 'story', 'review')),
  title            TEXT NOT NULL,
  body             TEXT NOT NULL,
  status           TEXT,
  assignee         TEXT,
  source_message_id TEXT,
  created_at       TEXT NOT NULL,
  updated_at       TEXT NOT NULL,
  CHECK (
    (kind IN ('spec', 'review') AND status IS NULL AND assignee IS NULL)
    OR (kind IN ('ticket', 'story') AND status IN ('todo', 'in_progress', 'done'))
  )
);
CREATE INDEX idx_artifacts_task ON artifacts(task_id);

CREATE TABLE comment_threads (
  id           TEXT PRIMARY KEY,
  artifact_id  TEXT NOT NULL REFERENCES artifacts(id),
  anchor_start INTEGER NOT NULL,
  anchor_end   INTEGER NOT NULL,
  resolved     INTEGER NOT NULL DEFAULT 0,
  created_at   TEXT NOT NULL,
  updated_at   TEXT NOT NULL
);

CREATE TABLE comments (
  id         TEXT PRIMARY KEY,
  thread_id  TEXT NOT NULL REFERENCES comment_threads(id),
  body       TEXT NOT NULL,
  created_at TEXT NOT NULL
);

INSERT OR REPLACE INTO schema_meta(key, value) VALUES ('schema', '5');
