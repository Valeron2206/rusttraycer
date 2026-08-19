> **v1.0 reader:** this file is the original draft. Added 1.0 methods: agent.cancel, worktree.*, git.status/diff. Envelope unchanged. See [v1-delta.md](v1-delta.md).

# Protocol v0 — client ↔ host

Статус: действующий контракт на провод. 2026-08-17.
Для: Core (`rt-host`) и UI (`rt-gui`). CLI ходит сюда же (`host.ping`, `host.doctor`, `handshake`).

Это источник правды для wire. Не код. Не storage.

Согласование с уже принятыми спеками:

- инварианты, процессы, сущности — `architecture-v0.md`
- жизненный цикл host, supervisor, `cli.generic` — `host-runtime-v0.md`
- при расхождении имён полей на проводе побеждает **этот** файл (везде camelCase)
- черновик handshake в architecture-v0 (§5, snake_case) — снят
- `storage-v0.md` ещё не написан; persistence-плоскость здесь не специфицируется

---

## 1. Transport

Один listener: `127.0.0.1:<ephemeral>`. Non-loopback — не биндить, remote — reject.
Один порт на HTTP и WebSocket.

| Метод | Путь | Роль |
|---|---|---|
| `POST` | `/rpc` | request/response, JSON-RPC-подобный конверт |
| `GET` | `/health` | liveness, без handshake и без сессии |
| `GET` | `/ws` | события, WebSocket upgrade |

Другие пути — HTTP 404. Тело RPC и ответы — `Content-Type: application/json`, UTF-8.

### Discovery

Клиент **не** сканирует порты. Читает pid-файл, который host пишет атомарно (temp + rename) при старте:

путь: `$RUSTTRAYCER_HOME/host/pid.json`, иначе `~/.rusttraycer/host/pid.json`

```json
{
  "hostId": "0191f0c6-7c2a-7c11-8000-6f0c1a2b3c4d",
  "pid": 12345,
  "rpcUrl": "http://127.0.0.1:47800",
  "wsUrl": "ws://127.0.0.1:47800/ws",
  "startedAt": "2026-08-17T10:50:00Z",
  "protocol": { "crate": "0.1.0" }
}
```

| Поле | Тип | Смысл |
|---|---|---|
| `hostId` | string, uuid v7 | стабилен между рестартами host, тот же id что в БД |
| `pid` | number (int) | pid процесса host |
| `rpcUrl` | string | origin + схема http, без path |
| `wsUrl` | string | полный URL `ws://127.0.0.1:<port>/ws` |
| `startedAt` | string, RFC3339 UTC | момент записи файла |
| `protocol.crate` | string, semver | semver крейта/бинаря, **не** RPC-плоскость |

Нет файла / pid мёртв — host не запущен. Второй живой процесс не стартует (exit 2, код `already_running`). Это не RPC.

### Конверт `POST /rpc`

Request:

```json
{
  "id": "<client-req-uuid>",
  "method": "task.create",
  "params": {}
}
```

| Поле | Тип | Правило |
|---|---|---|
| `id` | string | генерит клиент, echo в ответ. Рекомендуется uuid v7. Непустой, ≤ 128 символов |
| `method` | string | имя из §5 |
| `params` | object | объект, не null. Нет параметров — `{}` |

Success (HTTP 200):

```json
{
  "id": "<тот же id>",
  "ok": {}
}
```

Error (HTTP 200, если конверт разобран):

```json
{
  "id": "<тот же id>",
  "error": {
    "code": "agent_busy",
    "message": "agent has an in-flight turn"
  }
}
```

`ok` и `error` взаимоисключающи. Тело без `id` или не JSON — HTTP 400, без обязательства echo.

`GET /health` — **не** конверт:

```json
{
  "ok": true,
  "hostId": "0191f0c6-7c2a-7c11-8000-6f0c1a2b3c4d"
}
```

HTTP 200. Handshake не нужен.

### Сессия

После успешного `handshake` host выдаёт `sessionToken` (uuid v7, живёт в памяти процесса, умирает вместе с host).

Все последующие `POST /rpc` и upgrade `GET /ws` несут заголовок:

```
X-Rt-Session: <sessionToken>
```

Без заголовка / неизвестный токен → `unauthorized` (для `/ws` — закрыть upgrade, HTTP 401).

**Без сессии** разрешены только:

- `handshake`
- `host.ping`
- `GET /health`

Новый `handshake` выдаёт новый токен. Старые токены этого процесса остаются валидны до смерти host. Рестарт host инвалидирует все токены — клиент делает hello заново.

### Стабильные коды ошибок

Строки, не числа. Новый код = minor++ соответствующего метода или новый метод.

