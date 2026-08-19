# v2.1 — Complete DoD (missing → shipped)

Для: Core / UI / Integration по волнам ниже. Architect — эта спека.
От: Architect. Дата: 2026-08-19. Не код.
База: origin/main `ded044c` = tag **v2.0.0**. Тег не двигать. Origin не пушить.
Закон: [f7-release-v2](f7-release-v2.md) freeze **снят только** на строки ниже. Новых способностей вне списка нет. Новых Cxx нет.
Протокол: minor **1.9** (1.0–1.8 живы). Storage: **0009** (0001–0008 байтово).

## Закон

1. DoD директивы §5: на теге **v2.1.0** ни одной `missing`/`partial`. Остаются только `shipped` | `out-of-scope-by-ADR`.
2. Oos **без** правки ADR: C26, C66–C75 (телеметрия, managed cloud, CRDT, секреты в db, Windows/WSL, extra harnesses as required, inference, sharing). Не открывать.
3. C58 = self-hosted `rt-sync`. **ADR-0003 не правим:** там уже «min = export/import, goal = rt-sync». v2.1 делает goal **must**. Не cloud, не эскалация PO.
4. Секреты не в host.db / архиве / hooks table (ADR-0005 / C74). C51 — только env/keyring.
5. C37: workspace **обязателен**. Task — нет. «Start without folder» Traycer = не без cwd: нужен workspace. Без folder — не в v2.1.
6. Handshake: клиент 1.8 жив (chat/write/pty/artifact/a2a/switch/guides/sync.export). Новые методы не в `accepted`.
7. Windows / `.rpm` / notarize / Intel-only Mac — не этот релиз.

## Что must (сейчас missing/partial)

| ID | v2.1 | Контракт |
|---|---|---|
| C21 | search Task/workspace/artifact | `search.query` |
| C37 | Terminal/Shell без Task | `shell.create` / `agent.create` без `taskId`, с `workspaceId` |
| C51 | multi-account per provider | labels в sqlite; креды env/keyring |
| C53 | mid-turn steer | `agent.steer`; cap per harness |
| C58 | self-hosted rt-sync | тот же archive v1, HTTP между user hosts |
| C63 | monitor + hooks + stash + drag | metrics GUI; hooks file; `stash.*`; GUI drag |
| C64 | PR view | `pr.get` через system `gh`/`git`, без PAT в db |
| C65 | worktree cleanup + prefix | `worktree.gc` + setting |
| C42 | PDF | `artifact.export format=pdf` → 200 |
| — | nested `AGENTS.md` | walk; без нового RPC |
| — | user presets | `preset.create/update/delete` |
| — | `rt-cli logs --follow` | CLI only |
| — | мастер-e2e §5 | **обязательный** CI job `ubuntu-latest` |

## Что есть (v2.0.0)

- Search RPC нет. Terminal/shell требуют `taskId` (e4).
- Один implied account на provider. Steer нет.
- `sync.export`/`import` есть. `rt-sync` нет.
- `/metrics` есть, GUI monitor нет. Hooks/stash/drag нет.
- PR view нет. `worktree.ensure` есть, gc нет.
- `format=pdf` → `invalid_params`. AGENTS.md только root. 4 built-in presets.
- `logs --lines`, нет `--follow`. Мастер-e2e не в CI.

## Protocol 1.9

```
search.query          1.9
agent.steer           1.9
account.list          1.9
stash.list|add|delete 1.9
preset.create|update|delete  1.9   (list уже 1.7)
worktree.gc           1.9
pr.get                1.9
sync.push | sync.pull 1.9
```

Адitive:

- `shell.create` / `agent.create`: `taskId` optional; если нет — `workspaceId` required. Есть task — как 1.3/1.0.
- `agent.create` / `agent.switch`: optional `accountId`.
- `artifact.export`: `format=pdf` больше не `invalid_params`.
- `rt-cli logs --follow` (не RPC).

Коды: `not_supported` если harness без steer; `auth_required` если `gh` не залогинен (не собираем PAT); `conflict` как у import.

### Кратко по методам

`search.query { q, kinds?: ["task","workspace","artifact"] }` → `{ items: [{ kind, id, title, hint }] }`. Scan sqlite (title/body/path), без FTS-миграции.

`agent.steer { agentId, content }` только `status=running`. Иначе `invalid_params`. Cap: `cli.claude` + `cli.codex` must; `cli.generic` → `not_supported` (не блокер C53).

`account.list` → labels без секретов. Create account = запись label; секрет пользователь кладёт в env `RUSTTRAYCER_<PROVIDER>_<LABEL>` или keyring. Host не пишет token.

