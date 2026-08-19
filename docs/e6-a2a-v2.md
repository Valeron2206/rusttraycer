# E6 — A2A + Loops (v2), Ф4

Для: Core (host/protocol), UI (дерево/inbox/loop), Integration (caps.a2aInbox + vendor transcript).
От: Architect. Дата: 2026-08-19. Не код.
База: brief №7; matrix C43–C47; directive E6; architecture-v0 A2A; e2-ladder-v2; e4-terminal-v2; live [Agent-to-Agent](https://docs.traycer.ai/concepts/agent-to-agent.md) (2026-08-19).
Протокол: minor bump **1.5**. 1.0–1.4 не ломать. Конверт camelCase. Storage: миграция **0006** (только `loops`). 0001–0005 **байтово**. `agents.parent_id` уже в 0001 — не патчить.

## Закон №7

Три **разные** capability, не один trait:

| Capability | Что | Уже чем |
|---|---|---|
| **Reference** | `@` / id агента в Task | любой агент, любой interface |
| **Transcript** | прочитать историю | same user + правило host (ниже) |
| **Delivery** | доставить сообщение в inbox | same user, **оба** агента на этом host, `caps.a2aInbox` у **получателя** |

`reference ⊃ transcript ⊃ delivery`. Можно быть referenceable без inbox — это норма, не баг.

1. Delivery **local**: same `hostId`, same user. Cross-host → `cross_host`, **не очередь**.
2. Не копировать Traycer 1.1.10 «A2A requires full access» как default (C26 oos). Лестница E2: ask default, yolo явный. `a2a.deliver` и `loop.*` **не** выставляют yolo сами. Turn в loop, который спавнит exec — `kind=exec`.
3. Artifact ≠ A2A-сообщение. Inbox ≠ artifact viewer. E5 RPC не трогать.
4. Shell (C33) **не** участвует ни в одной из трёх.
5. Бесконечный цикл двух агентов = **P0**. `loop.start` без `maxIterations` → `invalid_params`. Host сам не крутит «пока не надоест».
6. Child — `agents.parent_id` (не `artifacts.parent_id`). Тот же `taskId`. Provenance, не ACL. Delete parent-агента **не** сносит child.
7. Vendor scrollback не наш transcript (e4). Terminal history — session провайдера, не `messages` и не PTY bytes.

## Решение по C43–C47 (закон)

**Все пять — Ф4 must.** Later только то, чего нет в C43–C47: fork chat, глубина nested tree, C63 stop-children.

## Caps per harness (Integration)

Сегодня в коде все `a2aInbox=false`. Ф4 включает **по провайдеру**:

| Harness | interface | reference | transcript | a2aInbox |
|---|---|---|---|---|
| `cli.claude` | chat | да | `messages` | **true** |
| `cli.claude` | terminal | да | vendor session на этом host (E6 transcript, не scrollback) | **true** (эталон Terminal inbox) |
| `cli.codex` | chat | да | `messages` | false |
| `cli.codex` | terminal | да | vendor session на этом host | **false** (эталон) |
| `cli.generic` | chat | да | `messages` | **false** |
| `cli.generic` | terminal | нет (`not_pty`) | — | — |

Эталон: на Chat у Codex/прочих есть delivery; **у нас** inbox только `cli.claude`, пока caps не сменят. Terminal delivery — только Claude. OpenCode в allowlist нет.

`a2a.deliver` на получателя без inbox → `no_inbox`. GUI серит `@` всем, Send — только если `a2aInbox`.

## Child в Task (C46)

`agent.create` 1.5: optional `parentId`. Host **принимает** и пишет `agents.parent_id` (сегодня INSERT всегда NULL). Колонку 0001 не менять.

`parentId` — тот же `taskId`, тот же host, существующий агент. Это **не** `artifacts.parent_id`. Цикл parent (A→B→A, A→A) → `invalid_params`. Чужой task → `invalid_params`.

Child = новый `Agent`, свой `messages`. Не merge в родителя. Reference не ограничен деревом.

Delete parent-агента (если появится) **не** сносит children: `parent_id` → NULL («поднимаются»). Delete артефакта-родителя агентов не трогает (E5).

## Loops (C47)

**Сущность** в sqlite (0006), не только память.

```sql
CREATE TABLE loops (
  id              TEXT PRIMARY KEY,
  task_id         TEXT NOT NULL REFERENCES tasks(id),
  agent_a         TEXT NOT NULL REFERENCES agents(id),
  agent_b         TEXT NOT NULL REFERENCES agents(id),
  max_iterations  INTEGER NOT NULL,
  budget_turns    INTEGER NOT NULL,
  iteration       INTEGER NOT NULL DEFAULT 0,
  turns           INTEGER NOT NULL DEFAULT 0,
  status          TEXT NOT NULL CHECK (status IN ('running', 'stopped')),
  reason          TEXT,
  prompt          TEXT NOT NULL,
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL,
  CHECK (max_iterations BETWEEN 1 AND 32),
  CHECK (budget_turns BETWEEN 1 AND 64)
);
```

Нет `ON DELETE CASCADE`. 0001–0005 не трогать.

```
loop.start  { taskId, agentIds: [id, id], maxIterations, budgetTurns?, prompt }
loop.get    { loopId }
loop.stop   { loopId }
```

- `agentIds` ровно 2, оба в Task, не Shell.
- `maxIterations` **обязателен**, 1…32. Нет / 0 / infinity → `invalid_params`.
- `budgetTurns` optional, default `maxIterations * 2`, потолок 64. Каждый send/deliver в loop = 1 turn.
- Stop: `loop.stop` | maxIterations | budget | `denied` | error | `pty_dead`. После stop новых send нет.
- Host чередует A→B→A… `prompt` на первом ходе. Каждый ход — turn + лестница. Loop **не** включает yolo.
- Рестарт host: все `running` → `stopped` reason=`error` (не возобновлять).
- Тест: два агента в loop с max=2 **останавливаются**. Без лимита = P0.

## Protocol 1.5

Новые / bumped, `{major:1, minor:5}`:

```
agent.create          1.5   // + parentId?
a2a.transcript        1.5
a2a.deliver           1.5
loop.start            1.5
loop.get              1.5
loop.stop             1.5
```

Reference — не RPC: `agent.list` + GUI `@`.

Клиент без 1.5: 1.4 artifact / pty / write живы. `a2a.*` не в `accepted`.

Новые коды: `cross_host`, `no_inbox`, `loop_exhausted`.

WS как в проекте (поле **`event`**, не `type`):

```json
{ "event": "a2a.delivered", "fromAgentId": "…", "toAgentId": "…", "messageId": "…" }
{ "event": "loop.stopped", "loopId": "…", "reason": "max_iterations" | "budget" | "stop" | "denied" | "error" }
```

Без body транскрипта в WS.

### `a2a.transcript`

`{ agentId }` → `{ "agentId", "interface", "messages": [Message] }`

- chat: строки `messages` этого агента (после `clear_transcript` — пусто, это ок).
- terminal: Integration читает **vendor session** на этом host. Нет session / провайдер молчит → ошибка, не `[]` и не PTY scrollback.
- чужой hostId → `cross_host` (Terminal). Chat у нас один host — тот же путь.

Same user подразумевается (один host.db).

### `a2a.deliver`

```json
{ "fromAgentId": "…", "toAgentId": "…", "content": "review this" }
```

Проверки по порядку: тот же task, тот же hostId, to.caps.a2aInbox, content 1…1 MiB. Иначе `cross_host` / `no_inbox` / `invalid_params`.

Пишет `messages` получателя: `role=system`, content с префиксом `a2a:<fromAgentId>\n`. Не artifact. Не новая role (CHECK 0001 не трогаем).

Если получатель chat idle — сообщение лежит, следующий send его видит в context. Если running — не очередь второго turn (как `agent_busy`); deliver всё равно INSERT (inbox), не стартует второй child.

Лестница: deliver сам по себе не spawn. Следующий `agent.send` / loop turn — `kind=exec` как сейчас.

Ok: `{ "messageId": "…", "toAgentId": "…" }` + WS `a2a.delivered`.

### `loop.start` / `get` / `stop`

Ok start: `{ "loopId", "iteration": 0, "turns": 0, "maxIterations", "budgetTurns" }`.
`get` → тот же + `status: running|stopped` + `reason?`.
Нет loopId → `not_found`. Повтор `stop` идемпотентен.

## GUI минимум

- Agents: дерево по `parentId` (provenance). `@` mention любого агента Task.
- Create child: `parentId` = выбранный. Interface/harness из пикера. Inbox badge если `a2aInbox`.
- Deliver: кнопка/composer «send to @agent», disabled без inbox.
- Loop: два агента + поле maxIterations (обязано) + Start/Stop. Баннер running. Нет «infinite».
- Не полный Traycer chrome (roles, agent-selection settings — E8). Не sharing.

## Вне скоупа

- C75 sharing, E9 sync, C21 search
- Traycer full-access-for-A2A default
- Cross-host queue / chat-across-hosts (у нас один host)
- Inference, секреты в db
- Vendor-history как строки `messages`
- A2A в Shell / artifacts-in-PTY
- Fork chat, глубина nested tree, C63 stop-children
- Новая message role

## Приёмка Ф4 / E6

1. `@` / reference: любой агент Task, в т.ч. terminal без inbox.
2. `a2a.transcript` chat читает `messages`; terminal не берёт pty scrollback.
3. `a2a.deliver` → claude: message у child. → generic/codex: `no_inbox`. Cross-host (разный hostId) → `cross_host`, очередь пуста.
4. Child: `agent.create parentId` → list дерево; свой transcript ≠ родителя.
5. e2e кусок: artifact жив (E5) → child create → `a2a.deliver` → child `get_context` видит system a2a-строку.
6. `loop.start` без maxIterations → `invalid_params`. С max=2: после 2 итераций `loop.stopped` reason=`max_iterations`, новых send нет. Два агента без cap = P0, регрессия тестом.
7. Клиент без 1.5: artifact/pty/write живы.
8. Нет INSERT в `artifacts` из a2a. 0001–0005 байтово целы. 0006 = только `loops`.
9. Delete parent-агента не удаляет child (если такой RPC появится в тесте — parent_id NULL).

Код — следующие STAR (Core host, UI дерево/loop, Integration caps). E7 не открывать из этого файла.