| Код | Когда |
|---|---|
| `not_found` | сущность с таким id нет |
| `invalid_params` | тип/форма/лимит/путь вне workspace/`..` |
| `agent_busy` | `agent.send` при `status=running` или есть inflight turn |
| `workspace_path_invalid` | path не существует, не директория, `canonicalize` не удался |
| `unsupported_method` | host не знает такое имя метода |
| `version_mismatch` | метод есть, но не принят правилом версий / не в `accepted` этой сессии |
| `unauthorized` | нет или неизвестен `X-Rt-Session` |
| `internal` | непредвиденный сбой host |
| `already_running` | только процесс (второй host, exit 2). Из `/rpc` в MVP не возвращается |
| `file_too_large` | `files.read`, файл > 256 KiB |
| `file_binary` | `files.read`, не UTF-8 текст |

`message` — человекочитаемый, не контракт. Клиент ветвится по `code`.

---

## 2. Versioning

Три независимые плоскости (ADR-0002). Не смешивать.

| Плоскость | Где живёт | MVP |
|---|---|---|
| semver крейта | `protocol.crate` в pid.json, `clientVersion` / `hostVersion` в handshake | `0.1.0` |
| per-method RPC `{major,minor}` | handshake `methods` / `accepted` / `rejected` | все методы `1.0` |
| persistence `{major,minor}` | `storage-v0` (ещё нет). Сейчас один глобальный schema version в миграциях | вне этого документа |

Handshake торгует **каждый метод отдельно**. Правило совместимости:

1. `major` клиента == `major` host
2. `client.minor <= host.minor`

Иначе метод попадает в `rejected`, не в `accepted`.

- новое optional-поле на существующем методе = `minor++` host; старый клиент совместим
- ломающее изменение = `major++` **и** новый метод; старый живёт, пока не выпилим
- неизвестное host имя = `rejected.reason = "unsupported"`
- правило версий не сошлось = `rejected.reason = "version_mismatch"`

Сессия запоминает `accepted` для своего токена. Вызов метода вне `accepted` этой сессии → `version_mismatch`. Имя, которого нет у host вообще → `unsupported_method`.

Клиент кладёт в hello методы, которые собирается звать (без `handshake`). Пустой `methods` → `accepted: {}`, дальше только `host.ping` / `/health`.

### Методы MVP, все `{ "major": 1, "minor": 0 }`

| Метод | Плоскость |
|---|---|
| `handshake` | 1.0 (сам hello не торгуется; в `methods` не включают) |
| `host.ping` | 1.0 |
| `host.doctor` | 1.0 |
| `workspace.list` | 1.0 |
| `workspace.add` | 1.0 |
| `task.list` | 1.0 |
| `task.create` | 1.0 |
| `task.get` | 1.0 |
| `task.rename` | 1.0 |
| `task.archive` | 1.0 |
| `agent.list` | 1.0 |
| `agent.create` | 1.0 |
| `agent.get` | 1.0 |
| `agent.send` | 1.0 |
| `agent.get_context` | 1.0 |
| `files.tree` | 1.0 |
| `files.read` | 1.0 |

Формы WS-событий — 1.0, не торгуются per-method. Подписка — не RPC.

Имени `epic` на проводе нет. Сущность — **Task**.

---

## 3. Shared types

Общие правила:

- идентификаторы — строки uuid v7
- времена — строки RFC3339 UTC (`2026-08-17T10:50:00Z`)
- перечисления — строки, не числа
- имена полей на проводе — camelCase
- клиент не присылает server-owned поля (`id`, timestamps, `hostId`, `status` при create и т.д.), кроме явно указанных params

Лимиты (нарушение → `invalid_params`):

- `Message.content` и `agent.send` `content`: максимум 1 MiB = 1 048 576 байт UTF-8
- `Task.title` (create/rename): 1…200 символов (Unicode scalar values), не пустая строка

### Workspace

```
{
  "id":        string,   // uuid v7
  "hostId":    string,   // uuid v7, этот host
  "path":      string,   // абсолютный канонический путь на машине host
  "name":      string,   // basename(path) в момент add
  "createdAt": string    // RFC3339 UTC
}
```

```json
{
  "id": "0191f0c6-aaaa-7000-8000-000000000001",
  "hostId": "0191f0c6-7c2a-7c11-8000-6f0c1a2b3c4d",
  "path": "/home/u/proj",
  "name": "proj",
  "createdAt": "2026-08-17T10:51:00Z"
}
```

### Task

Никогда не `epic`. MVP: ровно один workspace в `workspaceIds`.

```
{
  "id":           string,                 // uuid v7
  "title":        string,                 // 1…200
  "status":       "open" | "archived",
  "createdAt":    string,                 // RFC3339 UTC
  "updatedAt":    string,                 // RFC3339 UTC
  "workspaceIds": [string]                // uuid v7[], MVP: длина 1
}
```

