# E1 — Canvas parity (v2)

Для: UI (`rt-gui`), Core (`host.doctor` / `agent.*` уже есть).
От: Architect. Дата: 2026-08-19. Не код.
База: `docs/parity-matrix.md` C18–C22, C63, C65; `docs/directive-v2.md` E1; `docs/gui-ia-v0.md`.
Факт: host уже принимает N агентов и три harness. GUI v1: один `cli.generic`, `can_create_agent` = пустой список.

## Цель Ф1

Canvas перестаёт врать про MVP. Пикер и список агентов читают host, не константы.

### В Ф1 (must)

| ID | Что |
|---|---|
| C18 | Пикер harness при `agent.create`: allowlist + caps + `available` с `host.doctor`, **не** хардкод `"cli.generic"`. Disabled, если `available=false` (create всё равно разрешён host'ом — GUI предупреждает, не блокирует, как host). |
| C19 | N агентов на Task: список, выбор, статусы turn (`idle` / `running` / `error`). Снять «один агент на задачу». `agent.list` уже есть. |
| C22a | **Task tabs:** несколько открытых Task в одном окне (вкладки). Не workspace sub-tabs. |
| C20 | **Минимальный split:** два pane, в каждом — уже существующий вид (Task canvas / git / files / host). Divider. Без Chrome-drag-to-edge. |

### Later (не Ф1)

| ID | Почему later |
|---|---|
| C21 | Поиск Tasks/artifacts по branch/folder/PR — нет artifacts (E5), отдельная поверхность. |
| C22b | Workspace sub-tabs / несколько folder на Task — E8/E1-polish. |
| C63 | Resource monitor, notification hooks, prompt stash, drag-to-tile — 1.1.10 chrome, не каркас canvas. |
| C65 | Worktree cleanup / PR context / branch prefix — isolate+`worktree.ensure` уже есть; cleanup = E1-polish / E3 PR. |

## Пикер (C18)

Источник опций: `host.doctor.providers[]` (`id`, `available`, `detail`) плюс `caps` (host должен отдать `HarnessCaps` в doctor — сейчас Core отдаёт id/available/detail; **Ф1: добавить объект `caps` в provider**, без нового RPC).

Caps на проводе (уже в `rt-runtime::HarnessCaps`), camelCase:

```json
{
  "id": "cli.claude",
  "available": true,
  "detail": "/usr/bin/claude",
  "caps": {
    "oneShot": true,
    "longLived": false,
    "streamTokens": true,
    "tools": false,
    "sessionResume": false,
    "a2aInbox": false,
    "pty": false,
    "needsApiKey": false,
    "apiKeyEnv": null
  }
}
```

GUI не содержит списка `cli.*`. Новый harness в host → появляется сам. `pty`/`a2aInbox` в Ф1 только индикаторы (серые), не включают E4/E6.

`agent.create` params без изменения конверта: `{ taskId, provider }`. `provider` обязателен из пикера (дефолт GUI больше не подставляет вслепую; если doctor пуст — ошибка UI).

## N агентов (C19)

- Список агентов выбранного Task (`agent.list`).
- Клик = selected `agentId`; chat/git/worktree относятся к нему.
- Статус: `Agent.status` + WS `agent.status` (уже есть). Stop (`agent.cancel`) на **выбранном running**.
- Второй `agent.create` разрешён, пока host принимает.
- Переименование/архив агента — не Ф1.

## Tabs + split

- Task tabs: open / close / switch. Состояние selected agent per Task — в памяти GUI (не RPC).
- Split: `left` / `right` pane ids из `{ canvas, git, files, host }`. Persist layout в GUI only (не host.db).
- GUI по-прежнему **не** спавнит host.

## Вне скоупа E1

Смена harness на существующем агенте — ADR-0007 / E7.
Child agents, A2A — E6.
ПTY/terminal pane — E4.

## Приёмка Ф1

1. Doctor с тремя provider → пикер показывает три, caps видны.
2. Два агента разных provider на одном Task; switch меняет transcript.
3. Две Task-вкладки; split canvas|git.
4. Нет строки «Провайдер MVP: cli.generic. Один агент на задачу.»
5. C21/C63/C65 не реализованы — и не обещаны в README.
