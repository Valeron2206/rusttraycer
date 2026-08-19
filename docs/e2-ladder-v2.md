# E2 — Permission ladder + Yolo (v2), фундамент Ф1

Для: Core (host/storage/protocol), UI (контролы + approval card).
От: Architect. Дата: 2026-08-19. Не код.
База: brief №15; matrix C23–C26; directive E2.
Протокол: minor bump **1.1** на новых методах. Конверт camelCase не менять. Миграция storage **0003** (0001–0002 не трогать).

## Решение по C26 (закон)

Traycer Desktop 1.1.x сделал **full access default**. **Мы это не копируем.**

- Default новой policy: **`ask`**.
- Full-access как скрытый дефолт — запрещён.
- C26 в матрице после этой спеки: **out-of-scope** (не shipped). Не «missing, потом скопируем».

Yolo — не default и не «full access без entера». Это явный bypass с индикацией.

## Лестница (C23)

Каждый turn, который **exec** (сейчас: spawn harness в `agent.send`) или **edit** (E3: `files.write` / patch / `git.commit`), проходит:

| Mode | Поведение |
|---|---|
| `ask` | Host не исполняет. WS `agent.approval` + RPC ждёт `approval.respond`. |
| `allow-once` | Этот turn да; policy остаётся `ask`. |
| `allow-always` | Пока scope совпадает — без карточки. Scope: `agent` или `workspace`. |
| `deny` | Turn не стартует. `error.code = denied`. |

`allow-once` — решение на карточке, не stored mode. Stored modes: `ask` | `allow-always` | `deny`.

## Persist (C24)

Таблица `policies` (миграция 0003):

- `id`, `workspace_id` NULL, `agent_id` NULL, `mode` (`ask`/`allow-always`/`deny`), `scope` (`agent`/`workspace`), `yolo` INTEGER 0/1, `updated_at`
- CHECK: ровно одно из `workspace_id` / `agent_id` NOT NULL
- UNIQUE(agent_id) WHERE agent_id IS NOT NULL; UNIQUE(workspace_id) WHERE workspace_id IS NOT NULL

Резолв (жёсткий): **agent row > workspace row > default `ask`**.

Переживает рестарт host (DoD). Секреты в таблице запрещены (ADR-0005).

## Yolo (C25)

- Флаг `yolo=true` только через явный `policy.set` (GUI: отдельный confirm, не тот же dropdown).
- Пока yolo: лестница не зовётся; chrome GUI — постоянный баннер «Yolo» на Task/агенте; doctor/agent.get отдают `yolo`.
- Снять yolo = `policy.set { yolo: false }` → снова резолв mode.
- Не копировать extension Smart YOLO / Phase automation.

## Protocol 1.1 (новые методы)

Существующие методы остаются `{major:1, minor:0}`. Новые:

```
policy.get      1.1
policy.set      1.1
approval.respond 1.1
```

Handshake: клиент Ф1 объявляет эти три. Старый GUI без них работает как v1 (лестницы нет — **host Ф1 всё равно default ask** и отклоняет `agent.send` без 1.1, если не yolo/allow-always: лучше compat).

Compat (закон): если клиент **не** accepted `policy.*` 1.1, host Ф1 **не** блокирует v1 `agent.send` (иначе сломаем старый GUI). Лестница активна только когда handshake принял 1.1. GUI Ф1 обязан объявить 1.1.

### `policy.get`

Params: `{ "agentId"?: string, "workspaceId"?: string }` — ровно одно.
Ok:

```json
{
  "mode": "ask",
  "scope": "agent",
  "yolo": false,
  "source": "default" | "agent" | "workspace"
}
```

### `policy.set`

Params:

```json
{
  "agentId": "…",          // xor workspaceId
  "mode": "ask" | "allow-always" | "deny",
  "scope": "agent" | "workspace",
  "yolo": false
}
```

`scope=workspace` требует `workspaceId`. `scope=agent` требует `agentId`.
Ok: тот же объект, что `policy.get` после записи.

### `approval.respond`

Params: `{ "approvalId": "…", "decision": "allow-once" | "allow-always" | "deny" }`
`allow-always` на карточке пишет agent-policy `allow-always` + повторяет turn.
Идемпотентно, если turn уже кончился: `ok { applied: false }`.

### WS

`agent.approval` (когда mode=ask и turn хочет exec/edit):

```json
{
  "type": "agent.approval",
  "approvalId": "…",
  "agentId": "…",
  "taskId": "…",
  "kind": "exec" | "edit",
  "summary": "spawn cli.claude" 
}
```

Пока висит approval, `agent.send` → `agent_busy` (очередь не открываем, как cancel).

Новые коды: `denied`, `approval_expired`.

## GUI Ф1

- На агенте: dropdown mode (`ask` / `allow-always` / `deny`) + отдельный Yolo (confirm).
- Карточка approval: summary, Allow once, Always (this agent), Deny.
- Default отображается как Ask, не Full.
- Нет Traycer-копии «new conversations default to full access».

## Вне скоупа Ф1

- Реальный `files.write` / git write — E3, но **тот же** approval `kind=edit`.
- PTY exec — E4, тот же `kind=exec`.
- A2A «full access required» Traycer 1.1.10 — не переносим как default.

## Приёмка Ф1

1. Новый агент: `policy.get` → `mode=ask`, `source=default`.
2. GUI 1.1: `agent.send` при ask → WS approval; deny → нет child; allow-once → один turn, mode остаётся ask.
3. allow-always persist после рестарта host.
4. Yolo: баннер; send без карточки; после снятия — снова ask.
5. Клиент без 1.1: v1 send проходит (compat).
6. C26 не реализован как full-access default.