```json
{
  "id": "0191f0c6-bbbb-7000-8000-000000000002",
  "title": "Починить handshake",
  "status": "open",
  "createdAt": "2026-08-17T10:52:00Z",
  "updatedAt": "2026-08-17T10:52:00Z",
  "workspaceIds": ["0191f0c6-aaaa-7000-8000-000000000001"]
}
```

### Agent

Четыре разных понятия (architecture-v0 §9). На проводе не сплющивать.

| Понятие | Поле / факт MVP |
|---|---|
| Agent | строка в ответе, сессия внутри Task |
| Harness | `provider` = HarnessId. MVP: единственное значение `"cli.generic"` |
| Interface | `interface` всегда `"chat"` |
| Shell | не существует в MVP |

```
{
  "id":          string,                    // uuid v7
  "taskId":      string,                    // uuid v7
  "hostId":      string,                    // uuid v7, bind на жизнь
  "parentId":    null | string,             // MVP: всегда null
  "interface":   "chat",                    // других значений нет
  "provider":    "cli.generic",             // HarnessId
  "status":      "idle" | "running" | "error",
  "runLocation": "local",                   // других значений нет
  "createdAt":   string                     // RFC3339 UTC
}
```

```json
{
  "id": "0191f0c6-cccc-7000-8000-000000000003",
  "taskId": "0191f0c6-bbbb-7000-8000-000000000002",
  "hostId": "0191f0c6-7c2a-7c11-8000-6f0c1a2b3c4d",
  "parentId": null,
  "interface": "chat",
  "provider": "cli.generic",
  "status": "idle",
  "runLocation": "local",
  "createdAt": "2026-08-17T10:53:00Z"
}
```

Клиент не присылает `interface` / `parentId` / `runLocation` / `hostId`. Host выставляет константы MVP.

### Message

```
{
  "id":        string,                              // uuid v7
  "agentId":   string,                              // uuid v7
  "role":      "user" | "assistant" | "system" | "tool",
  "content":   string,                              // ≤ 1 MiB UTF-8
  "createdAt": string                               // RFC3339 UTC
}
```

```json
{
  "id": "0191f0c6-dddd-7000-8000-000000000004",
  "agentId": "0191f0c6-cccc-7000-8000-000000000003",
  "role": "user",
  "content": "что в README?",
  "createdAt": "2026-08-17T10:54:00Z"
}
```

`agent.send` всегда создаёт `role=user`. `assistant` пишет host (по чанку — отдельная строка Message, см. §6). `system` / `tool` в MVP клиент не шлёт; если host когда-нибудь запишет — клиент показывает как есть.

Мультимодального content нет: только строка.

### FileEntry

Только для `files.tree`. Не сущность БД.

```
{
  "name":       string,                 // basename
  "path":       string,                 // относительно корня workspace, разделитель `/`, без ведущего `/`
  "kind":       "file" | "dir",
  "size":       number | null,          // байты; для dir всегда null
  "modifiedAt": string | null           // RFC3339 UTC mtime; неизвестно — null
}
```

```json
{
  "name": "README.md",
  "path": "README.md",
  "kind": "file",
  "size": 1024,
  "modifiedAt": "2026-08-17T09:00:00Z"
}
```

Корень workspace в `path` = `""` не возвращаем как entry; entries — содержимое.

---

## 4. Handshake

Первый RPC после `GET /health` (health опционален, но GUI так делает). Сессия не нужна.

Имена на проводе: `ClientHello` = `params`, `ServerHello` = `ok`.

### ClientHello (`params`)

```json
{
  "client": "gui",
  "clientVersion": "0.1.0",
  "methods": {
    "host.ping": { "major": 1, "minor": 0 },
    "host.doctor": { "major": 1, "minor": 0 },
    "workspace.list": { "major": 1, "minor": 0 },
    "workspace.add": { "major": 1, "minor": 0 },
    "task.list": { "major": 1, "minor": 0 },
    "task.create": { "major": 1, "minor": 0 },
    "task.get": { "major": 1, "minor": 0 },
    "task.rename": { "major": 1, "minor": 0 },
    "task.archive": { "major": 1, "minor": 0 },
    "agent.list": { "major": 1, "minor": 0 },
    "agent.create": { "major": 1, "minor": 0 },
    "agent.get": { "major": 1, "minor": 0 },
    "agent.send": { "major": 1, "minor": 0 },
    "agent.get_context": { "major": 1, "minor": 0 },
    "files.tree": { "major": 1, "minor": 0 },
    "files.read": { "major": 1, "minor": 0 }
  }
}
```

