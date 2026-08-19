> **v1.0 reader:** this file is the original draft. cli.claude and cli.codex are implemented; generic wire is unchanged. See [v1-delta.md](v1-delta.md).

# Runtime adapters — спецификация v0

Для: Core (`rt-host` вызывает trait), Architect (`rt-protocol` типы).
От: Integration. Дата: 2026-08-17.
Статус: действующий контракт. Закрывает открытый пункт Architect про wire `cli.generic`.
Обзор продукта: `architecture-v0.md`. Host: `host-runtime-v0.md`.

Это не код. Именованные харнессы после MVP не реализуем — только резервируем id и швы.

---

## 0. Зона Integration

Пишу я:

- trait `AgentBackend` и все его реализации в `rt-runtime`
- wire `cli.generic` (наш протокол, не вендорский)
- позже: `cli.claude`, `cli.codex`, `cli.cursor`, `cli.opencode`, `native`
- позже: сборка shared context и A2A-типы на проводе

Не пишу:

- supervisor, HTTP/WS, БД (`rt-host`, `rt-storage`) — Core
- GUI, CLI процесса
- `rt-protocol` как крейт — Architect. Я предлагаю типы turn/harness, он кладёт их в крейт

`rt-runtime` не открывает БД и не ходит в сеть сам. Сеть делает только дочерний CLI вендора.

---

## 1. Инварианты

1. `Agent.provider` на проводе = `HarnessId`. Не тип агента, не interface, не shell.
2. Один trait на все харнессы. Capability — данные (`HarnessCaps`), не ветвление в supervisor.
3. Именованный харнесс говорит **нативным** протоколом вендора. Он не прогоняется через `cli.generic`.
4. `cli.generic` — наш минимальный протокол для произвольного CLI и для моков. Один JSON на stdin, EOF, сырой текст на stdout.
5. Host отдаёт полный transcript в `TurnRequest.messages`. Адаптер не читает БД.
6. Адаптер эмитит только `TurnEvent`. Supervisor пишет Message и шлёт WS.
7. Один активный turn на агента — это правило host, не runtime.
8. Kill child на shutdown host и на timeout (10 минут). Отмены RPC в MVP нет.

---

## 2. Trait (без изменений семантики Architect)

```
trait AgentBackend: Send + Sync {
  fn id(&self) -> &'static str;
  fn available(&self) -> Availability;
  fn caps(&self) -> HarnessCaps;          // NEW, см. §5. MVP можно захардкодить
  fn start_turn(&self, req: TurnRequest) -> impl Stream<Item = TurnEvent> + Send;
}

Availability { available: bool, detail: String }

TurnRequest {
  agent_id: AgentId,
  task_id: TaskId,
  workspace_path: PathBuf,               // абсолютный, cwd child
  messages: Vec<WireMessage>,            // полный transcript, включая свежий user
  extra_env: BTreeMap<String, String>
}

WireMessage { role: User | Assistant | System | Tool, content: String }

TurnEvent {
  Token { text } |
  Tool { name, payload } |               // MVP: generic не эмитит
  Finished { exit_code: i32 } |
  Failed { message: String }
}
```

`caps()` добавляю сейчас, чтобы второй харнесс не ломал supervisor. В MVP Core может игнорировать поле.

---

## 3. Wire `cli.generic` — зафиксировано

Единственный backend MVP. Id: `"cli.generic"`.

### Spawn

- Бинарь: env `RUSTTRAYCER_GENERIC_CMD` (обязателен для `available=true`) — **один путь** к executable, без split. Args: `RUSTTRAYCER_GENERIC_ARGS` = JSON-массив строк, иначе unset/пусто. Без shell.
- cwd = `workspace_path`
- env: процесс host + `extra_env` + `RUSTTRAYCER_AGENT_ID`, `RUSTTRAYCER_TASK_ID`
- stdin: один JSON-объект `{"messages":[...]}` + `\n` + close stdin (EOF). Как host-runtime-v0 §3 и protocol-v0 §6. Не NDJSON, stdin не держим.
- stdout: UTF-8 текст. Любой кусок = `Token { text }`. Не JSON.
- stderr: только `tracing::warn`, в transcript не попадает.
- exit 0 → `Finished { 0 }`. иначе `Failed { "exit {code}" }`.
- timeout 10 мин → kill + `Failed { "timeout" }`.
- kill group на shutdown (process group, не только pid).

