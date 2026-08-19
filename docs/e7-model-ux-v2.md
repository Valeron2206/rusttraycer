# E7 — Model UX (v2), Ф5

Для: Core (host/protocol/storage), UI (switch + profiles). Integration — только caps уже существующих трёх harness.
От: Architect. Дата: 2026-08-19. Не код.
База: ADR-0007; brief №8, №11; matrix C48–C53; directive E7.
Протокол: minor bump **1.6**. 1.0–1.5 не ломать. Конверт camelCase. Storage: миграция **0007**. 0001–0006 **байтово**.

## Закон ADR-0007

1. Один `agentId` = один durable transcript. Switch harness/model: host меняет `AgentBackend`, `messages` остаются. **Не клон, не новый агент.**
2. Model profiles = локальные именованные пресеты `harness + {model, effort, fast}`. Не облако.
3. Слот `native` можно показать как provider id. **Inference в v2 нет** (C67 oos).
4. C66 (Grok/Amp/Hermes/…) **oos**. Allowlist: `cli.generic` | `cli.claude` | `cli.codex`.
5. Новые RPC — handshake minor (№11). Секреты в host.db запрещены (ADR-0005 / C74).
6. Лестницу / A2A / artifacts **не открывать**. Switch не yolo и не `kind=edit|exec`.
7. E8 (C54–C56) — отдельная спека после E7.

## Решение C48–C53 (закон)

| ID | Ф5 |
|---|---|
| C48 switch same agentId | **must** |
| C49 named profiles | **must** |
| C50 remember last model/effort/fast per harness | **must**, sqlite без секретов |
| C51 multi-account per provider | **later** (креды env/keyring; не sqlite; не блокер switch) |
| C52 agent roles | **later** |
| C53 mid-turn steer ⌘Enter | **later** (не все harness; не каркас UX) |

Матрица: C51–C53 wave **later**.

## Что есть

- `agent.create { taskId, provider, parentId?, interface? }` фиксирует `provider` навсегда.
- `Agent.provider` на проводе есть; model/effort/fast **нет**.
- Doctor отдаёт три provider + caps. Пикер только на create (E1).
- Backend в supervisor привязан к агенту с create. Switch нет.
- Profiles / prefs таблиц нет.

## Storage 0007

Не править файлы 0001–0006. Только новая миграция:

```sql
ALTER TABLE agents ADD COLUMN model TEXT;
ALTER TABLE agents ADD COLUMN effort TEXT;
ALTER TABLE agents ADD COLUMN fast INTEGER NOT NULL DEFAULT 0;

CREATE TABLE model_profiles (
  id         TEXT PRIMARY KEY,
  name       TEXT NOT NULL UNIQUE,
  provider   TEXT NOT NULL,
  model      TEXT,
  effort     TEXT,
  fast       INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE harness_prefs (
  provider   TEXT PRIMARY KEY,
  model      TEXT,
  effort     TEXT,
  fast       INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL
);
```

Нет token/pat/account/key колонок. `fast` 0/1. `provider` ∈ allowlist (check в host, не обязательно SQL).

C50: после успешного switch/create host UPSERT `harness_prefs` для этого provider. Следующий create/switch без явных params берёт prefs, иначе null (harness default).

## Protocol 1.6

```
agent.switch      1.6
profile.create    1.6
profile.list      1.6
profile.get       1.6
profile.update    1.6
profile.delete    1.6
prefs.get         1.6
```

`agent.create` / `agent.get`: optional `model`, `effort`, `fast` на `Agent` (новые поля = host 1.6; старый клиент игнорирует). Create без params → подставить `harness_prefs` если есть.

Клиент без 1.6: 1.5 a2a/loop / 1.4 artifact / pty / write живы. `agent.switch` не в `accepted`.

Новые коды: нет обязательных. `not_pty` уже есть (switch terminal → generic). `agent_busy` если status=running: **сначала cancel, потом switch** — не молчаливый kill. GUI: Stop или отказ. Host при running → `agent_busy` (не менять backend).

WS не обязателен. После switch GUI делает `agent.get` + `get_context` (тот же id, те же messages).

### `agent.switch`

```json
{
  "agentId": "…",
  "provider": "cli.codex",
  "model": "o3",
  "effort": "high",
  "fast": false,
  "profileId": null
}
```

Правила:

- `profileId` задан → взять provider+params из профиля; явные поля поверх.
- Иначе `provider` optional: нет → тот же harness, меняются только params.
- Provider вне allowlist / `native` → `invalid_params`.
- interface=terminal и новый provider без `caps.pty` → `not_pty`.
- `agentId` тот же. `messages` не копировать и не чистить. `parentId` / `providerSessionId`: session id **сбросить** (другой vendor); parent не трогать.
- Worktree остаётся. Policy/yolo не менять.
- Ok: `Agent` с новым provider/params.

Не клонировать row. Не `agent.create`.

### Profiles

`profile.create { name, provider, model?, effort?, fast? }` → `Profile`. `name` 1…80, unique.

`list` → `{ items: [Profile] }`. `get` / `update` / `delete` по `profileId`. Delete не трогает агентов (у них копия params).

`prefs.get` → `{ items: [ { provider, model, effort, fast } ] }` на три harness (пустые null). Отдельного `prefs.set` нет: пишет switch/create.

### `native`

Doctor может отдать `{ id: "native", available: false, detail: "reserved", caps: {… pty false, needsApiKey false } }`. Create/switch на `native` → `invalid_params`. Нет бинаря, нет llama.cpp.

## GUI Ф5

- На выбранном агенте: harness picker (doctor) + model/effort/fast. Apply = `agent.switch`. Тот же чат, тот же id.
- Profiles: список, save current as profile, apply profile.
- Create agent: те же поля; дефолты из `prefs.get`.
- Нет account switcher (C51), нет roles (C52), нет ⌘Enter steer (C53).
- Нет поля API key.

## Вне скоупа

- C51 / C52 / C53
- C54–C56 E8
- C66 named extra harnesses, C67 inference
- E9 sync профилей
- Секреты в sqlite, Settings PAT

## Приёмка Ф5 / E7

1. Два turn на `cli.generic` → `agent.switch` на `cli.claude` → тот же `agentId`, `get_context` те же messages, provider новый.
2. Switch running → `agent_busy`, messages целы.
3. Profile create → apply на другой агент → params совпали, id агента тот же.
4. Create без params после switch: prefs подставили last model/effort/fast этого harness. В sqlite нет колонок secret.
5. Terminal agent → switch на generic → `not_pty`.
6. `native` create → `invalid_params`.
7. Клиент без 1.6: send / artifact / a2a живы.
8. 0001–0006 байтово целы.

Код — следующие STAR. E8 не открывать из этого файла.