| Поле | Тип | Правило |
|---|---|---|
| `client` | `"gui"` \| `"cli"` | иначе `invalid_params` |
| `clientVersion` | string (semver) | информационный, не гейт |
| `methods` | object | ключ = имя метода, значение `{major: int, minor: int}`. Не включать `handshake` |

### ServerHello (`ok`)

```json
{
  "hostId": "0191f0c6-7c2a-7c11-8000-6f0c1a2b3c4d",
  "hostVersion": "0.1.0",
  "sessionToken": "0191f0c6-eeee-7000-8000-000000000005",
  "accepted": {
    "host.ping": { "major": 1, "minor": 0 },
    "host.doctor": { "major": 1, "minor": 0 },
    "workspace.list": { "major": 1, "minor": 0 },
    "workspace.add": { "major": 1, "minor": 0 },
    "task.list": { "major": 1, "minor": 0 },
    "task.create": { "major": 1, "minor": 0 },
    "task.get": { "major": 1, "minor": 0 },
    "task.rename": { "major": 1, "minor": 0 },
    "task.archive": { "major": 1, "minor": 0 },
    "agent.list": { "major": 1, "minor": 0 },
    "agent.create": { "major": 1, "minor": 0 },
    "agent.get": { "major": 1, "minor": 0 },
    "agent.send": { "major": 1, "minor": 0 },
    "agent.get_context": { "major": 1, "minor": 0 },
    "files.tree": { "major": 1, "minor": 0 },
    "files.read": { "major": 1, "minor": 0 }
  },
  "rejected": {}
}
```

`rejected` пример, если клиент попросил лишнее:

```json
{
  "artifact.create": { "reason": "unsupported" },
  "task.create": { "reason": "version_mismatch" }
}
```

`reason`: `"unsupported"` | `"version_mismatch"`.

Handshake **успешен**, даже если часть методов в `rejected`. Клиент сам решает, жить ли с дыркой. `sessionToken` выдаётся всегда при `ok`.

`hostId` в ServerHello == `hostId` в pid.json == row `host` в БД.

---

## 5. RPC methods

Общее, если метод не оговаривает иначе:

- «когда» = после handshake, валидный `X-Rt-Session`, метод в `accepted`
- неизвестный uuid-формат в id-полях → `invalid_params`
- верный формат, нет строки → `not_found`
- чужой тип JSON → `invalid_params`
- `params` не объект → `invalid_params`

Ниже: version, когда, params, ok, errors. `errors` — специфичные плюс общие (`unauthorized`, `unsupported_method`, `version_mismatch`, `internal`).

---

### `handshake` — 1.0

Когда: всегда, без сессии. Повторный вызов разрешён.

params: ClientHello, §4.

ok: ServerHello, §4.

errors: `invalid_params`.

---

### `host.ping` — 1.0

Когда: всегда, без сессии. Liveness после discovery, до или после hello.

params:

```json
{}
```

ok:

```json
{
  "hostId": "0191f0c6-7c2a-7c11-8000-6f0c1a2b3c4d",
  "now": "2026-08-17T10:55:00Z"
}
```

`now` — часы host, RFC3339 UTC.

errors: нет специфичных.

---

### `host.doctor` — 1.0

Когда: после handshake.

params:

```json
{}
```

ok:

```json
{
  "hostId": "0191f0c6-7c2a-7c11-8000-6f0c1a2b3c4d",
  "pid": 12345,
  "rpcUrl": "http://127.0.0.1:47800",
  "dbOk": true,
  "dataDir": "/home/u/.rusttraycer/host",
  "dbPath": "/home/u/.rusttraycer/host/host.db",
  "logPath": "/home/u/.rusttraycer/host/host.log",
  "providers": [
    {
      "id": "cli.generic",
      "available": true,
      "detail": "RUSTTRAYCER_GENERIC_CMD=/usr/local/bin/my-agent"
    }
  ],
  "workspaceCount": 1,
  "taskCount": 2,
  "agentCount": 1
}
```

| Поле | Тип | Смысл |
|---|---|---|
| `hostId` | string | тот же, что pid.json |
| `pid` | number | pid процесса |
| `rpcUrl` | string | как в pid.json |
| `dbOk` | bool | БД открыта / быстрый integrity |
| `dataDir` | string | каталог данных host |
| `dbPath` | string | путь `host.db` |
| `logPath` | string | путь `host.log` |
| `providers` | array | все известные harness; MVP — один элемент |
| `providers[].id` | string | HarnessId, `"cli.generic"` |
| `providers[].available` | bool | `cli.generic`: true ⇔ задан `RUSTTRAYCER_GENERIC_CMD` и бинарь резолвится. Критерий — host-runtime-v0 §3, не дублируем здесь |
| `providers[].detail` | string | почему да/нет, для человека |
| `workspaceCount` | number | число row workspace |
| `taskCount` | number | все Task, включая archived |
| `agentCount` | number | все Agent |

