> **v1.0 reader:** this file is the original draft. worktree + git RPC implemented; empty pty.rs/mux.rs remain reserved. See [v1-delta.md](v1-delta.md).

# Host Runtime — спецификация v0

Для: Core (`rt-host`, `rt-storage`, `rt-runtime`).
От: Architect. Дата: 2026-08-17.
Статус: действующий контракт MVP. Не production-код: типы и швы, не реализация.

Обзор всего продукта: `/workspace/rusttraycer-arch/architecture-v0.md`
Репо/ветки нет. Источник правды — эти два файла на общей машине.

---

## 0. Твоя зона и чужая

Пишешь ты:
- `crates/rt-host` — демон, HTTP/WS, домен, supervisor агентов
- `crates/rt-storage` — rusqlite, миграции, единственный писатель `host.db`

Вызываешь, не реализуешь:
- `crates/rt-runtime` — trait `AgentBackend` и `cli.generic`. Владеет Integration. Контракт: `runtime-adapters-v0.md`.

Не пишешь:
- `rt-protocol` — типы на проводе. Источник правды: `protocol-v0.md`.
- `rt-cli` — жизненный цикл процесса (start/stop/doctor снаружи). Ты только реализуешь то, что процесс делает после exec.
- `rt-gui` — не твой.
- реализации `AgentBackend` — Integration

Жёсткий запрет зависимостей:
- `rt-host` → `rt-protocol`, `rt-storage`, `rt-runtime`
- `rt-storage` не зависит от axum/tokio-net и не знает HTTP
- `rt-runtime` не открывает БД
- никто из этих трёх не зависит от `rt-gui` / `rt-cli`

---

## 1. Инварианты (ломать = баг)

1. Host — единственный процесс, который трогает workspace FS, git, дочерние процессы и `host.db`.
2. Слушаешь только `127.0.0.1`. Любой non-loopback remote reject.
3. `hostId` стабилен между рестартами. Берётся из БД (таблица `host`), не генерится заново при старте. В `pid.json` пишется тот же id.
4. Агент навсегда привязан к этому `hostId`. Нет migrate на другой host.
5. Chat transcript durable: `agent.send` сначала пишет user-message в БД, потом стартует turn. Assistant-токены/финал тоже в БД. Рестарт GUI не теряет историю. Рестарт host: недописанный turn → `Error`, частичный assistant-текст сохраняется.
6. Один активный turn на агента. Второй `agent.send` пока `Running` → ошибка `agent_busy`, не очередь.
7. GUI/CLI не дети host. Host переживает их смерть. Их смерть не стопает turn.
8. PTY, terminal multiplexer, git worktree — **не MVP**. Модульные швы можно оставить пустыми (`rt-host/src/pty.rs`, `worktree.rs` как `todo`/пустой mod), реализации нет. Не тащи `portable-pty` и `git2` в MVP.
9. Один host на машину в MVP. Второй процесс, увидев живой pid, выходит с ошибкой.

---

## 2. Жизненный цикл процесса

### Старт (то, что делает бинарь `rt-host`)

1. Каталог данных: `$RUSTTRAYCER_HOME` или `directories::ProjectDirs` → `~/.rusttraycer/host/`.
2. Открыть/создать `host.db` (WAL, `busy_timeout=5000`, foreign_keys=ON). Прогнать миграции.
3. Прочитать или создать row `host(id, name, created_at)`. `id` = uuid v7, один раз.
4. Проверить `pid.json`: если pid жив и это не мы — exit 2 (`already_running`).
5. Bind `127.0.0.1:0` (эфемерный порт). Один listener и для HTTP, и для WS.
6. Атомарно записать `pid.json` (write temp + rename):

```json
{
  "hostId": "<uuid v7>",
  "pid": 12345,
  "rpcUrl": "http://127.0.0.1:<port>",
  "wsUrl": "ws://127.0.0.1:<port>/ws",
  "startedAt": "<RFC3339>",
  "protocol": { "crate": "0.1.0" }
}
```

7. Лог: `~/.rusttraycer/host/host.log` (tracing, append).
8. SIGINT/SIGTERM: разослать `host.going_away`, дождаться flush БД (не ждать конца turn дольше 2с), убить children, снять `pid.json` только если pid совпал, выйти.

