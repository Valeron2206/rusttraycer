# E10 — Ops & platforms (v2), Ф6

Для: Core (host `/metrics`, rt-cli), UI только если target-cfg eframe/rfd. Integration не трогать.
От: Architect. Дата: 2026-08-19. Не код.
База: directive E10; ADR-0001; ADR-0006; ADR-0008; matrix C59–C62, C73.
Протокол: **без** minor bump. 1.0–1.8 живы. Нового RPC нет. Storage: миграции нет. 0001–0008 **байтово**.

## Закон ADR-0006 / ADR-0001 / ADR-0008

1. **v2.0.0 required:** Linux x86_64 (ADR-0001). Пакеты **AppImage + .deb**. CI gate = `ubuntu-latest`.
2. **v2.0.0 target:** macOS **aarch64**. В E10 — target-cfg eframe/rfd + job `macos-latest`. Intel Mac best-effort, не DoD. Notarize / signed .dmg — later.
3. **Windows и WSL — не v2.0** (C73 / ADR-0006). v2.x после macOS. `windows-latest` в CI не добавлять.
4. `.rpm` optional, не DoD. WSL не цель.
5. `GET /metrics` — только loopback, без сессии, **не** в vendor (ADR-0008). Нет Sentry/PostHog/OTel exporter.
6. Секреты не в host.db (ADR-0005). Managed cloud не предлагать.
7. E9 (`sync.*`, C58 rt-sync) **не открывать**. Лестницу / A2A / artifacts / E7 / E8 не менять.

## Решение C59–C62 (закон)

| ID | Ф6 |
|---|---|
| C59 `GET /metrics` loopback | **must**, HTTP как `/health`, не RPC |
| C60 `rt-cli status` / `logs` / `reset-db` | **must** |
| C61 Linux AppImage + .deb | **must** (tarball linux-x86_64 остаётся) |
| C62 macOS aarch64 | **must** = target-cfg + CI compile; пакет macOS later |
| Windows / WSL / .rpm / notarize | **oos** v2.0 / later |

Матрица: C59–C62 Ф6. C73 без изменений (oos).

## Что есть

- HTTP: `POST /rpc`, `GET /health` (sessionless), `GET /ws`. **Нет** `/metrics`.
- Bind только `127.0.0.1`. pid.json + `host.log` в `$RUSTTRAYCER_HOME/host/` (или `~/.rusttraycer/host`).
- `rt-cli`: `start` / `stop` / `doctor`. Нет status/logs/reset-db.
- CI / release: `ubuntu-latest` only. Артефакт: `rusttraycer-v*-linux-x86_64.tar.gz` + SHA256SUMS.
- eframe features `x11`; rt-gui rfd = `xdg-portal` + `async-std`. macOS cfg нет.
- Handshake 1.8 (E9). Нового method не нужно.

## C59 — `GET /metrics`

Тот же listener, что `/health`. Без токена, без handshake. Не loopback → уже отказ bind (закон host).

- Метод: `GET /metrics`. `POST` / другие → 404/405 как у `/health`.
- Тело: Prometheus text 0.0.4 (`text/plain; version=0.0.4`). Не JSON-RPC.
- Имена (минимум):

```
rusttraycer_up 1
rusttraycer_agents{status="idle|running|error"} <n>
rusttraycer_tasks{status="open|archived"} <n>
```

Без путей, без transcript, без hostId в labels, без секретов. Не скрейпить наружу. Не новый crate telemetry.

Doctor / handshake **не** дублировать этот набор. Клиент без знания `/metrics` жив.

## C60 — CLI

Не ломать `start` / `stop` / `doctor`.

### `rt-cli status`

Внешний взгляд на pid.json: alive?, pid, rpcUrl, dataDir. Не ходит в `/rpc` (в отличие от глубокого doctor). Host мёртв → exit 0, JSON `{"alive":false}`.

### `rt-cli logs`

Печатает хвост `host.log` в stdout. `--lines` default 200, min 1 max 10000. Файла нет → пустой stdout, exit 0. **Не** `--follow` в E10.

### `rt-cli reset-db`

Деструктивно. Обязателен `--yes`. Host alive (pid жив) → ошибка, сначала `stop`. Удаляет только `host.db`, `host.db-wal`, `host.db-shm`. Не трогает `host.log`, `agent-selection-guide.md`, keyring, workspace FS. Следующий `start` поднимает пустую схему (0001–0008 как обычно).

Нет GUI для reset-db.

## C61 — Linux packages

`release.yml` на tag `v*`:

- как сейчас: linux-x86_64 tarball + SHA256SUMS
- **плюс** AppImage и `.deb` (rt-gui + rt-host + rt-cli). Один amd64.
- `.rpm` не делать.

CI `check` на ubuntu остаётся гейтом (fmt/clippy/test/release build). Сборка пакетов — release job, не каждый PR.

## C62 — macOS aarch64

- Cargo target-cfg: Linux оставляет `x11` / `xdg-portal`; macOS — eframe без `x11`, rfd без portal (системный диалог).
- CI: job `macos-latest` = `cargo build --workspace` (и тесты, если зелёные без extra deps). Не гейт для linux job: красный macos не блокирует ubuntu, но C62 не shipped пока macos job зелёный.
- Intel Mac: не в матрице.
- Windows job: не добавлять.
- Release macOS artifact: later (не DoD 2.0.0). README: supported = Linux x86_64 packages; macOS aarch64 = build-from-source.

## Protocol / Storage

Новых RPC нет. 1.8 не ломать. Миграции нет. reset-db = unlink файлов, не SQL DROP в спеке.

## GUI минимум

Нет экрана metrics. Нет кнопки wipe db. macOS dialog — следствие cfg, не новый chrome. E9 export/import не трогать.

## Вне скоупа

- C58 `rt-sync`
- Windows / WSL / .rpm / signed dmg
- Sentry / PostHog / OTel / crashpad (C68)
- Новый handshake method, секреты в db
- `logs --follow`, GUI reset-db, scrape metrics off-box

## Приёмка Ф6 / E10

1. `curl -sS http://127.0.0.1:<port>/metrics` без токена → 200, есть `rusttraycer_up`. С хоста не loopback bind нет.
2. `rt-cli status` / `logs --lines 10` работают при живом и мёртвом host.
3. `reset-db` без `--yes` → отказ. С `--yes` при running → отказ. После stop + reset + start: пустой doctor, схема жива. `host.log` на месте.
4. Tag release (или dry-run job): tarball + AppImage + .deb, sha256. Нет `.exe`.
5. `macos-latest` build проходит с cfg. `windows-latest` в yml нет.
6. Клиент 1.8: rpc/ws/health/sync живы. 0001–0008 байтово целы.
7. Нет vendor SDK, нет DSN.

Код — следующие STAR. E9 C58 и Windows не открывать из этого файла.