`wsUrl` в doctor нет (он в pid.json).

errors: нет специфичных.

---

### `workspace.list` — 1.0

Когда: после handshake.

params:

```json
{}
```

ok:

```json
{
  "items": [
    {
      "id": "0191f0c6-aaaa-7000-8000-000000000001",
      "hostId": "0191f0c6-7c2a-7c11-8000-6f0c1a2b3c4d",
      "path": "/home/u/proj",
      "name": "proj",
      "createdAt": "2026-08-17T10:51:00Z"
    }
  ]
}
```

Порядок: `createdAt` asc, затем `id`. Пустой список — валиден.

errors: нет специфичных.

---

### `workspace.add` — 1.0

Когда: после handshake.

params:

```json
{
  "path": "/home/u/proj"
}
```

| Поле | Тип | Правило |
|---|---|---|
| `path` | string | обязателен. Host делает `canonicalize`. Должен существовать и быть директорией |

ok: `Workspace` (канонический `path`, `name` = basename).

Повторный add того же канонического пути — идемпотентен: вернуть существующий row, новый id не создавать.

errors: `workspace_path_invalid` (нет пути / не dir / canonicalize failed), `invalid_params` (нет `path` / не строка).

---

### `task.list` — 1.0

Когда: после handshake.

params:

```json
{
  "status": "open"
}
```

| Поле | Тип | Правило |
|---|---|---|
| `status` | `"open"` \| `"archived"` \| `"all"` | обязателен |

ok:

```json
{
  "items": [ { "id": "0191f0c6-bbbb-7000-8000-000000000002", "title": "Починить handshake", "status": "open", "createdAt": "2026-08-17T10:52:00Z", "updatedAt": "2026-08-17T10:52:00Z", "workspaceIds": ["0191f0c6-aaaa-7000-8000-000000000001"] } ]
}
```

Порядок: `updatedAt` desc, затем `id` desc.

errors: `invalid_params`.

---

### `task.create` — 1.0

Когда: после handshake.

params:

```json
{
  "title": "Починить handshake",
  "workspaceId": "0191f0c6-aaaa-7000-8000-000000000001"
}
```

| Поле | Тип | Правило |
|---|---|---|
| `title` | string | 1…200 символов |
| `workspaceId` | string | существующий Workspace этого host |

ok: `Task` (`status` = `"open"`, `workspaceIds` = `[workspaceId]`, timestamps выставляет host).

errors: `invalid_params` (title/длина/тип), `not_found` (workspace нет).

MVP: ровно одна связь task↔workspace. Поля под несколько workspace на проводе уже есть, RPC их не принимает.

---

### `task.get` — 1.0

Когда: после handshake.

params:

```json
{
  "id": "0191f0c6-bbbb-7000-8000-000000000002"
}
```

ok: `Task`.

errors: `invalid_params`, `not_found`.

---

### `task.rename` — 1.0

Когда: после handshake.

params:

```json
{
  "id": "0191f0c6-bbbb-7000-8000-000000000002",
  "title": "Новое имя"
}
```

ok: `Task` с новым `title` и свежим `updatedAt`. То же имя — ок, `updatedAt` всё равно трогаем.

После успеха — WS `task.updated`.

errors: `invalid_params`, `not_found`.

---

### `task.archive` — 1.0

Когда: после handshake.

params:

```json
{
  "id": "0191f0c6-bbbb-7000-8000-000000000002"
}
```

ok: `Task` со `status=archived`, свежий `updatedAt`. Уже archived — идемпотентно, вернуть как есть (без обязательного нового timestamp).

После смены статуса — WS `task.updated`. Unarchive в MVP нет.

errors: `invalid_params`, `not_found`.

Архив не стопает inflight turn и не удаляет Agent/Message.

---

### `agent.list` — 1.0

Когда: после handshake.

params:

```json
{
  "taskId": "0191f0c6-bbbb-7000-8000-000000000002"
}
```

ok:

```json
{
  "items": [ { "id": "0191f0c6-cccc-7000-8000-000000000003", "taskId": "0191f0c6-bbbb-7000-8000-000000000002", "hostId": "0191f0c6-7c2a-7c11-8000-6f0c1a2b3c4d", "parentId": null, "interface": "chat", "provider": "cli.generic", "status": "idle", "runLocation": "local", "createdAt": "2026-08-17T10:53:00Z" } ]
}
```

Порядок: `createdAt` asc, `id` asc. Нет агентов — `items: []`. Нет таска — `not_found`.

errors: `invalid_params`, `not_found`.

---

### `agent.create` — 1.0