### Stdin JSON

Совпадает с Architect. Не расширяем объект в MVP — `agentId` / `taskId` уже в env, путь уже cwd.

```json
{
  "messages": [
    { "role": "user", "content": "..." }
  ]
}
```

`role`: `user` | `assistant` | `system` | `tool`.
Поля camelCase. Лишние поля child игнорирует. Нет `version`.

Это **наш** контракт, не вендорский. Мок для тестов:

```
#!/bin/sh
# читает JSON, печатает content последнего user
python3 -c 'import json,sys; m=json.load(sys.stdin)["messages"];
print(next(x["content"] for x in reversed(m) if x["role"]=="user"))'
```

`RUSTTRAYCER_GENERIC_CMD=/path/to/this` + `agent.send` = петля MVP.

Не парсим tool-calls. Не читаем session id. Не ходим в сеть.

---

## 4. Зарезервированные HarnessId (не реализовывать в MVP)

`available()` для них — probe PATH / env. В MVP `host.doctor` отдаёт только `cli.generic`. Остальные можно не регистрировать.

| id | Бинарь | Как будем звать (черновик, не код) | Транспорт вендора |
|---|---|---|---|
| `cli.generic` | `$RUSTTRAYCER_GENERIC_CMD` | наш JSON + EOF / stdout text | наш |
| `cli.claude` | `claude` | `claude -p --output-format stream-json --verbose` ; позже `--input-format stream-json` | Anthropic stream-json / Agent SDK |
| `cli.codex` | `codex` | сначала `codex exec --json` (one-shot); потом `codex app-server --stdio` | JSON-RPC JSONL |
| `cli.cursor` | `agent` | `agent -p --output-format stream-json` + `CURSOR_API_KEY` | Cursor print/stream-json |
| `cli.opencode` | `opencode` | `opencode run --format json` ; позже `opencode acp` | JSON events / ACP nd-JSON |
| `native` | нет child | in-process inference в `rt-runtime` | нет stdin |

Правило: адаптер **переводит** вендорский поток в `TurnEvent`. Supervisor вендорский JSON не видит.

Имена id стабильны. Синонимов (`claude-code`, `cursor-cli`) нет.

`native` — отдельный контур (свой inference), не «ещё один CLI». Появится своей спекой: загрузка модели, очередь, отмена. В MVP его нет даже как заглушки в doctor.

---

## 5. Capability matrix

Не таблица в БД. Константа на backend. Когда Integration принесёт второй харнесс — supervisor читает это, а не `match provider`.

```
HarnessCaps {
  one_shot: bool,            // процесс на один turn (generic, exec, -p)
  long_lived: bool,          // app-server / acp / stream-json session
  stream_tokens: bool,
  tools: bool,               // эмитит TurnEvent::Tool
  session_resume: bool,      // vendor session id, не наш scrollback
  a2a_inbox: bool,
  pty: bool,
  needs_api_key: bool,
  api_key_env: Option<&'static str>,
}
```

MVP `cli.generic`:

```
one_shot=true, long_lived=false, stream_tokens=true,
tools=false, session_resume=false, a2a_inbox=false, pty=false,
needs_api_key=false, api_key_env=None
```

Черновик (не обещание, проверять при реализации):

| cap | generic | claude | codex | cursor | opencode | native |
|---|---|---|---|---|---|---|
| one_shot | да | да (`-p`) | да (`exec`) | да (`-p`) | да (`run`) | да |
| long_lived | нет | да (SDK stdin) | да (app-server) | позже | да (`acp`) | нет |
| stream_tokens | да | да | да | да | да | да |
| tools | нет | да | да | да | да | позже |
| session_resume | нет | да | да | `--resume` | `-s` / `-c` | нет |
| a2a_inbox | нет | нет | нет | нет | нет | нет |
| pty | нет | нет | нет | нет | нет | нет |
| api_key_env | — | (claude login) | (codex login) | `CURSOR_API_KEY` | (opencode auth) | — |

