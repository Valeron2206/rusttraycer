# E5 — Artifacts (v2), Ф4

Для: Core (host/protocol/storage), UI (панель + viewer + comments). Integration не в этом срезе.
От: Architect. Дата: 2026-08-19. Не код.
База: brief №1, №2, №3, №6, №11, №16; matrix C38–C42; directive E5; architecture-v0 Artifacts; ADR-0003, ADR-0004; live [Artifacts](https://docs.traycer.ai/panels/artifacts.md), [Comments](https://docs.traycer.ai/panels/comments.md) (2026-08-19).
Протокол: minor bump **1.4**. 1.0–1.3 не ломать. Конверт camelCase. Storage: миграция **0005**. 0001–0004 **байтово** не трогать.

## Закон

1. Artifact — durable сущность Task, не строка чата (brief №6). Body = **markdown TEXT в host.db**. Не md-on-disk, не Traycer live document layer.
2. Нет таблицы — нет сущности. Пока 0005 не применена, сущности нет.
3. **CASCADE на `tasks` запрещён.** Детей артефакта и comments сносит **код** host. FK без `ON DELETE CASCADE`.
4. C40 = `agent.clear_transcript`. Режет `messages`. Artifact жив. `sourceMessageId` → NULL. Не `task.delete` / `task.archive`.
5. `artifact.create` / `update` — **не** edit/exec. Лестницы нет.
6. UI ≠ Host. Три плоскости (№11). Имя **Task** (№16). Sharing (C75) / CRDT — не здесь. Artifact в E9 export (ADR-0003).
7. Review = тип + git diff. Не extension Review Mode (ADR-0004). Line comments на diff — later; comments на тексте артефакта — Ф4.
8. Нет `a2a.*`, inbox, child, loops, `caps.a2aInbox`. E6 — отдельная спека.
9. leftover `artifact.create`: настоящий метод в **TRADABLE_METHODS** `{1,4}`. Handshake-тесты «всегда `rejected.unsupported`» **переписать**.

## Решение C41 / C42 (закон, заморожено)

**C41 comments = Ф4 must.** Sqlite threads: якорь (диапазон текста) + reply + resolve. Не CRDT, не sharing. Эталон: Comments panel.

**C42 MD = Ф4 must. PDF = later** (как C37).

`artifact.export { format: "md" }` → 200 + markdown. `format: "pdf"` → `invalid_params` (не HTTP 200, не «пустой ok»). Сети нет (ADR-0008). Матрица C42 остаётся Ф4 (MD закрывает строку как must; PDF — later в этой же спеке).

**Read-state** фильтр эталона — later. Assignee = optional **free text**, без directory.

## Что есть (stub 0036, снять)

| Факт сегодня | Ф4 |
|---|---|
| Handshake `rejected.artifact.create` unsupported | в `accepted` как 1.4 |
| RPC `unsupported_method`, не в TRADABLE_METHODS | в TRADABLE_METHODS |
| Нет `artifacts` / `comments` | 0005 |
| Нет RPC clear transcript | `agent.clear_transcript` |
| GUI boards non-goal | панель Artifacts + Comments, не boards |

## Типы C38

Live 1.1.10 + architecture-v0:

| kind | status | assignee |
|---|---|---|
| `spec` | нет (`null`) | нет |
| `review` | нет | нет |
| `ticket` | `todo` \| `in_progress` \| `done`, старт `todo` | optional free text |
| `story` | то же | optional free text |

Spec/review + status/assignee → `invalid_params`.

`parentId` optional, тот же `taskId`. Цикл → `invalid_params`. Delete parent: код сносит вложенные **артефакты** (+ их threads). Агентов не трогает.

## Storage 0005

```sql
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
```

Нет `ON DELETE CASCADE`. `source_message_id` **без FK** на `messages`: `clear_transcript` делает `UPDATE … SET source_message_id = NULL`, потом `DELETE FROM messages`. `body` markdown, лимит 1 MiB. `title` 1…200. `anchor_*` — UTF-8 codepoint offsets в `body` на момент create (`anchor_end > anchor_start`, в пределах body).

Нет колонок sync / pdf / read-state / author-directory.

## Protocol 1.4

Все новые — `{major:1, minor:4}`:

```
artifact.create
artifact.get
artifact.list
artifact.update
artifact.delete
artifact.export
comment.create
comment.list
comment.resolve
agent.clear_transcript
```

Клиент без 1.4: Chat / write / pty живы. `artifact.create` не в `accepted`.

`invalid_params` | `not_found`. `clear_transcript` идемпотентен: `{ "cleared": 0 }`.

### WS (без body)

```json
{ "type": "artifact.updated", "artifactId": "…", "taskId": "…" }
{ "type": "artifact.deleted", "artifactId": "…", "taskId": "…" }
```

После create/update/delete/resolve — `updated` (delete → `deleted`, в т.ч. на каждом снесённом ребёнке). Клиент сам делает get/list.

### `Artifact`

```json
{
  "id": "…",
  "taskId": "…",
  "parentId": null,
  "kind": "spec",
  "title": "Auth",
  "body": "# Auth\n",
  "status": null,
  "assignee": null,
  "sourceMessageId": null,
  "createdAt": "…",
  "updatedAt": "…"
}
```

### `artifact.create`

```json
{
  "taskId": "…",
  "parentId": null,
  "kind": "ticket",
  "title": "Add login",
  "body": "",
  "assignee": null,
  "sourceMessageId": null
}
```

Лестницы нет. Host не парсит chat. Ok: `Artifact` (ticket/story → `todo`) + WS `artifact.updated`.

### `artifact.list` / `get`

`list { taskId, kind? }` → `{ items: [Artifact], truncated }` потолок 500. Дерево — у GUI. `get { artifactId }` → `Artifact` | `not_found`.

### `artifact.update`

`artifactId` + любое из `title` / `body` / `status` / `assignee` / `parentId`. Kind не меняется. Лестницы нет. Ok: `Artifact` + WS `updated`.

### `artifact.delete`

`{ artifactId }`. Код: дети + их threads/comments, затем сам. Агентов нет. Ok: `{ "deleted": ["…"] }` + WS `deleted` на каждый id.

### `artifact.export`

`{ artifactId, format: "md" | "pdf" }`

- `md` → `{ "format": "md", "markdown": "<title + body>", "filename": "<id>.md" }`
- `pdf` → **`invalid_params`**, не 200. Later.

### Comments

`comment.create`:

```json
{
  "artifactId": "…",
  "threadId": null,
  "anchorStart": 0,
  "anchorEnd": 12,
  "body": "nit"
}
```

`threadId` null → новый thread (якорь обязателен). `threadId` задан → reply, якорь игнор. Ok: thread целиком (см. list).

`comment.list { artifactId }` → `{ threads: [ { id, artifactId, anchorStart, anchorEnd, resolved, comments: [{ id, body, createdAt }], createdAt, updatedAt } ] }`

`comment.resolve { threadId }` → thread `resolved=true` + WS `artifact.updated`. Повтор идемпотентен.

Нет `comment.delete` в Ф4. Нет sharing.

### `agent.clear_transcript` (C40)

`{ agentId }` → `{ "cleared": 12 }`

1. `UPDATE artifacts SET source_message_id = NULL WHERE source_message_id IN (SELECT id FROM messages WHERE agent_id = ?)`
2. `DELETE FROM messages WHERE agent_id = ?`

Не трогать: agent row, policy, worktree, `providerSessionId`, artifact row/body/comments. Не `task.delete`.

## GUI Ф4

- Панель Artifacts: дерево, фильтр kind, create/rename/status/delete, markdown viewer.
- Comments: контекстная панель активного артефакта — select text → thread, reply, resolve ([comments.md](https://docs.traycer.ai/panels/comments.md)).
- Export: кнопка Markdown. PDF нет (не показывать как рабочий путь).
- Нет read-state, boards, C21 search, `@`, artifacts-in-PTY, sharing.
- GUI не спавнит host.

## Вне скоупа

- PDF (C42 later)
- Read-state, line comments на git diff
- C21, C43–C47 A2A, C75, C57
- md-on-disk, CRDT, live document layer
- directory/assignee picker

## Приёмка Ф4

1. `artifact.create` в TRADABLE_METHODS и `accepted` 1.4. Тесты handshake больше **не** ждут `rejected.unsupported`.
2. Create spec + child ticket → list/get. Update status ticket. Delete parent сносит детей, агент жив.
3. Comment: якорь + reply + resolve. После рестарта host threads на месте.
4. C40: messages есть, artifact со `sourceMessageId`. `agent.clear_transcript` → `cleared>0`, messages 0, artifact.get тот же body, `sourceMessageId=null`. Рестарт — artifact жив.
5. `export format=md` → 200 markdown. `format=pdf` → `invalid_params`, не 200.
6. create/update без `agent.approval`.
7. Клиент без 1.4: send/git/pty живы; artifact.* не приняты.
8. 0005: нет `ON DELETE CASCADE` на tasks. Body только в sqlite.

Код — следующие STAR (Core, UI). E6 не открывать из этого файла.