Когда: после handshake.

params:

```json
{
  "taskId": "0191f0c6-bbbb-7000-8000-000000000002",
  "provider": "cli.generic"
}
```

| Поле | Тип | Правило |
|---|---|---|
| `taskId` | string | существующий Task |
| `provider` | string | HarnessId. Можно опустить → `"cli.generic"`. Любое другое значение → `invalid_params` |

ok: `Agent` (`status=idle`, `interface=chat`, `parentId=null`, `runLocation=local`, `hostId` = этот host).

`available=false` у harness **не** блокирует create. Упадёт на `agent.send` (`internal` / `Failed` → `error`). Doctor показывает available.

errors: `invalid_params`, `not_found`.

---

### `agent.get` — 1.0

Когда: после handshake.

params:

```json
{
  "id": "0191f0c6-cccc-7000-8000-000000000003"
}
```

ok: поля `Agent` плюс одно extra-поле этого метода:

```json
{
  "id": "0191f0c6-cccc-7000-8000-000000000003",
  "taskId": "0191f0c6-bbbb-7000-8000-000000000002",
  "hostId": "0191f0c6-7c2a-7c11-8000-6f0c1a2b3c4d",
  "parentId": null,
  "interface": "chat",
  "provider": "cli.generic",
  "status": "idle",
  "runLocation": "local",
  "createdAt": "2026-08-17T10:53:00Z",
  "lastMessageAt": "2026-08-17T10:54:00Z"
}
```

`lastMessageAt`: RFC3339 UTC последнего Message этого агента, либо `null` если транскрипт пуст. В общем типе `Agent` (§3) поля нет — только здесь.

errors: `invalid_params`, `not_found`.

---

### `agent.send` — 1.0

Когда: после handshake. Один активный turn на агента (host-runtime-v0 §1.6).

params:

```json
{
  "agentId": "0191f0c6-cccc-7000-8000-000000000003",
  "content": "что в README?"
}
```

| Поле | Тип | Правило |
|---|---|---|
| `agentId` | string | существующий Agent |
| `content` | string | непустой, ≤ 1 048 576 байт UTF-8 |

ok (сразу, не дожидаясь assistant):

```json
{
  "userMessage": {
    "id": "0191f0c6-dddd-7000-8000-000000000004",
    "agentId": "0191f0c6-cccc-7000-8000-000000000003",
    "role": "user",
    "content": "что в README?",
    "createdAt": "2026-08-17T10:54:00Z"
  }
}
```

Семантика (host-runtime-v0 §3, здесь только провод):

1. нет агента → `not_found`
2. `status=running` или есть inflight → `agent_busy`, очередь не строится
3. user Message пишется в БД **до** старта turn
4. RPC возвращает этот `userMessage`
5. стрим assistant идёт только в WS (`agent.message` по чанку, `agent.status`)
6. смерть GUI turn не стопает

errors: `invalid_params` (пустой/слишком большой content), `not_found`, `agent_busy`.

`agent.cancel` в MVP нет.

---

### `agent.get_context` — 1.0

Когда: после handshake. Снимок транскрипта. Отдельной таблицы Context нет (architecture-v0 §4).

params:

```json
{
  "agentId": "0191f0c6-cccc-7000-8000-000000000003"
}
```

ok:

```json
{
  "messages": [
    {
      "id": "0191f0c6-dddd-7000-8000-000000000004",
      "agentId": "0191f0c6-cccc-7000-8000-000000000003",
      "role": "user",
      "content": "что в README?",
      "createdAt": "2026-08-17T10:54:00Z"
    }
  ]
}
```

Порядок: `createdAt` asc, затем `id` asc. Полный transcript агента, без артефактов (их нет). После рестарта GUI клиент зовёт это, а не ждёт replay WS.

errors: `invalid_params`, `not_found`.

---

### `files.tree` — 1.0

Когда: после handshake. Read-only обход FS **этого** host внутри workspace. Не watch, не git.

params:

```json
{
  "workspaceId": "0191f0c6-aaaa-7000-8000-000000000001",
  "path": "src",
  "depth": 2,
  "maxEntries": 500
}
```

| Поле | Тип | Правило |
|---|---|---|
| `workspaceId` | string | существующий Workspace |
| `path` | string, optional | относительно корня workspace. Нет / `""` / `"."` = корень. Разделитель `/`. Без ведущего `/` |
| `depth` | number, optional | целый ≥ 1. Default **2**. 1 = только непосредственные дети `path` |
| `maxEntries` | number, optional | целый ≥ 1. Default **500**. Потолок числа элементов в `items` |

`path` после join с корнем + canonicalize обязан остаться внутри workspace (префикс канонического корня + separator, либо равен корню). `..`, symlink наружу, абсолютный path → `invalid_params`.

