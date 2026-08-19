# E4 — Terminal (v2), Ф3

Для: Core (host/pty/mux/protocol/storage), UI (панели), Integration (caps + resume argv).
От: Architect. Дата: 2026-08-19. Не код.
База: brief №1, №2, №4, №13; [e2-ladder-v2](e2-ladder-v2.md) `kind=exec`; matrix C32–C37; directive E4; live [Terminal Agents VS Terminals](https://docs.traycer.ai/concepts/terminal-agents-vs-terminals.md) (2026-08-19).
Протокол: minor bump **1.3**. 1.0–1.2 не ломать. Конверт camelCase. Storage: миграция **0004** (0001–0003 не трогать).

## Закон

1. Четыре типа, не два (brief №4): **Agent** ≠ **Harness** ≠ **Interface** ≠ **Shell**.
2. Interface агента: `chat` | `terminal`. Shell — **не** агент и не interface.
3. Live PTY / scrollback — только этот host, **не** в `messages`, **не** durable (brief №1, №2). Chat можно потом sync. PTY — нет.
4. Resume Terminal-агента — **provider session id**, не replay нашего scrollback (brief №13).
5. Spawn PTY для Terminal-агента — лестница `kind=exec` (тот же `agent.approval` / `approval.respond`). Новых ladder-методов нет.
6. GUI **не** спавнит PTY. I/O у host. `pty.rs` / `mux.rs` перестают быть пустыми.
7. Не копировать extension (Phase / YOLO-automation / Epic). UI говорит Task.

## Решение по C37 (закон)

**C37 Terminals outside a Task / start without folder — later, не Ф3.**

Это chrome 1.1.x (changelog), не фундамент четырёх типов. У нас workspace обязателен, host владеет FS/PTY. Терминал без Task/folder ломает корень cwd и лестницу.

- Матрица: epic E4, wave **later**.
- Ф3: каждый Shell и каждый Terminal-агент принадлежит **Task** с workspace (или worktree).
- Нет RPC без `taskId`. Нет «start screen terminal».

## Что уже есть

| Есть | Где |
|---|---|
| `Agent.interface` всегда `"chat"` | protocol 1.0, storage CHECK |
| `HarnessCaps.pty` / `sessionResume` (сейчас false) | doctor / runtime |
| Пустые `pty.rs` / `mux.rs` | host, без deps |
| Лестница 1.1 `kind=exec` на `agent.send` | e2-ladder-v2 |
| Write path 1.2 | e3-write-v2 |
| v1: «PTY не делать» | снято этой спекой |

## Четыре типа (C32 / C33)

| Тип | Что | Где живёт |
|---|---|---|
| Agent | Долгая сессия в Task | таблица `agents` |
| Harness | `cli.claude` / `cli.codex` / `cli.generic` | `provider` |
| Interface | `chat` или `terminal` | `agents.interface` |
| Shell | Голый PTY пользователя | live mux; **не** `agents` |

**Terminal Agent** (эталон): тот же агент, interface=`terminal`. Живёт в панели Agents, не в Terminals. Получает Task context (task id, agent id). Артефакты / A2A / skills — E5/E6, не Ф3.

Launch фиксируется на старте: workspace или worktree, harness, optional `launchArgs`. Смена = новый агент (ADR-0007 / E7 — смена harness на том же агенте, не здесь).

Кто может `interface=terminal`: только harness с `caps.pty=true`. Ф3: Integration ставит `pty=true` и `sessionResume=true` на **`cli.claude` и `cli.codex`**. `cli.generic` — нет (как Traycer: generic CLI ≠ Terminal interface). OpenCode в allowlist нет — не добавляем имя (ADR-0007); появится позже через caps.

**Shell:** `$SHELL` или `/bin/bash` в cwd workspace/worktree. Нет system prompt, нет A2A, нет `agent.send`. Пользователь жмёт «New terminal» — **явное действие, карточки ask нет** (как commit/push).

`agent.send` на `interface=terminal` → `invalid_params`. Ввод в PTY = `pty.write`. Chat остаётся `agent.send`.

## Mux (C34)

`mux.rs` — таблица живых сессий **в памяти процесса**:

```
PtySession { ptyId, kind: "agent" | "shell", entityId, pid, cols, rows }
```

Несколько PTY на host. Выход не смешивается. Resize / write / close по `ptyId`. Это не tmux(1) и не зависимость GUI.

Host: `portable-pty` в `pty.rs` теперь можно (запрет MVP снят). Linux x86_64 (ADR-0001/0006). GUI — клиент байтов, не эмулятор на стороне host.

## Resume (C35) и durable vs live (C36)

| Что | Durable (sqlite) | Live (память, умирает с host) |
|---|---|---|
| Agent row + `interface` | да | — |
| `providerSessionId` (только terminal) | да | — |
| Chat `messages` | да | — |
| PTY fd / pid / **scrollback** | **нет** | да |
| Shell PTY | **нет** | да |
| Mux `ptyId` | **нет** | да |

После рестарта host:

- Chat: messages на месте, PTY нет.
- Terminal-агент: row + `providerSessionId` на месте. `pty.open` зовёт vendor resume (Integration: argv харнесса). Host **не** подсовывает сохранённый scrollback.
- Shell: живой PTY нет. `shell.create` = новый PTY в том же cwd. Resume по scrollback **запрещён**.

Тест C36 (закон): байты PTY **никогда** не INSERT в `messages`. После сессии Terminal-агента таблица messages этого агента пустая (или только то, что написал Chat — а Chat на этом агенте запрещён). После рестарта messages Chat-агента живы, live PTY нет.

Эталон: transcript Terminal-агента читается из **истории провайдера**, не из нашего scrollback — это E6 (A2A read), не Ф3.

## Storage 0004

```sql
-- agents.interface CHECK: 'chat' | 'terminal'
-- agents.provider_session_id TEXT NULL
-- CHECK: interface='chat' → provider_session_id IS NULL
```

Таблицы `shells` / `pty_sessions` **нет**. Shell не переживает рестарт как сущность (brief №2). 0001–0003 не трогать.

## Protocol 1.3

Новые методы `{major:1, minor:2}` нет — все новые **1.3**. `agent.create` получает optional поля → host minor **1.3** (старый клиент 1.0 совместим: `client.minor <= host.minor`).

```
agent.create    1.3   // + interface?, launchArgs?
shell.create    1.3
shell.list      1.3
shell.close     1.3
pty.open        1.3
pty.write       1.3
pty.resize      1.3
pty.close       1.3
```

GUI Ф3 объявляет эти методы. Клиент без 1.3: Chat/write живут, terminal RPC не в `accepted`.

Новые коды: `not_pty` (harness без caps.pty), `pty_dead` (ptyId нет / host рестартнулся). Уже есть: `denied`, `approval_expired`, `agent_busy`.

`Agent` на проводе: optional `providerSessionId` (null у chat). Старый клиент игнорирует.

### `agent.create` 1.3

Как 1.0 `{ taskId, provider }` плюс optional:

```json
{ "taskId": "…", "provider": "cli.claude", "interface": "terminal", "launchArgs": ["--foo"] }
```

`interface` default `"chat"`. `terminal` + `!caps.pty` → `not_pty`. `launchArgs` только для terminal, потолок 32 строки. Ok: `Agent` с `interface=terminal`, PTY ещё нет (idle), `providerSessionId=null`.

### `pty.open`

Params: ровно одно из `agentId` | `shellId`; `cols`, `rows` (1…500).

- `agentId` + interface=chat → `invalid_params`
- `agentId` + terminal, mode=ask → WS `agent.approval` `kind=exec`, summary вроде `spawn pty cli.claude`. deny → PTY нет. allow-once / allow-always / yolo → spawn.
- Уже живой PTY этого entity → тот же `ptyId` (идемпотентно).
- Есть `providerSessionId` и caps.sessionResume → Integration resume, ok `{ ptyId, resumed: true }`.
- Иначе новый spawn, потом host записывает `providerSessionId` когда harness его отдал. Ok `{ ptyId, resumed: false }`.

Пока висит approval, повторный open → `agent_busy`.

### `pty.write` / `pty.resize` / `pty.close`

```json
{ "ptyId": "…", "data": "<base64>" }
{ "ptyId": "…", "cols": 80, "rows": 24 }
{ "ptyId": "…" }
```

`data` raw PTY stdin, потолок 64 KiB на вызов. Нет ptyId → `pty_dead`. `pty.close` убивает child; Terminal-агент → `status=idle`, session id **не** стираем.

### `shell.create` / `list` / `close`

```json
{ "taskId": "…", "workspaceId": "…", "worktreeId": null, "cols": 80, "rows": 24 }
```

Сразу живой PTY (user action). Ok: `{ "shellId": "…", "ptyId": "…", "cwd": "…" }`. `shell.list { taskId }` — только live этого процесса. `shell.close { shellId }` убивает PTY. Без `taskId` / без workspace — нет (C37).

### WS

Формы 1.3, не торгуются per-method:

```json
{ "type": "pty.data", "ptyId": "…", "data": "<base64>" }
{ "type": "pty.exit", "ptyId": "…", "code": 0 }
```

Не писать это в `messages`. Не `agent.message`.

`agent.cancel` на Terminal-агенте с живым PTY = `pty.close` + cancel semantics v1 (owns gen).

## GUI Ф3

- Create agent: Chat | Terminal. Terminal disabled если `!caps.pty`.
- Terminal-агент: pane эмулятора (ANSI/VT — выбор UI), не composer `agent.send`.
- Панель **Terminals**: list live shells, new, close. Не показывает Terminal-агентов (они в Agents). Эталон: [panels/terminals](https://docs.traycer.ai/panels/terminals.md).
- Split Ф1: в набор pane добавить `terminal`.
- Карточка approval Ф1 для `kind=exec` на `pty.open`.
- Нет «терминал на start screen», нет открытия без folder.

## Integration Ф3

- `cli.claude` / `cli.codex`: `pty=true`, `sessionResume=true`. Как добыть/передать vendor session id — зона Integration (host только хранит строку).
- Не читать scrollback как transcript.
- `cli.generic`: оба флага false.

## Вне скоупа Ф3

- C37 outside Task / no-folder
- A2A inbox / чтение vendor history (E6)
- Artifacts в PTY (E5)
- Смена harness на том же агенте (E7)
- Windows PTY (ADR-0006)
- tmux(1), serializing scrollback в sqlite
- extension terminal automation

## Приёмка Ф3

1. `agent.create interface=terminal` + `cli.generic` → `not_pty`. + `cli.claude` при ask → `pty.open` даёт WS `kind=exec`; deny: child нет.
2. Два PTY (shell + terminal agent): `pty.data` не путает `ptyId`.
3. Байты PTY не появляются в `messages` (тест на sqlite).
4. Рестарт host: Chat messages живы; live PTY нет (`pty.write` → `pty_dead`); повторный `pty.open` terminal-агента идёт с сохранённым `providerSessionId` (`resumed=true` если Integration отдал id).
5. Shell после рестарта не в `shell.list`. Новый `shell.create` — новый PTY, не replay.
6. Нет метода создать shell без `taskId`.
7. Клиент без 1.3: Chat + 1.2 write живы.

Код — следующие STAR (Core host/protocol, UI панели, Integration caps/resume).