`stash.*` — prompt (+ optional image bytes path, не cloud). Durable в 0009.

`worktree.gc { dryRun }`: stale / merged-via-`gh` / landed. Prefix из settings (default `rt/`). Не удалять worktree агента `running`.

`pr.get { workspaceId, number? | url? }` → checks, commits, files, local diff. Бинарь `gh` + `git`. Нет token field.

`sync.push { peerUrl }` / `sync.pull { peerUrl, workspaceId }`: archive v1 как E9. Peer = user-owned host. Loopback или явный URL. Auth: optional shared secret **только env** `RUSTTRAYCER_SYNC_SECRET`, не sqlite. Managed cloud = стоп.

Nested AGENTS.md: от attached path / cwd вверх до workspace root, все найденные, nearest первым. Root по-прежнему must. Disable toggle — не в v2.1.

User preset: те же поля, что built-in + `name`. Не board.

## Storage 0009

Не править 0001–0008.

```sql
CREATE TABLE provider_accounts (
  id TEXT PRIMARY KEY,
  provider TEXT NOT NULL,
  label TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE (provider, label)
);

CREATE TABLE user_presets (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  default_role TEXT NOT NULL,
  title_hint TEXT,
  prompt TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE prompt_stash (
  id TEXT PRIMARY KEY,
  body TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE worktree_settings (
  workspace_id TEXT PRIMARY KEY REFERENCES workspaces(id),
  branch_prefix TEXT NOT NULL DEFAULT 'rt/'
);
```

Нет token/pat/hook-secret колонок. Hooks: файл `$RUSTTRAYCER_HOME/hooks.json` (command или URL), не sqlite.

`agents.account_id` nullable TEXT, без FK на секреты.

## Волны (порядок)

Параллель внутри волны по зонам. Следующая волна не раньше APPROVE предыдущей, если есть зависимость.

| Волна | Что | Кто | Зависимость |
|---|---|---|---|
| V1 | C42 PDF; `logs --follow`; nested AGENTS.md | Core + Integration (CLI) | нет |
| V2 | C21 search; C65 gc+prefix | Core + UI | нет |
| V3 | C37 terminal/shell без Task | Core + UI | нет |
| V4 | C51 accounts; C53 steer | Core + UI + Integration (caps) | нет |
| V5 | C64 PR view | Core + UI | `gh` на машине CI/dev |
| V6 | C63 monitor/hooks/stash/drag | UI + Core (stash) | C59 уже есть |
| V7 | C58 rt-sync + user presets | Core + Integration + UI | E9 archive |
| V8 | мастер-e2e §5 в CI ubuntu | Integration | V1–V7 must в дереве |

Новых epic ID нет. Тег **v2.1.0** после V8 + Reviewer + матрица 0 missing/partial.

## ADR-0003 (C58)

Не меняем файл ADR. Уточнение здесь: `rt-sync` = процесс или режим host, который гоняет уже специфицированный `rusttraycer.export` v1. Не Yjs. Не Traycer Sync $10. Предложить managed endpoint = эскалация (закон ADR уже).

## Мастер-e2e (CI must)

Job `e2e-master` на `ubuntu-latest`, host-API, GUI smoke отдельно. Цепочка директивы §5 целиком. Красный job блокирует merge в main / тег v2.1.0. Куски later из v2.0 (PDF, rt-sync) **входят** в цепочку в v2.1 (export/import уже был; PDF — отдельный assert; rt-sync — push/pull на 127.0.0.1 второй host).

## GUI минимум

- Search box (C21). PR panel (C64). Worktree cleanup confirm (C65).
- New terminal на workspace без Task (C37). Account picker (C51). ⌘Enter steer (C53).
- Metrics chip (C63). Hooks в Settings (путь к hooks.json). Stash palette. Drag agent → tile.
- Sync push/pull: URL peer, без cloud login.
- PDF download рядом с MD.

## Вне скоупа

- C26, C66–C75
- Windows package, rpm, notarize
- Disable AGENTS.md toggle
- `cli.generic` steer
- Новый minor после 1.9 в этом релизе

## Приёмка v2.1.0

1. Матрица: 0 `missing`/`partial`. Later-таблица Ф7 пуста (всё либо shipped, либо oos).
2. Клиент 1.8: create/send/export живы.
3. 0001–0008 байтово. 0009 без секретов.
4. `format=pdf` 200. `logs --follow` пишет пока SIGINT.
5. CI `e2e-master` зелёный на ubuntu.
6. Tag v2.0.0 не переписан. Origin — по Chief.

Код — STAR после APPROVE этой спеки. Не стартовать волны без STAR.