ok:

```json
{
  "items": [
    {
      "name": "src",
      "path": "src",
      "kind": "dir",
      "size": null,
      "modifiedAt": "2026-08-17T09:00:00Z"
    },
    {
      "name": "main.rs",
      "path": "src/main.rs",
      "kind": "file",
      "size": 220,
      "modifiedAt": "2026-08-17T09:10:00Z"
    }
  ],
  "truncated": false
}
```

Порядок: depth-first, на каждом уровне dirs затем files, имя ascending (байтовый UTF-8).

`path` указывает на файл — `items` из одного FileEntry, `depth` игнорируется.

`truncated=true`, если упёрлись в `maxEntries` (остальное не отдаём). Стабильный префикс того же порядка.

Не существующий `path` → `not_found`. Нет workspace → `not_found`.

errors: `invalid_params` (escape, типы, depth/maxEntries < 1), `not_found`.

---

### `files.read` — 1.0

Когда: после handshake. Read-only. Редактора в GUI нет — только просмотр.

params:

```json
{
  "workspaceId": "0191f0c6-aaaa-7000-8000-000000000001",
  "path": "README.md"
}
```

| Поле | Тип | Правило |
|---|---|---|
| `workspaceId` | string | существующий Workspace |
| `path` | string | относительный, обязателен, непустой. Те же правила «не выйти из workspace», что у `files.tree` |

ok:

```json
{
  "path": "README.md",
  "content": "# proj\n",
  "truncated": false,
  "encoding": "utf8"
}
```

| Поле | Тип | Правило |
|---|---|---|
| `path` | string | нормализованный relative (как в FileEntry) |
| `content` | string | весь файл, UTF-8 |
| `truncated` | bool | в MVP всегда `false`: файл больше лимита не читаем, а режем ошибкой |
| `encoding` | `"utf8"` | других значений нет |

Отказы:

- файл > 256 KiB (262 144 байт) → `file_too_large` (не читаем кусок)
- не текст: байт `0x00` в первых 8 KiB **или** содержимое не валидный UTF-8 → `file_binary`
- path вне workspace / `..` / абсолютный → `invalid_params`
- path — директория → `invalid_params`
- файла нет → `not_found`
- workspace нет → `not_found`

errors: `invalid_params`, `not_found`, `file_too_large`, `file_binary`.

---

## 6. WebSocket

`GET /ws` на том же listener. Заголовок `X-Rt-Session` на upgrade обязателен. Иначе HTTP 401, сокет не открывать.

Первое клиентское сообщение после open — и только оно в эту сторону в MVP:

```json
{
  "type": "subscribe",
  "taskId": "0191f0c6-bbbb-7000-8000-000000000002"
}
```

или на все события этого host:

```json
{
  "type": "subscribe",
  "taskId": null
}
```

`taskId` строка несуществующего таска — закрыть сокет после текстовой ошибки не надо: просто не будет `task.*` / агентских событий (таска нет). Невалидный uuid → закрыть 1008.

Повторный `subscribe` на том же сокете заменяет фильтр. Иное первое сообщение / не JSON — закрыть 1003.

Replay нет. После reconnect: `subscribe` + `agent.get_context` / `task.get` по нужным id.

Несколько сокетов на один токен — можно. Смерть GUI сокет рвёт, turn живёт.

### События (host → client)

Одно JSON-сообщение = одно событие. Поле-дискриминатор: `event`.

#### `agent.message`

```json
{
  "event": "agent.message",
  "taskId": "0191f0c6-bbbb-7000-8000-000000000002",
  "agentId": "0191f0c6-cccc-7000-8000-000000000003",
  "message": {
    "id": "0191f0c6-ffff-7000-8000-000000000006",
    "agentId": "0191f0c6-cccc-7000-8000-000000000003",
    "role": "assistant",
    "content": "в корне есть README.md",
    "createdAt": "2026-08-17T10:54:01Z"
  }
}
```

Эмитится:

- сразу после записи user Message в `agent.send` (клиент уже имеет её в RPC `ok`; на WS — дубль, идемпотентность по `message.id`)
- на каждый assistant-чанк

**MVP: один Message row на чанк.** Нет UPDATE одной assistant-строки. Граница чанка — дело host (≈100 мс или `\n`, host-runtime-v0 §3). Клиент конкатенирует последовательные `role=assistant` для отображения, если хочет сплошной пузырь; в БД они раздельно.

#### `agent.status`

```json
{
  "event": "agent.status",
  "taskId": "0191f0c6-bbbb-7000-8000-000000000002",
  "agentId": "0191f0c6-cccc-7000-8000-000000000003",
  "status": "running"
}
```