`rt-cli start` просто exec этот бинарь. Ты не импортируешь cli.

### Doctor (внутренний, RPC `host.doctor`)

Вернуть JSON:
- `hostId`, `pid`, `rpcUrl`
- `dbOk` (pragma integrity_check быстро / просто open)
- `dataDir`, `dbPath`, `logPath`
- `providers`: `[{ "id": "cli.generic", "available": bool, "detail": "..." }]`
- `workspaceCount`, `taskCount`, `agentCount`

---

## 3. Публичные API внутри host (швы крейтов)

Это контракт между твоими крейтами. Имена можно чуть двигать, семантика нет.

### `rt-storage`

Один `Store` на процесс. Не `Clone` соединения вслепую: либо `r2d2`/`deadpool` с max 1 writer + N readers, либо один `Mutex<Connection>` на MVP. Предпочтение MVP: `Arc<Mutex<Connection>>`. Просто и соответствует «один писатель».

```
Store::open(path) -> Store
Store::migrate() -> Result

Store::host_get() -> HostRow
Store::host_insert_if_absent(id, name)

Store::workspace_list() -> Vec<Workspace>
Store::workspace_add(path, name) -> Workspace   # path абсолютный, канонизированный
Store::workspace_get(id) -> Option<Workspace>

Store::task_list(filter: Open|Archived|All) -> Vec<Task>
Store::task_create(title, workspace_id) -> Task
Store::task_get(id) -> Option<Task>
Store::task_rename(id, title)
Store::task_archive(id)
Store::task_touch(id)  # updated_at

Store::agent_list(task_id) -> Vec<Agent>
Store::agent_create(...) -> Agent
Store::agent_get(id) -> Option<Agent>
Store::agent_set_status(id, Idle|Running|Error)

Store::message_append(agent_id, role, content) -> Message
Store::message_list(agent_id) -> Vec<Message>   # порядок created_at, id
```

Идентификаторы: uuid v7, в БД TEXT. Времена: RFC3339 UTC TEXT.

Миграция `0001`:

```
host(id PK, name, created_at)
workspaces(id PK, host_id FK, path UNIQUE, name, created_at)
tasks(id PK, title, status CHECK IN ('open','archived'), created_at, updated_at)
task_workspaces(task_id FK, workspace_id FK, PRIMARY KEY(task_id, workspace_id))
agents(
  id PK, task_id FK, host_id FK, parent_id NULL,
  interface CHECK IN ('chat'),          -- только chat в MVP
  provider TEXT,                        -- 'cli.generic'
  status CHECK IN ('idle','running','error'),
  run_location CHECK IN ('local'),
  created_at
)
messages(id PK, agent_id FK, role CHECK IN ('user','assistant','system','tool'),
         content TEXT, created_at)
schema_meta(key PK, value)  -- key='schema', value='1'
```

MVP: один глобальный schema version. Per-record `{major,minor}` не делаем.

`workspace.add`: path должен существовать и быть директорией, иначе ошибка `workspace_path_invalid`. Нормализуй через `canonicalize`.

`task.create`: `workspace_id` обязан существовать. MVP: ровно одна связь в `task_workspaces`.

### `rt-runtime` (вызов, не реализация)

Источник правды на trait и wire: `runtime-adapters-v0.md`.

Supervisor вызывает `AgentBackend::start_turn` и читает `TurnEvent`.
`caps()` на trait есть; в MVP игнорируй.
Не парси stdout child сам. Не знай про stdin JSON.

`cli.generic` wire (кратко, детали у Integration):
- env `RUSTTRAYCER_GENERIC_CMD` = один путь к бинарю (не split по пробелам)
- опционально `RUSTTRAYCER_GENERIC_ARGS` = JSON-массив строк
- stdin: `{"messages":[...]}` + newline + EOF; agentId/taskId в env, путь = cwd
- stdout: сырой UTF-8 = `Token`
- timeout 10 мин, kill process group

`available=false` не блокирует `agent.create`. Блокирует/роняет `agent.send` (`internal` + detail).

### `rt-host` — supervisor

