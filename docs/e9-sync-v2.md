# E9 — Sync (v2), Ф6

Для: Core (export/import RPC). UI (файл ↔ JSON). Integration не трогать.
От: Architect. Дата: 2026-08-19. Не код.
База: ADR-0003; brief №2, №3, №12; directive E9; matrix C57–C58.
Протокол: minor bump **1.8**. 1.0–1.7 не ломать. Конверт camelCase. **Новой миграции нет.** 0001–0008 **байтово**.

## Закон ADR-0003

1. **Минимум E9:** export / import durable-сущностей. Clone-not-migrate: копия — данные **нового** host; **оба `hostId` каноничны**. Source host row не переписывать. Dest `host.id` не менять.
2. **Цель того же эпика, не блокер:** self-hosted `rt-sync`. Не Traycer cloud. C58 = **later**.
3. **Никогда не синкать:** PTY, worktree directories, terminal scrollback, in-flight turns, `provider_session_id`.
4. **Никогда в v2:** live collab, Yjs/CRDT, managed cloud / org SSO / seat billing. Предложить managed sync = эскалация PO.
5. Секреты не в архиве и не в host.db (ADR-0005 / C74). Нет PAT/token/keyring dump.
6. E10 (metrics, packaging, platforms, новый CLI surface) **не открывать**. Лестницу / A2A / artifacts / E7 / E8 RPC не менять.

## Решение C57–C58 (закон)

| ID | Ф6 |
|---|---|
| C57 export/import durable | **must** |
| C58 self-hosted `rt-sync` | **later** (тот же archive по HTTP, не cloud) |
| Full-host dump / device-switch SaaS | **oos** (ADR-0003) |

Матрица: C57 Ф6, C58 later.

## Что есть

- Один host, один `host.db`. RPC переноса нет.
- Durable в sqlite: Task (preset), Agent (role, model, effort, fast, parent_id, provider_session_id, run_location), Message, Artifact + threads/comments, model_profiles, harness_prefs, loops, policies, worktrees, workspaces.
- Live: PTY в памяти (E4). Worktree — путь на диске. Session id вендора.
- `hostId` каноничен с 0001. Clone нет.

## Что в архиве

Формат: JSON, `kind: "rusttraycer.export"`, `exportVersion: 1`. Не sqlite dump. Не CASCADE.

**Входит** (id как на source — clone):

- `tasks` (+ `preset`)
- `agents` (+ `role`, `model`, `effort`, `fast`, `parentId`, `interface`, `provider`, `createdAt`)
- `messages`
- `artifacts` + `commentThreads` + `comments`
- `modelProfiles` (host-level sidecar)

**Не входит:**

- `host` row
- `workspaces` / абсолютные path
- `worktrees` / worktree path
- `providerSessionId` (всегда strip)
- `loops`, `policies`
- `harness_prefs` (dest помнит свои last model)
- global / workspace markdown guides (файлы, не sqlite; sync гайдов later)
- pid.json, env, keyring, git creds

**Нормализация при export:**

- `agent.status` → `idle` (даже если source running)
- `runLocation` → `local`
- `sourceMessageId` артефакта: оставить, если message в архиве, иначе JSON `null`

Пустой `taskIds` → `invalid_params`. Не full-db. Потолок **32** task за вызов.

## Protocol 1.8

```
sync.export   1.8
sync.import   1.8
```

Клиент без 1.8: 1.7 guides / 1.6 switch / 1.5 a2a живы. `sync.*` не в `accepted`.

### `sync.export`

```json
{ "taskIds": ["…"] }
```

ok: `{ "archive": { … } }` — весь JSON в ответе. Host **не** пишет файл на диск (GUI/caller сохраняет).

Нет task → `not_found`. Больше 32 / пусто / дубли → `invalid_params`.

Архив:

```json
{
  "kind": "rusttraycer.export",
  "exportVersion": 1,
  "sourceHostId": "…",
  "exportedAt": "2026-08-19T12:00:00Z",
  "tasks": [ ],
  "agents": [ ],
  "messages": [ ],
  "artifacts": [ ],
  "commentThreads": [ ],
  "comments": [ ],
  "modelProfiles": [ ]
}
```

`sourceHostId` = канонический id **этого** host. Не поле для импорта в `host` таблицу dest.

### `sync.import`

```json
{
  "workspaceId": "…",
  "archive": { }
}
```

Правила:

- `kind` / `exportVersion` не те → `invalid_params`.
- `workspaceId` должен быть workspace **dest** host. Path source не использовать. `not_found` если нет.
- Транзакция: всё или ничего. Нет CASCADE delete.
- Id сущностей **те же**. Если любой id уже есть на dest → `conflict`, rollback, ничего не вставлено.
- `agents.hostId` на dest = dest `host.id`. Source `hostId` не писать в dest `host`.
- `task_workspaces`: все импортированные task → только этот `workspaceId`.
- `parentId` агента: оставить, если parent в архиве, иначе `null`.
- Профили: INSERT если `name` свободен; занятое имя — skip (не error). Новые profile id из архива; коллизия id профиля → `conflict` как у task.
- Секретов в архиве быть не должно; host не читает env в import.

ok:

```json
{
  "tasks": 1,
  "agents": 2,
  "messages": 10,
  "artifacts": 1,
  "profilesImported": 0,
  "profilesSkipped": 1
}
```

После успеха GUI делает обычный `task.get` / `agent.list` / `get_context`. WS не обязателен.

Импорт на тот же host (source == dest) с теми же id → `conflict` (это не migrate in place).

## Storage

Миграции нет. 0001–0008 не трогать. Import идёт существующими INSERT. FK без CASCADE — как сейчас: нет task → нельзя agent; нет artifact → нельзя thread.

## GUI минимум

- На Task: Export → сохранить JSON.
- Import: выбрать файл + текущий workspace. Показать counts. `conflict` — сообщение, дерево не менять.
- Нет cloud login. Нет поля token. Нет device-switch SaaS.

## Вне скоупа

- C58 `rt-sync`
- E10 metrics / AppImage / macOS / новый CLI
- CRDT, live comments sync, managed cloud
- PTY / worktree / scrollback / session id
- Full-host export, sync гайдов
- Секреты в архиве

## Приёмка Ф6 / E9 (C57)

1. Host A: Task + два агента + messages + artifact+comment → export → import на Host B в его workspace. Id те же. `B.agents.hostId == B.host.id`. `A.host.id` не изменился.
2. В архиве нет path worktree, нет `providerSessionId`, нет `host` row.
3. Повторный import того же архива на B → `conflict`, counts не выросли.
4. Import без `workspaceId` / чужой workspace → `invalid_params` / `not_found`.
5. Клиент без 1.8: create / send / guides живы.
6. 0001–0008 байтово целы.

Код — следующие STAR. `rt-sync` и E10 не открывать из этого файла.