`status`: `"idle"` | `"running"` | `"error"`.
Эмит при каждом реальном переходе (send → running; Finished → idle; Failed / паника turn / рестарт host с недописанным turn → error).

#### `task.updated`

```json
{
  "event": "task.updated",
  "taskId": "0191f0c6-bbbb-7000-8000-000000000002"
}
```

Эмит после `task.rename`, `task.archive` (если статус сменился), и после `agent.send` (host трогает `updatedAt` таска). Тела Task нет — клиент делает `task.get` при необходимости.

#### `host.going_away`

```json
{
  "event": "host.going_away",
  "hostId": "0191f0c6-7c2a-7c11-8000-6f0c1a2b3c4d"
}
```

SIGINT/SIGTERM: разослать, flush БД (не ждать turn дольше 2 с), убить children, снять pid.json, выйти. Клиент не шлёт новые RPC.

### Связка `agent.send` и WS

```
POST /rpc agent.send
  → 200 { ok: { userMessage } }     // сразу

WS (подписчикам taskId или null):
  agent.status  running
  agent.message user                 // тот же id, что userMessage
  agent.message assistant            // 0..N чанков, каждый — отдельный Message
  agent.status  idle | error
  task.updated
```

Порядок status(running) относительно RPC-ответа не гарантирован жёстче, чем «оба после commit user Message». Клиент ключ — `message.id`.

Формат stdin/stdout `cli.generic` на провод **не** выносится. Он во внутреннем шве host-runtime-v0 §3: stdin = `{"messages":[...]}` + `\n` + close; stdout = UTF-8 текст = Token. Здесь не меняем.

---

## 7. Sequence

Петля MVP: папка → Task → агент → сообщение → токены на WS → транскрипт жив после рестарта GUI.

```mermaid
sequenceDiagram
  participant GUI
  participant Pid as pid.json
  participant Host

  GUI->>Pid: read ~/.rusttraycer/host/pid.json
  GUI->>Host: GET /health
  Host-->>GUI: {ok:true, hostId}
  GUI->>Host: POST /rpc handshake
  Host-->>GUI: {sessionToken, accepted}
  GUI->>Host: workspace.add {path}
  Host-->>GUI: Workspace
  GUI->>Host: task.create {title, workspaceId}
  Host-->>GUI: Task
  GUI->>Host: agent.create {taskId, provider}
  Host-->>GUI: Agent
  GUI->>Host: GET /ws + X-Rt-Session
  GUI->>Host: {type:subscribe, taskId}
  GUI->>Host: agent.send {agentId, content}
  Host-->>GUI: {userMessage}
  Host-->>GUI: WS agent.status running
  Host-->>GUI: WS agent.message (assistant chunks)
  Host-->>GUI: WS agent.status idle
  Note over GUI,Host: host.db держит Message; рестарт GUI → handshake + get_context
```

Рестарт **host**: `hostId` тот же, Task/Agent/Message на месте, токены мертвы, агент с недописанным turn → `error`, частичный assistant сохранён. Клиент: заново health → handshake → subscribe → get_context.

---

## 8. Out of MVP (не реализовывать)

На проводе и в host **нет**:

- `terminal.*`, PTY, mux, Shell как сущность
- `worktree.*`, `runLocation` кроме `"local"`
- `artifact.*` (Spec/Ticket/Story/Review)
- `a2a.*` (reference / transcript / deliver)
- `comments.*`
- `git.diff` (достаточно того, что doctor не врёт про БД; git status как поверхность — нет)
- `agent.cancel` / очередь send
- несколько host в одном GUI, cross-host, cloud sync, аккаунты
- `interface=terminal`, ненулевой `parentId`, второй Harness
- file write / watch / «открыть в редакторе» как RPC
- replay WS, persistence-версии на запись

Пустые модули `pty.rs` / `worktree.rs` / `mux.rs` в host — не повод открывать методы.

---

## 9. Open questions

Закрыто этим документом:

- `files.tree` / `files.read` — §5. Open item host-runtime-v0 §6 снят.
- лимит `content` — 1 MiB, иначе `invalid_params`.
- имена на проводе — camelCase, конверт, сессия, коды ошибок включая `unauthorized` / `file_too_large` / `file_binary`.

Не переопределяем:

- stdin/stdout `cli.generic` — host-runtime-v0 §3 (`{"messages":[...]}` + `\n` + close stdin; stdout = Token). Integration хочет другой первый адаптер — отдельная спека, не правка этого файла.

Ещё открыто (не блокирует MVP, не выбирать вслепую):

- **`agent.cancel`.** В MVP нет. Второй `agent.send` при Running → `agent_busy`. Нужен ли cancel до первого релиза — решает Architect, когда UI упрётся в 10-минутный timeout, а не Core «на всякий случай».
