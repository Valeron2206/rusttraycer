# Ф7 — Hardening & Release v2.0.0

Для: Chief (процесс), Core/UI/Integration (harden + pack), Reviewer (финальный проход). Architect — матрица + v2-delta на теге.
От: Architect. Дата: 2026-08-19. Не код.
База: directive-v2 §4 Ф7 / §5 DoD; ADR-0001, 0006, 0008; e10-ops-v2; phase0-merge `fff9fb9`.
Протокол: **1.8**, без bump. Новых RPC нет. Storage 0001–0008 **байтово**. Миграции нет. Windows oos (ADR-0006). `.rpm` нет. Телеметрии нет (ADR-0008).

## Закон

1. Feature freeze. После APPROVE этой спеки новые способности не открывать. Только harden, docs, pack, tag.
2. Протокол остаётся **1.8**. Поле/метод = новый minor — **запрещено** в Ф7.
3. Не править ADR-0003…0008 без отдельного STAR.
4. Не трогать контракт E9 archive, E7/E8 prefs, A2A loops.
5. Origin: не force-push `main`. Пуш — только по приказу Chief.
6. DoD §5 «ни одной missing/partial» **сужается**: на теге `v2.0.0` каждая **must**-строка = `shipped` или `out-of-scope-by-ADR`. Строки **later** могут остаться `missing` до v2.x — это не блокер тега.

Новых ID Cxx нет.

## Freeze — входит в v2.0.0

Ф0–Ф6 / E1–E10 must, уже в дереве к `fff9fb9` и хвостам E10:

- Host + GUI + CLI, loopback `/rpc` `/health` `/ws` `/metrics`
- Allowlist `cli.generic` | `cli.claude` | `cli.codex`. N агентов. Ladder ask (не C26). Write + git без секретов в db.
- Terminal in-Task (C32–C36). Artifacts MD + comments (C38–C41; C42 = **MD**). A2A + loops (C43–C47).
- Switch + profiles + prefs (C48–C50). Roles + AGENTS.md + guides + 4 presets (C52, C54–C56).
- Export/import (C57). CLI status/logs/reset-db (C60). AppImage + .deb + tarball (C61). macOS aarch64 = cfg + CI compile (C62).
- Linux x86_64 — единственный supported package target.

## Freeze — later (v2.x, не тег)

| ID / тема | Почему не 2.0.0 |
|---|---|
| C37 terminals outside Task | e4-terminal-v2 |
| C42 PDF | MD must, PDF later |
| C51 multi-account | e7-model-ux-v2 |
| C53 mid-turn steer | e7-model-ux-v2 |
| C58 `rt-sync` | e9-sync-v2 |
| C64 Epic PR View | e3-write-v2 |
| C63 chrome (monitor/hooks/stash/drag) | не must эпиков |
| C65 worktree cleanup / branch prefix | C08/C16 must; cleanup later |
| Nested AGENTS.md, user presets, disable-detection | e8-workspace-v2 |
| Intel Mac, macOS signed dmg/notarize | e10-ops-v2 |
| Windows / WSL package (C73) | ADR-0006, v2.x |
| `.rpm` | ADR-0006 optional |
| `logs --follow` | e10-ops-v2 |

Oos без later: C26, C66–C75 (ADR).

## DoD на теге `v2.0.0`

Must зелёные:

- Brief №1–16: тест или ADR (колонка матрицы).
- Все must-строки матрицы `shipped` \| `oos-by-ADR`. Later — в таблице выше, не `shipped`.
- Секреты не в host.db / pid.json / наших config (ревью E3/E7/E9/E10).
- Нет телеметрии (ADR-0008): нет vendor SDK, нет скрытого outbound.
- Качество: `cargo fmt --check`, `clippy --workspace --all-targets -- -D warnings`, 0 `#[allow]` и 0 prod `unwrap`/`expect` на затронутых в Ф7 crates (исключение `#[cfg(test)]`). `cargo audit` на ubuntu.
- Покрытие новых модулей не резать ниже планки v1 там, где уже ≥70%.
- README-цикл на Linux x86_64 с чистого клона.
- `docs/v2-delta.md` + CHANGELOG + crate versions **2.0.0**.
- GitHub Release: tarball + AppImage + `.deb` + SHA256SUMS. Нет `.exe`, нет `.rpm`.

Мастер-e2e директивы §5 — **цель Ф7**, гоняется локально / CI ubuntu. Если кусок красный из-за later (нет PDF, нет rt-sync) — не блокер. Красный must (ladder, artifact MD, A2A, export, metrics) — блокер тега.

## e2e smoke (локально, без origin)

На Linux host после freeze-merge:

1. `rt-cli start` → `GET /health` 200.
2. `GET /metrics` → `rusttraycer_up` (C59), без токена, loopback.
3. `rt-cli status` / `logs --lines 10` / `reset-db` отказ без `--yes`; с `--yes` только после stop (C60).
4. GUI стартует против уже живого host (не спавнит).
5. `pack-linux.sh` dry-run **или** собранные AppImage + `.deb` (C61).
6. CI job `macos-latest` compile зелёный (C62). Не гейт ubuntu, но C62 не shipped пока красный.
7. Кусок директивы §5, который уже закрыт кодом: два harness, worktree, ladder ask→allow, commit, artifact, clear_transcript (artifact жив), A2A child, loop max-iterations, PTY resume session id, export→import второй host (оба hostId каноничны).

Origin не нужен. Не пушить, чтобы «прогнать CI».

## Релизный процесс

1. Ф7 STAR: harden + `v2-delta.md` + CHANGELOG + bump 2.0.0. Conventional Commits. Автор без git config как в проекте.
2. Reviewer APPROVE. Chief мержит в `phase0-merge` / main. **Не** force-push origin/main.
3. Chief (или STAR) ставит annotated tag **`v2.0.0`**. `release.yml` (`on: push tags v*`) собирает linux-x86_64 tarball + AppImage + `.deb`.
4. GitHub Release: те же файлы + SHA256SUMS. Smoke скачанного AppImage/`.deb` на linux x86_64.
5. Отчёт `[RELEASE v2.0.0]`: тег, матрица с доказательствами, later/oos списки, заметки v2.x.

Пуш origin — только приказ Chief. Локальный тег без origin допустим как rehearsel.

## Харден

- clippy `-D warnings`, 0 `allow` на crates, которые трогает Ф7.
- 0 `unwrap`/`expect` вне тестов в новом/изменённом коде.
- fmt. audit.
- Grep: нет token/pat/DSN в дереве и в host.db schema.
- Не рефакторить E7/E8/E9 «заодно».

## GUI / CLI минимум

Нового chrome нет. reset-db не в GUI. Metrics не в GUI. Pack — scripts/CI.

## Вне скоупа

- Любой новый RPC / 1.9 / 0009
- C58, Windows package, rpm, PDF, C37/C51/C53/C64
- Правка текста ADR-0003…0008
- Force-push, managed cloud, телеметрия

## Приёмка Ф7 (чеклист, без новых C)

1. Спека в дереве. Кода в этом STAR нет.
2. После code STAR: must-строки матрицы не `missing` (кроме later-таблицы).
3. Smoke 1–7 выше зелёные локально.
4. Tag `v2.0.0` → артефакты C61, нет Windows.
5. Handshake 1.8, схема 0008. Клиент без 1.8 не цель релиза (v2 = 1.8).
6. Reviewer + Architect расписались по матрице. Chief публикует `[RELEASE v2.0.0]` когда скажет пушить.

Код harden/delta/bump — следующие STAR. Later не открывать из этого файла.
