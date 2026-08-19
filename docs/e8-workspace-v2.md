# E8 — Workspace / plan-layer (v2), Ф5

Для: Core (host читает файлы + RPC), UI (пресеты, roles, settings guide). Integration не трогать.
От: Architect. Дата: 2026-08-19. Не код.
База: ADR-0004; brief №14, №16; directive E8; matrix C52, C54–C56; live docs Agent selection + AGENTS.md.
Протокол: minor bump **1.7**. 1.0–1.6 не ломать. Конверт camelCase. Storage: миграция **0008**. 0001–0007 **байтово**.

## Закон ADR-0004

1. **Не копировать** extension Phase / Plan Mode / Review Mode / Epic boards / mermaid workflows / YOLO-as-automation. UI и protocol говорят **Task**. Epic Mode не воскрешать.
2. «Review» = artifact + git (E5), не extension Review Mode.
3. C54: host читает workspace `AGENTS.md` как контекст агента. Не GUI sqlite. Не копия тела в host.db.
4. C55: global selection guide на Current Host + optional `<ws>/.traycer/agent-selection-guide.md`. Workspace **уточняет / перекрывает** global (live [Agent selection](https://docs.traycer.ai/settings/agents.md)).
5. C56: четыре локальных пресета `planning` / `review` / `debug` / `document` — шаблоны Task/агента, не boards.
6. C52 roles — **E8 must** (e6/e7 отдали сюда). Метка агента, не новая `messages.role` (CHECK 0001 не трогать).
7. Секреты не в host.db (ADR-0005 / C74). Лестницу / A2A / artifacts / E7 switch **не открывать**.
8. E9 sync гайдов — не эта спека.

## Решение C52 / C54–C56 (закон)

| ID | Ф5 |
|---|---|
| C54 read `AGENTS.md` | **must**, host FS, не sqlite |
| C55 selection guide global + ws file | **must**, файлы, не sqlite |
| C56 presets planning/review/debug/document | **must**, built-in, не boards |
| C52 agent roles | **must** |
| Nested AGENTS.md walk (monorepo) | **later** |
| User-defined presets | **later** |
| Settings «disable AGENTS.md» | **later** |

Матрица: C52, C54–C56 wave **Ф5**. Nested / custom presets / disable — later.

## Что есть

- `agent.get_context` → `{ messages }` только. AGENTS.md нет.
- `task.create { title, workspaceId }`. Пресет нет.
- `Agent` без `role`. `Message.role` = user/assistant/system/tool.
- Host не читает `AGENTS.md` и `.traycer/`.
- Settings guide нет. Global файл нет.

## Storage 0008

Не править файлы 0001–0007. Только новая миграция:

```sql
ALTER TABLE agents ADD COLUMN role TEXT NOT NULL DEFAULT 'coder';
ALTER TABLE tasks ADD COLUMN preset TEXT;
```

`role` ∈ `coder` | `planner` | `reviewer` | `debugger` | `documenter` (check в host).
`preset` ∈ `planning` | `review` | `debug` | `document` или NULL.

Нет колонок под markdown гайдов, API keys, Phase, Epic. Тела `AGENTS.md` / selection guide **не** писать в sqlite.

Global guide path (host data dir, рядом с host.db): `agent-selection-guide.md`.
Workspace: `<workspacePath>/AGENTS.md` и `<workspacePath>/.traycer/agent-selection-guide.md`.

## Protocol 1.7

```
workspace.guides.get   1.7
settings.guide.get     1.7
settings.guide.set     1.7
preset.list            1.7
agent.update           1.7
```

Адitive на живых методах (minor 1.7 у host; старый клиент поля игнорит):

- `task.create` — optional `preset`
- `task.get` / list — optional `preset` на `Task`
- `agent.create` — optional `role`
- `agent.get` / list — optional `role` на `Agent`

`agent.get_context` **не** расширять: transcript only. Гайды не становятся `Message`.

Клиент без 1.7: 1.6 switch/profiles / 1.5 a2a / 1.4 artifact / pty / write живы. Новые методы не в `accepted`.

### `workspace.guides.get`

```json
{ "workspaceId": "…" }
```

ok:

```json
{
  "agentsMd": { "path": "/ws/AGENTS.md", "content": "…", "truncated": false },
  "workspaceGuide": null,
  "globalGuide": { "path": "…/agent-selection-guide.md", "content": "…", "truncated": false }
}
```

Каждое из трёх: объект или `null` (файла нет / не файл). Читать только workspace **root** `AGENTS.md` (не walk вверх, не nested). Cap **65536** байт UTF-8; длиннее — обрезать, `truncated=true`. Нет файла — `null`, не ошибка. `not_found` только если нет workspace.

Писать `AGENTS.md` / ws guide RPC **нет**. Редактирует пользователь в редакторе.

### `settings.guide.get` / `set`

get → `{ path, content, truncated }` или `{ path, content: "", truncated: false }` если файла нет.

set `{ content }` (0…65536) → атомарно пишет global файл. Пустая строка = пустой файл, не delete. Не sqlite.

### `preset.list`

```json
{
  "items": [
    { "id": "planning", "title": "Planning", "defaultRole": "planner" },
    { "id": "review",   "title": "Review",   "defaultRole": "reviewer" },
    { "id": "debug",    "title": "Debug",    "defaultRole": "debugger" },
    { "id": "document", "title": "Document", "defaultRole": "documenter" }
  ]
}
```

Ровно эти четыре, порядок фиксирован. Не user CRUD. Не kanban.

`task.create { title, workspaceId, preset? }`: невалидный preset → `invalid_params`. title по-прежнему 1…200 (GUI подставляет hint). Host пишет `tasks.preset`.

### Roles — `agent.create` / `agent.update`

`role` default: если omitted на create → `tasks.preset.defaultRole` если preset есть, иначе `coder`. Явный `role` побеждает. Вне набора → `invalid_params`.

`agent.update { agentId, role }` — тот же `agentId`, transcript жив. Не клон. Provider/model не менять (это E7 `agent.switch`).

Role = метка + prefix в **turn inject** (ниже). Не меняет allowlist harness.

### Turn inject (host, не GUI)

На `agent.send` (и spawn child / A2A child create уже существующим путём) host **собирает преамбулу в runtime request**, не в `messages` таблице:

1. `AGENTS.md` root, если есть
2. global selection guide, если непустой
3. workspace selection guide, если есть (последний = override)
4. role prefix: коротко кто этот агент (`planner` / …)

Порядок 1→4. Не yolo. Не `kind=edit|exec`. Не ladder. Клиент `get_context` этих строк не видит.

## GUI минимум

- New Task: пикер 4 пресетов (можно None). Не Phase stepper.
- New / existing agent: пикер role. Не account switcher.
- Settings: textarea global selection guide (`settings.guide.*`).
- На канвасе workspace: chip `AGENTS.md` / guide present|missing из `workspace.guides.get`.
- Review пресет не открывает PR view и не клонирует Review Mode.
- Нет поля API key. Нет Epic / boards / mermaid engine.

## Вне скоупа

- C48–C51, C53 (E7 / later)
- C66 named extra harnesses, C67 inference
- E9 sync гайдов
- Nested AGENTS.md, disable-detection toggle
- User-defined presets, Phase/Epic/YOLO RPC
- Тела гайдов в sqlite, секреты в db
- Писать workspace файлы через RPC

## Приёмка Ф5 / E8

1. Root `AGENTS.md` есть → send turn уходит с преамбулой; `get_context.messages` без этого текста.
2. Файла нет → send жив, `guides.agentsMd` = null.
3. Global set → файл на диске рядом с host.db; в sqlite нет этой строки.
4. Оба guide есть → inject содержит оба, workspace после global.
5. `task.create` + `preset=planning` → `Task.preset=planning`, не board, не epic.
6. `agent.create` без role на таком Task → `role=planner`. `agent.update` на `reviewer` → тот же `agentId`, messages целы.
7. Клиент без 1.7: create / send / get_context / switch живы.
8. 0001–0007 байтово целы. Нет Phase/Epic методов.

Код — следующие STAR. E9 не открывать из этого файла.