```
HostService {
  store: Store,
  backends: HashMap<ProviderId, Arc<dyn AgentBackend>>,
  inflight: HashMap<AgentId, JoinHandle<()>>,  # один на агента
  events: broadcast::Sender<WsEvent>,
}

HostService::send(agent_id, content) -> Result<Message, SendError>
```

Алгоритм `send`:

1. Загрузить agent. Нет → `not_found`.
2. Если `status==Running` или есть inflight → `agent_busy`.
3. `message_append(user, content)`.
4. `agent_set_status(Running)`, emit `agent.status`, emit `agent.message` (user).
5. Собрать `TurnRequest` (workspace path из единственного workspace таска, полный `message_list`).
6. Spawn task: читать stream backend.
   - каждый `Token`: append к буферу; на границе (каждые ~100мс или \n) `message_append(assistant, chunk)` + emit `agent.message`. Можно один assistant-message копить и апдейтить: **решение MVP — много Message row, по чанку**. Проще, нет UPDATE.
   - `Finished`: если буфер не сброшен, дописать; `status=Idle`; emit status.
   - `Failed`: дописать буфер если есть; `status=Error`; emit status.
7. Вернуть user `Message` из шага 3 (RPC ответ). Стрим идёт в WS.

Паника в turn task не роняет host. Ловим, ставим `Error`.

### HTTP поверхность (`rt-host`)

Один JSON-RPC-подобный POST, не REST-коллекция.

```
POST /rpc
Content-Type: application/json

{ "id": "<client-req-id>", "method": "task.create", "params": { ... } }

200:
{ "id": "...", "ok": { ... } }
или
{ "id": "...", "error": { "code": "agent_busy", "message": "..." } }
```

`GET /health` → 200 `{ "ok": true, "hostId": "..." }` без handshake.

`GET /ws` → websocket, после connect клиент шлёт `{ "type": "subscribe", "taskId": "..." | null }`.
`null` = все события этого host. MVP достаточно.

Коды ошибок (стабильные строки):
`not_found`, `invalid_params`, `agent_busy`, `workspace_path_invalid`,
`unsupported_method`, `version_mismatch`, `internal`, `already_running` (только процесс).

---

## 4. Протокол RPC (то, что ты принимаешь)

Все `{major:1, minor:0}` в MVP. Handshake обязателен перед любым методом кроме `host.ping` и `GET /health`. Сессию handshake в MVP можно сделать stateless: клиент повторяет hello в каждом запросе **не надо**. Проще: после `handshake` выдать `sessionToken` (uuid) и требовать заголовок `X-Rt-Session`. Token живёт в памяти процесса, умирает с host.

### `handshake`

params:
```
{ "client": "gui"|"cli", "clientVersion": "0.1.0",
  "methods": { "task.create": {"major":1,"minor":0}, ... } }
```
result:
```
{ "hostId": "...", "hostVersion": "0.1.0", "sessionToken": "...",
  "accepted": { "task.create": {"major":1,"minor":0}, ... },
  "rejected": {} }
```
Правило: major равен, client.minor ≤ host.minor. Иначе метод в `rejected`.

### Остальные методы (params → result)

`host.ping` → `{ "hostId", "now" }`
`host.doctor` → см. §2
`workspace.list` → `{ "items": [Workspace] }`
`workspace.add` `{ "path": "/abs" }` → `Workspace`
`task.list` `{ "status": "open"|"archived"|"all" }` → `{ "items": [Task] }`
`task.create` `{ "title", "workspaceId" }` → `Task`
`task.get` `{ "id" }` → `Task`
`task.rename` `{ "id", "title" }` → `Task`
`task.archive` `{ "id" }` → `Task`
`agent.list` `{ "taskId" }` → `{ "items": [Agent] }`
`agent.create` `{ "taskId", "provider": "cli.generic" }` → `Agent`
`agent.get` `{ "id" }` → `Agent` + можно `lastMessageAt`
`agent.send` `{ "agentId", "content" }` → `{ "userMessage": Message }`
`agent.get_context` `{ "agentId" }` → `{ "messages": [Message] }`

JSON-имена полей: camelCase на проводе. В Rust — snake_case + serde rename.