Permissions ladder Traycer (Supervised / Auto-accept / Full) — не runtime. Если вендор имеет `--force` / `--yolo` / sandbox, адаптер прокинет флаги позже. В MVP не прокидываем.

---

## 6. Shared context

MVP: context = `TurnRequest.messages`. Host уже отдаёт это через `agent.get_context` → `{ messages }`. Runtime ничего не собирает.

После MVP (своя спека, не делать сейчас):

```
ContextSnapshot {
  messages: Vec<WireMessage>,
  artifacts: Vec<ArtifactRef>,     // когда появятся
  peers: Vec<AgentRef>,            // другие агенты того же Task
  files: Vec<FileRef>              // явные вложения, не весь workspace
}
```

Между моделями / харнессами контекст общий на уровне Task, не на уровне процесса вендора. Клон на другой host = новый агент, context копируется как данные, session id вендора нет.

---

## 7. A2A (после MVP)

Три capability, как у Traycer: `reference ⊃ transcript ⊃ delivery`.
Cross-host deliver = reject, не очередь.
Runtime даст типы сообщений и inbox-cap на харнессе. Доставку делает host.

В MVP: колонки/RPC/событий нет. Не резервировать поля в `TurnRequest`.

---

## 8. Doctor

`host.doctor.providers` в MVP:

```
[{ "id": "cli.generic", "available": bool, "detail": "RUSTTRAYCER_GENERIC_CMD unset" | "/abs/cmd" }]
```

`available=false` не блокирует `agent.create` (protocol-v0). Падает на `agent.send` (`internal` / `Failed`).

---

## 9. Definition of done (Integration, MVP)

1. Этот файл — контракт wire `cli.generic`.
2. Когда появится репо: `rt-runtime` с `AgentBackend` + `cli.generic` + мок-бинарь в тестах.
3. Тест: stdin `{"messages":[...]}` + newline → stdout text → `Token+` + `Finished`.
4. Тест: unset env → `available=false`.
5. Тест: timeout / nonzero exit → `Failed`.
6. Именованные харнессы и `native` — не в дереве, только id в этом файле.

---

## 10. Решено / открыто

Решено:

- wire `cli.generic`: `{"messages":[...]}` + newline + EOF, stdout = text (как host-runtime-v0 / protocol-v0)
- именованные харнессы говорят нативно, не через generic
- зарезервированные id: `cli.claude` `cli.codex` `cli.cursor` `cli.opencode` `native`
- `HarnessCaps` на trait сейчас, реализация matrix — со вторым харнессом
- shared context и A2A не в MVP
- Integration владеет `rt-runtime`, Core только вызывает
- `RUSTTRAYCER_GENERIC_CMD` = один путь; args = `RUSTTRAYCER_GENERIC_ARGS`
- `available=false` не блокирует create

Закрыто Architect в §11:

- `RUSTTRAYCER_GENERIC_CMD` = один путь; args = `RUSTTRAYCER_GENERIC_ARGS` JSON-массив
- `available=false` не блокирует create, падает send (`internal` + detail)
- `TurnEvent::Stderr` не заводим
- первый именованный харнесс после MVP — `cli.claude`, не сейчас

---

## 11. Принято Architect (2026-08-17)

Швы ок. Два уточнения, считать локами:

1. **`RUSTTRAYCER_GENERIC_CMD`** — один путь к executable, без split по пробелам. Аргументы, если нужны: `RUSTTRAYCER_GENERIC_ARGS` = JSON-массив строк (`["--flag","value"]`). Нет args → пустой / unset. Whitespace-split не делаем: сломается на пути с пробелом.

2. **`available=false`** не блокирует `agent.create`. Create пишет row. Падает `agent.send` (`internal` + `detail` из `available()`). Doctor показывает правду. Не `invalid_params` на create.

`TurnEvent::Stderr` — нет, не заводим.
Первый именованный харнесс после MVP — `cli.claude`, когда скажем. Не начинать сейчас.

Core вызывает trait, не реализует backends. `caps()` в MVP игнорирует.
