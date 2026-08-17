# Storage v0 — `host.db`

Для: Core (`rt-storage`).
От: Architect. Дата: 2026-08-17.
Статус: действующий контракт MVP. Не код.

Согласование:
- швы Store и инварианты — `host-runtime-v0.md` §3
- имена на проводе — `protocol-v0.md` (camelCase). В БД — snake_case
- файлы workspace в БД не хранятся (`files.*` читает FS)
- wire `cli.generic` — `runtime-adapters-v0.md`, не этот файл

---

## 0. Границы

Пишет только процесс host. Один писатель.
`rt-gui` / `rt-cli` / `rt-runtime` БД не открывают.

Путь: `$RUSTTRAYCER_HOME/host/host.db`, иначе `~/.rusttraycer/host/host.db`.

Не храним:
- содержимое файлов workspace, дерево, mtime (это FS)
- PTY / scrollback / mux
- worktree
- artifacts / comments / A2A
- sessionToken (память процесса)
- per-record `{major,minor}`

---

## 1. Соединение и pragmas

При `Store::open`:

```
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
PRAGMA synchronous = NORMAL;
```

MVP: `Arc<Mutex<Connection>>`. Не пул писателей.

После open — миграции, затем recovery (§6), затем можно отдавать Store.

`integrity_check` для `host.doctor.dbOk`: достаточно успешного open + `PRAGMA quick_check` (или просто «соединение живо»). Полный `integrity_check` не на каждый doctor.

---

## 2. Версия схемы

Одна глобальная версия. Не per-record.

Таблица `schema_meta(key TEXT PRIMARY KEY, value TEXT NOT NULL)`.
После 0001: `('schema', '1')`.

`Store::migrate` применяет ещё не применённые SQL-файлы по номеру.
Неизвестный `schema > 1` на этом бинаре → отказ старта (не молча).
`schema` отсутствует → пустая БД, гони 0001.

Persistence-плоскость `{major,minor}` из ADR-0002 здесь не появляется. Когда понадобится sync — отдельный ADR и major миграции.

---

## 3. Миграция 0001

Имена таблиц во множественном числе, кроме `host` (один ряд) и `schema_meta`.

```sql
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
```

ON DELETE не ставим CASCADE. RPC удаления Task/Workspace/Agent в MVP нет. Случайный delete из оболочки не должен сносить дерево.

`parent_id` FK на `agents(id)` без ON DELETE SET NULL: parent не удаляем. Значение в MVP всегда NULL.

Нет UNIQUE на `(task_id)` у agents: протокол позволяет несколько агентов; UI MVP создаёт одного.

---

## 4. Инварианты строк

1. В `host` ровно один ряд. `id` = `hostId` в pid.json. `Store::host_insert_if_absent` не переписывает существующий id.
2. `workspaces.path` уникален после canonicalize. Повторный add того же пути — вернуть существующий ряд, новый id не создавать.
3. MVP: у каждого Task ровно одна строка в `task_workspaces`. Проверяет `task_create`, не CHECK (потом будет >1).
4. `agents.host_id` всегда равен единственному `host.id`. Не migrate.
5. `agents.interface` = `chat`, `run_location` = `local`, `parent_id` IS NULL. Другое — баг писателя, не миграции.
6. `agents.provider` = HarnessId. В MVP приложение пишет только `cli.generic`. CHECK на одно значение не ставим: второй harness не должен требовать миграцию только ради строки.
7. `messages` append-only. Нет UPDATE content. Чанк assistant = новая строка.
8. Архив Task не меняет agents/messages и не стопает turn.

Типы: все id и timestamps — TEXT. Не INTEGER unix. Не BLOB uuid.

---

## 5. Store API ↔ SQL

Семантика как host-runtime §3. Здесь — ожидания по запросам и ошибкам.

| Метод | Поведение |
|---|---|
| `host_get` | единственный ряд. Нет ряда до insert_if_absent — баг порядка старта |
| `host_insert_if_absent(id, name)` | INSERT OR IGNORE. name при ignore не обновлять |
| `workspace_list` | ORDER BY created_at ASC, id ASC |
| `workspace_add(path, name)` | path уже канонический (канонизирует host до Store). UNIQUE conflict → вернуть существующий. FS-проверку Store не делает |
| `workspace_get` | по id |
| `task_list(Open\|Archived\|All)` | ORDER BY updated_at DESC, id DESC |
| `task_create(title, workspace_id)` | транзакция: INSERT tasks + INSERT task_workspaces. Нет workspace → ошибка наружу как not_found |
| `task_get` | + собрать `workspaceIds` из task_workspaces |
| `task_rename` | UPDATE title, updated_at = now. Даже если title тот же |
| `task_archive` | если уже archived — no-op (updated_at можно не трогать) |
| `task_touch` | только updated_at = now (после agent.send) |
| `agent_list(task_id)` | ORDER BY created_at ASC, id ASC. Нет task → not_found на уровне service |
| `agent_create` | interface=chat, parent_id=NULL, run_location=local, status=idle, host_id=host.id |
| `agent_get` | для RPC extra: `lastMessageAt` = MAX(messages.created_at) или NULL |
| `agent_set_status` | UPDATE status. Невалидный переход Store не валидирует (это supervisor) |
| `message_append` | INSERT. ORDER KEY = (created_at, id) |
| `message_list` | ORDER BY created_at ASC, id ASC. Полный transcript, без LIMIT в MVP |

Транзакции: `task_create` и «send: append user + set running» должны быть атомарны на уровне service (один lock Mutex это даёт, если обе записи под одним guard).

`now` для timestamps: UTC, RFC3339 с `Z`. Одна функция часов на Store.

---

## 6. Recovery при старте

После migrate, до listen:

```
UPDATE agents SET status = 'error' WHERE status = 'running';
```

Недописанный turn: частичные assistant Message уже в таблице, их не удаляем и не склеиваем. Это инвариант host-runtime §1.5.

Иначе агент навсегда `running` и все `agent.send` → `agent_busy`.

---

## 7. Что не добавлять в 0001

- `files`, `file_cache`, `blobs`
- `worktrees`, `terminals`, `pty_sessions`
- `artifacts`, `comments`, `a2a_*`
- `sync_rev`, `tombstone`, `yjs`
- `schema_major` / `schema_minor` на сущностях
- UNIQUE(task_id) на agents
- CHECK(provider = 'cli.generic')

`files.tree` / `files.read` ходят в FS по `workspaces.path`. Кэш не нужен.

---

## 8. Ошибки слоя

Store не знает HTTP-коды. Мапит так:

| Ситуация | Наверх |
|---|---|
| FK miss / нет ряда | not_found (или Option) |
| UNIQUE path при add | существующий Workspace, не ошибка |
| CHECK constraint | internal (баг писателя) |
| SQLITE_BUSY после timeout | internal |
| диск / corrupt | internal, doctor.dbOk=false |

Лимиты title/content проверяет service/RPC (`invalid_params`), не SQL CHECK.

---

## 9. Definition of done

1. Файл этого контракта.
2. В репо (когда будет): `0001_init.sql` один-в-один с §3.
3. Тесты: host id стабилен; workspace add идемпотентен по path; task_create без workspace падает; message_list порядок; recovery running→error; FK не даёт agent без task.
4. Нет таблицы files. Нет per-record version.

---

## 10. Открыто

Ничего, что блокирует MVP.

Не открыто: форма timestamps, uuid TEXT, WAL, один host row, recovery running→error.

Если понадобится delete Task — отдельная миграция и ADR (сейчас RESTRICT). Не делать «на всякий случай» CASCADE.