Формы сущностей на проводе:

```
Workspace { id, hostId, path, name, createdAt }
Task      { id, title, status, createdAt, updatedAt, workspaceIds }
Agent     { id, taskId, hostId, parentId, interface, provider, status, runLocation, createdAt }
Message   { id, agentId, role, content, createdAt }
```

`parentId` всегда `null` в MVP. `interface` всегда `"chat"`. `runLocation` всегда `"local"`.

### WS события

```
{ "event": "agent.message", "taskId", "agentId", "message": Message }
{ "event": "agent.status",  "taskId", "agentId", "status": "idle"|"running"|"error" }
{ "event": "task.updated",  "taskId" }
{ "event": "host.going_away", "hostId" }
```

---

## 5. Модульная раскладка `rt-host` (чтобы PTY/worktree не расползлись)

```
rt-host/src/
  main.rs          # только старт процесса
  lib.rs
  bind.rs          # pid.json, listen, shutdown
  rpc.rs           # axum /rpc + /health + /ws
  handshake.rs
  service.rs       # HostService
  supervisor.rs    # inflight turns
  files.rs         # read-only tree/read — после RPC файлов; в MVP можно заглушку
  pty.rs           # ПУСТО. не реализовывать
  worktree.rs      # ПУСТО. не реализовывать
  mux.rs           # ПУСТО. terminal multiplexer, не реализовывать
```

Когда дойдём до post-MVP:
- PTY живёт в `pty.rs` + отдельный live-state, не в `messages`
- worktree — отдельная таблица и `RunLocation`, не подмена `workspaces.path`
- mux — поверх PTY, не поверх Chat

Не смешивай Chat transcript и terminal scrollback.

---

## 6. Решено / открыто

Решено:
- три крейта, зависимости, инварианты §1
- эфемерный порт + pid.json
- localhost only, без auth кроме sessionToken в памяти
- SQLite WAL, один writer, миграция 0001
- uuid v7, RFC3339 UTC
- один turn на агента, reject busy
- chat durable, чанки = отдельные Message
- `cli.generic` через stdin JSON / stdout text
- PTY / mux / worktree / A2A / cloud — не делать
- JSON-RPC POST /rpc + WS /ws
- handshake → sessionToken

Открыто (не блокирует MVP, не выбирай сам если упрёшься — напиши мне):
- точный wire-формат `cli.generic` — закрыто в runtime-adapters-v0.md
- read-only file tree RPC — закрыто в protocol-v0.md (`files.tree`, `files.read`)
- нужен ли `agent.cancel` в MVP (сейчас нет)
- лимит размера `content` (пока 1 MiB на message, обрезать с ошибкой `invalid_params`)

---

## 7. Definition of done MVP (host)

1. `rt-host` стартует, пишет pid.json, переживает рестарт, второй инстанс не встаёт.
2. handshake + все RPC §4 кроме файлов.
3. `agent.send` гоняет backend из `rt-runtime` (мок Integration), пишет transcript, шлёт WS.
4. Рестарт host: старые Task/Agent/Message на месте, `hostId` тот же, Running-агент стал Error.
5. Тесты: storage roundtrip; supervisor busy; handshake version reject; pid lock.
6. В дереве есть пустые `pty.rs` / `worktree.rs` / `mux.rs` без зависимостей.

Нет репо. Когда Chief/Integration заведут — эти файлы переедут туда как `docs/`.

---

## 8. Уточнение: harness, не «просто provider»

`Agent.provider` на проводе = **HarnessId**. В MVP строка `"cli.generic"`.
Это не тип агента и не interface.

- Agent — строка в `agents`
- Harness — backend в `rt-runtime` (`AgentBackend::id`)
- Interface — колонка `interface='chat'`
- Shell — не существует в MVP (`pty.rs` пустой)

Не заводи таблицу harnesses. Не сплющивай эти четыре вещи в один enum.
`cli.generic` остаётся единственным backend в MVP. `HarnessCaps` уже на trait (runtime-adapters-v0 §5); matrix живая со вторым харнессом. Не match по provider в supervisor.

Permissions ladder и `AGENTS.md` — не твой MVP.
