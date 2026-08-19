> **v1.0 reader:** this file is the original draft. Host: cancel, claude/codex, git/worktree. GUI: still one `cli.generic` per Task. PTY/A2A/cloud did not ship. See [v1-delta.md](v1-delta.md).

# RustTraycer — архитектура v0

Статус: предложение Architect, 2026-08-17.
Это не код. Это контракт для MVP и каркас, в который потом ляжет остальной продукт.

Ориентир: открытый Traycer (клиенты + CLI + protocol). Их Host закрыт.
Мы пишем и клиент, и host на Rust. Повторяем границы и сущности, не копируем TS.

Источники: [traycerai/traycer](https://github.com/traycerai/traycer), [docs.traycer.ai](https://docs.traycer.ai/).

---

## 1. Инварианты (не спорить на каждом PR)

1. Host владеет живой работой: workspace-папки, файлы, git, терминалы, агенты, БД.
2. GUI не спавнит host и не проксирует RPC. GUI ходит на localhost HTTP/WS после discovery.
3. CLI ставит, стартует, стопает и диагностирует host.
4. `hostId` каноничен. Отдельного `deviceId` нет. «Устройство» — только UI-копия.
5. Вкладка/сессия привязана к `hostId` на всю жизнь. Cross-host = clone, не migrate.
6. Три версии живут отдельно: semver крейта, per-method RPC `{major,minor}`, persistence `{major,minor}`.
7. Chat-транскрипт — данные Task (переживает закрытие UI). Terminal-транскрипт принадлежит host и PTY (после MVP).

---

## 2. Карта крейтов

Один cargo workspace `rusttraycer`. Префикс крейтов `rt-`.

```
rusttraycer/
  Cargo.toml
  crates/
    rt-protocol/   # wire-контракт: типы, RPC, версии. Единственный источник правды между GUI/CLI/host
    rt-host/       # демон: HTTP/WS, домен, оркестрация агентов
    rt-storage/    # rusqlite + миграции. Только host
    rt-runtime/    # адаптеры coding agents. Владеет Integration; host только вызывает
    rt-cli/        # clap: install/start/stop/doctor/auth
    rt-gui/        # desktop (eframe + egui). Не знает про sqlite и pty
```

Зависимости:

```
rt-gui ----\
            +--> rt-protocol
rt-cli ----/
rt-host ----> rt-protocol, rt-storage, rt-runtime
rt-storage -> (не зависит от protocol-RPC, только от persistence-типов если вынесем)
rt-runtime -> rt-protocol (AgentId, события)
```

Правила границ:

- `rt-gui` не линкует `rt-storage` и `rt-runtime`.
- `rt-cli` не линкует `rt-storage`. Жизненный цикл host — через процессы и pid-файл, не через прямую БД.
- Новые RPC появляются сначала в `rt-protocol`, потом в host, потом в клиентах.

Библиотеки (MVP):

| Слой | Выбор | Почему |
|---|---|---|
| async | tokio | де-факто runtime |
| HTTP/WS | axum | простой localhost server + WS |
| сериализация | serde + serde_json | протокол человекочитаемый на старте |
| БД | rusqlite + rusqlite_migration | host-local, один писатель |
| CLI | clap | |
| GUI | eframe + egui | плотный IDE-layout (панели, деревья, диффы) быстрее, чем iced |
| id | uuid (v7) | сортируемые, годны как PK |
| ошибки | thiserror / anyhow на границах | |
| логи | tracing | |
| конфиг путей | directories | `~/.rusttraycer/` |

iced оставляем как отвергнутый вариант: красивее нативные виджеты, хуже для докинга панелей и быстрого MVP. Пересмотр только если GUI упрётся в egui.

---

## 3. Процессы и discovery

```
rt-cli start
  -> пишет ~/.rusttraycer/host/pid.json
  -> слушает 127.0.0.1:<port>
  -> БД ~/.rusttraycer/host/host.db
  -> лог ~/.rusttraycer/host/host.log

rt-gui
  -> читает pid.json
  -> handshake по HTTP
  -> события по WS
  -> не держит дочерний процесс host
```

`pid.json` (черновик):

```json
{
  "hostId": "0191f0c6-...",
  "pid": 12345,
  "rpcUrl": "http://127.0.0.1:47800",
  "wsUrl": "ws://127.0.0.1:47800/ws",
  "startedAt": "2026-08-17T10:50:00Z",
  "protocol": { "crate": "0.1.0" }
}
```

MVP: один host на машину. Несколько host (ноут + workstation) — после MVP, модель уже это допускает.

---

## 4. Модель данных

### Слои

- **Durable (БД host, потом когда-нибудь sync):** Task, Agent (мета), Chat transcript, Artifact, привязки workspace.
- **Live (только этот host):** PTY, file watch, git status, running child processes, Terminal transcript.

### Сущности MVP

```
Host
  id: HostId
  name: String
  created_at

Workspace
  id: WorkspaceId
  host_id: HostId
  path: PathBuf          # абсолютный путь на этой машине
  name: String

Task
  id: TaskId
  title: String
  status: Open | Archived
  created_at, updated_at
  workspace_ids: Vec<WorkspaceId>   # MVP: ровно 1

Agent
  id: AgentId
  task_id: TaskId
  host_id: HostId        # bind на жизнь
  parent_id: Option<AgentId>  # колонка есть, в MVP не используем
  interface: Chat        # Terminal после MVP
  provider: ProviderId   # "cli.generic" в MVP
  status: Idle | Running | Error
  run_location: Local    # Worktree после MVP
  created_at

Message                  # Context / transcript
  id: MessageId
  agent_id: AgentId
  role: User | Assistant | System | Tool
  content: String
  created_at
```

### После MVP (поля и таблицы закладываем, RPC не делаем)

- `Artifact` (Spec, Ticket, Story, Review)
- `Worktree` + `RunLocation = Local | NewWorktree | ExistingWorktree`
- `TerminalSession` (не агент)
- `CommentThread`
- A2A: reference / transcript / deliver — три разные capability, как у Traycer
- Cloud sync durable-данных

### Context

`Context` — не отдельная таблица. Это проекция:

- transcript агента (`Message[]`)
- приложенные артефакты и файлы (после MVP)
- ссылки на других агентов в Task (после MVP)

На проводе метод `agent.get_context` может собирать этот снимок. Храним нормализовано.

---

## 5. Протокол client ↔ host

Транспорт: JSON, HTTP для request/response, WS для событий.

Handshake (первый вызов):

```
ClientHello
  client: "gui" | "cli"
  client_version: semver
  methods: { "task.create": { major: 1, minor: 0 }, ... }

ServerHello
  host_id
  host_version
  accepted: { "task.create": { major: 1, minor: 0 }, ... }
  rejected: { "artifact.create": { reason: "unsupported" } }
```

Совместимость метода: major должен совпасть, minor клиента ≤ minor host.
Новый optional-поле = minor++. Ломающее изменение = major++ и новый метод, старый живёт до выпила.

### RPC MVP

| Метод | Назначение |
|---|---|
| `handshake` | см. выше |
| `host.ping` | liveness |
| `host.doctor` | пути, БД, провайдеры |
| `workspace.list` | |
| `workspace.add` | path |
| `task.list` | фильтр status |
| `task.create` | title, workspace_id |
| `task.get` | |
| `task.rename` | |
| `task.archive` | |
| `agent.list` | task_id |
| `agent.create` | task_id, provider |
| `agent.get` | |
| `agent.send` | agent_id, content |
| `agent.subscribe` | WS-подписка на события агента/таска |

### События WS

- `agent.message` (append)
- `agent.status`
- `task.updated`
- `host.going_away`

### Persistence versions

Отдельно от RPC. Таблица `schema_meta(record, major, minor)`.
MVP: один глобальный schema version в миграциях rusqlite. Per-record версии — когда появится sync.

---

## 6. Границы MVP vs следующий контур

MVP должен закрыть петлю: открыл папку → создал Task → создал агента → отправил сообщение → увидел ответ и транскрипт после рестарта.

Входит:

- один local host
- workspace folder
- Task
- Chat-агент
- один адаптер `cli.generic` (произвольный coding-agent CLI)
- persist транскрипта
- простой GUI: список тасков, чат, дерево файлов read-only

Не входит (явно):

- Terminal-агенты и PTY
- worktrees
- artifacts / comments / sharing
- agent-to-agent
- cloud sync, аккаунты, биллинг
- несколько host в одном GUI
- git diff как отдельная поверхность (достаточно `git status` в doctor)

---

## 7. ADR

### ADR-0001 — Host владеет I/O и БД

Контекст: Traycer Desktop (Electron) не тащит sqlite и не проксирует host RPC.
Решение: то же. `rt-gui` — тонкий клиент. `rt-host` — единственный писатель `host.db`.
Следствие: CLI не ходит в БД напрямую. Рестарт GUI не убивает агентов (когда они появятся как процессы).

### ADR-0002 — Три системы версий

Как у `@traycer/protocol`. semver крейта ≠ RPC schema ≠ persistence schema.
Handshake торгует per-method `{major,minor}`.

### ADR-0003 — egui, не iced

MVP — плотный tool UI. egui дешевле для панелей, деревьев, виртуализированных списков.
Пересмотр: если появится требование «нативный look» как отдельная цель, не как вкусовщина.

### ADR-0004 — Chat transcript в БД host

Chat — durable Task data. Нужен, чтобы пережить рестарт GUI и потом (не в MVP) читаться с другого host.
Terminal transcript, когда появится, не кладём в ту же корзину: он live/host-bound.

### ADR-0005 — Clone, не migrate

Сессия агента привязана к `hostId`. Смена host = новый агент-клон, не перенос PTY/процессов.

### ADR-0006 — Local-first, cloud нет в схеме MVP

Поля под sync не выдумываем заранее (нет `sync_rev` / `tombstone` в v0).
Когда sync понадобится — отдельный ADR и persistence major.

---

## 8. Что писать следующим

1. `protocol-v0.md` — точные JSON-схемы handshake, task.*, agent.*, события.
2. `storage-v0.md` — таблицы SQLite и миграция 0001.
3. GUI IA — три экрана MVP (список Task, Task canvas, settings/host).

---

## 9. Уточнения после сверки с Traycer Desktop 3.0

Не ломают v0. Зафиксировать, чтобы модель не сплющилась.

### Четыре типа, не два

| Тип | Что это | MVP |
|---|---|---|
| **Agent** | Долгая сессия внутри Task | да |
| **Harness** | Провайдер, который её крутит (`cli.generic`, потом claude/codex/…) | одно значение `cli.generic` |
| **Interface** | Chat или Terminal | только Chat |
| **Shell** | Голый PTY, не агент | нет |

Поле `provider` на проводе в MVP — это id харнесса. Отдельной таблицы Harness нет.

Chat: model/permissions можно менять на следующий turn (когда появятся).
Terminal: workspace, harness, args фиксируются на старте. Resume — через session id харнесса, не через scrollback.

### Именование

У Traycer UI = Task, protocol/CLI до сих пор epic. У нас везде **Task**. Никакого `epic` в RPC и БД.

### Permissions (после MVP, колонка не нужна сейчас)

Supervised / Auto-accept edits / Full access. Лестница на turn, который может писать или exec. В MVP `cli.generic` бежит as-is, без нашей лестницы.

### Не IDE

File tree + позже diff + «открыть во внешнем редакторе». Редактора файлов в `rt-gui` нет.

### Artifacts (после MVP)

Spec/Review — контекст, без status/assignee.
Ticket/Story — работа, Todo → In Progress → Done, assignee да.
Удаление родителя сносит вложенные артефакты, не агентов (агенты поднимаются).

### A2A (после MVP)

Три capability: reference ⊃ transcript ⊃ delivery. Cross-host deliver = reject, не очередь.

### Cloud

BYOA/local-first. Sync и свой inference — отдельные контуры, не в схеме MVP.

### Extension не копируем

Plan/Phases/YOLO/handoff — старый продукт. Desktop 3.0 держит intent в Task + artifacts + agents. Мы там.
