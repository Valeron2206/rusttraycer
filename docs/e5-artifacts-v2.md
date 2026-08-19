# E5 — Artifacts (v2), Ф4

Для: Core (host/protocol/storage), UI (панель + viewer). Integration не в этом срезе.
От: Architect. Дата: 2026-08-19. Не код.
База: brief №1, №2, №3, №6, №11, №16; matrix C38–C42; directive E5; architecture-v0 Artifacts; ADR-0003, ADR-0004; live [Artifacts](https://docs.traycer.ai/panels/artifacts.md) (2026-08-19).
Протокол: minor bump **1.4**. 1.0–1.3 не ломать. Конверт camelCase. Storage: миграция **0005**. 0001–0004 **байтово** не трогать.

## Закон

1. Artifact — durable сущность Task, не строка чата (brief №6). Источник правды — sqlite host, не файл в git и не `messages`.
2. Нет таблицы — нет сущности. Пока 0005 не применена, `artifact.create` остаётся stub (0036).
3. **CASCADE в SQL запрещён.** Снос детей / transcript — в коде host. FK без `ON DELETE CASCADE`.
4. C40 = удалить **транскрипт агента** (`messages`), не Task и не Agent row. Artifact жив.
5. UI ≠ Host: GUI не пишет sqlite. Три плоскости версий (№11). Имя **Task**, не epic (№16).
6. Artifact едет в E9 export (ADR-0003), не CRDT, не live collab. Sharing (C75) — не здесь.
7. Review = тип артефакта + существующий git diff. Не extension Review Mode (ADR-0004). Line comments — с C41 later.
8. В этом файле нет `a2a.*`, inbox, child, loops, `caps.a2aInbox`. E6 — отдельная спека.

## Решение C41 / C42 (закон)

**C41 comments — later. Viewer — Ф4 must.**

Строка матрицы C41 смешивает viewer и comments. Ф4: дерево + markdown viewer + create. Anchored comment threads (эталон) — later, не блокер мастер-e2e. Таблицы `comments` нет.

**C42 export MD/PDF — later, не Ф4.**

Не блокер e2e (create → delete transcript → artifact жив). PDF = лишняя зависимость. Сети нет (ADR-0008). Локальный Save As — когда дойдём, без outbound. Матрица: wave **later**.

## Что есть (stub, закон 0036)

| Факт | Где |
|---|---|
| Handshake `rejected.artifact.create.reason = unsupported` | architecture-v0, handshake test |
| RPC `unsupported_method` | host, не в TRADABLE_METHODS |
| Таблиц `artifacts` / `comments` нет | migrations 0001–0004 |
| `messages` append-only, RPC delete transcript нет | storage |
| GUI boards — non-goal v0 | gui-ia-v0 |

Ф4 снимает stub: метод в TRADABLE_METHODS, `{major:1, minor:4}`. Клиент, который шлёт `artifact.create` без 1.4 — как раньше, не в `accepted` / `unsupported_method`.

## Типы C38

Как эталон и architecture-v0:

| kind | Роль | status | assignee |
|---|---|---|---|
| `spec` | цели, ограничения, решения | нет (`null`) | нет |
| `ticket` | работа, что менять, как проверить | `todo` / `in_progress` / `done` | optional string |
| `story` | пользовательский срез, может крыть несколько ticket | то же | optional string |
| `review` | находки, критика, follow-up | нет | нет |

Новый ticket/story: `status=todo`. Spec/review с ненулевым status/assignee → `invalid_params`.

Иерархия: `parentId` optional, тот же `taskId`. Типично Spec → Ticket/Story/Review. Host не навязывает схему дерева. Цикл parent → `invalid_params`.

Удаление родителя: host **в коде** удаляет вложенные артефакты. Агентов не трогает (в Ф4 агент к артефакту не привязан; когда появятся — «поднимаются», не удаляются).

## Storage 0005

```sql
CREATE TABLE artifacts (
  id          TEXT PRIMARY KEY,
  task_id     TEXT NOT NULL REFERENCES tasks(id),
  parent_id   TEXT REFERENCES artifacts(id),
  kind        TEXT NOT NULL CHECK (kind IN ('spec', 'ticket', 'story', 'review')),
  title       TEXT NOT NULL,
  body        TEXT NOT NULL,
  status      TEXT,
  assignee    TEXT,
  source_agent_id TEXT,
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL,
  CHECK (
    (kind IN ('spec', 'review') AND status IS NULL AND assignee IS NULL)
    OR (kind IN ('ticket', 'story') AND status IN ('todo', 'in_progress', 'done'))
  )
);
CREATE INDEX idx_artifacts_task ON artifacts(task_id);
```

Никакого `ON DELETE CASCADE`. `source_agent_id` **без FK** на `agents`: транскрипт/агент можно снести, id остаётся историей (из turn). `body` — markdown, лимит как `Message.content` (1 MiB). `title` 1…200.

Нет колонок sync / comments / pdf.

## Protocol 1.4

Новые методы, все `{major:1, minor:4}`:

```
artifact.create            1.4
artifact.list              1.4
artifact.get               1.4
artifact.update            1.4
artifact.delete            1.4
agent.transcript.delete    1.4
```

`artifact.update` / `delete` нужны: status ticket/story и «удаление родителя сносит детей» — закон architecture, не later. GUI Ф4 без них врёт.

Клиент без 1.4: Chat / write / pty живы. `artifact.create` не в `accepted`.

Новые коды: нет обязательных. `not_found`, `invalid_params`. Transcript delete идемпотентен: повтор → `ok { deleted: 0 }`.

WS не обязателен. После mutate GUI перечитывает `artifact.list`.

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
  "sourceAgentId": null,
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
  "sourceAgentId": null
}
```

`sourceAgentId` optional: «из turn», тот же host/task. Host **не** парсит markdown из chat. Create — единственная запись (GUI или будущий adapter через тот же RPC). Ok: `Artifact` (ticket/story → `status=todo`).

### `artifact.list` / `get`

`list { taskId, kind? }` → `{ items: [Artifact] }` (плоский список + `parentId`, дерево собирает GUI). Потолок 500, `truncated`. `get { artifactId }` → `Artifact` | `not_found`.

### `artifact.update`

Params: `artifactId` + любое из `title` / `body` / `status` / `assignee` / `parentId`. Kind не меняется. Status/assignee на spec/review → `invalid_params`. Ok: `Artifact`.

### `artifact.delete`

`{ artifactId }`. Рекурсивно дети (в коде). Агентов нет. Ok: `{ "deleted": ["id", …] }`.

### `agent.transcript.delete` (C40)

`{ agentId }`. `DELETE FROM messages WHERE agent_id = ?`. Agent row, policy, worktree, `providerSessionId`, **artifacts** — не трогать. Ok: `{ "agentId": "…", "deleted": 12 }`.

Это не `task.archive`. Не `agent` delete.

## GUI Ф4

- Панель **Artifacts** у выбранного Task: дерево по `parentId`, фильтр kind.
- Viewer: title + markdown body (read). Ticket/story: смена status. Create spec/ticket/story/review. Rename / delete (с детьми).
- Нет comments, нет export MD/PDF, нет boards, нет search (C21), нет `@` mention, нет artifacts-in-PTY.
- GUI не спавнит host.

## Вне скоупа Ф4 / E5

- C41 comments / line comments на diff
- C42 export MD/PDF
- C21 search
- C43–C47 A2A (отдельная спека)
- C75 sharing / C57 E9 export-import
- Диск `.md` как source of truth, CRDT
- drop `@`, artifacts в PTY (e4 reserved)

## Приёмка Ф4

1. После 0005: `artifact.create` в `accepted` как 1.4, не `rejected.unsupported`.
2. Create spec + ticket (child) → `list` видит оба. `get` отдаёт body.
3. C40: есть messages у агента + artifact с `sourceAgentId`. `agent.transcript.delete` → messages 0, `artifact.get` тот же id/body. Рестарт host — artifact на месте.
4. `artifact.delete` родителя сносит детей; агент Task жив.
5. Клиент без 1.4: send/git/pty живы; `artifact.create` не принят.
6. sqlite: нет `ON DELETE CASCADE` в 0005. Нет таблицы comments.
7. C42 в продукте нет.

Код — следующие STAR (Core storage/RPC, UI панель). E6 не открывать из этого файла.
