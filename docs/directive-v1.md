# ДИРЕКТИВА: RustTraycer → Production Release v1.0.0

**Кому:** Chief (Project Manager / Chief of Staff)
**Видимость:** Architect, Core, UI, Reviewer, Integration
**От:** Валерий (Product Owner)
**Режим:** Полная автономия. Довести проект до production-ready релиза без участия Product Owner.

Сохранено Chief 2026-08-19 как единственный источник правды о целях релиза `v1.0.0`.

---

## 0. Передача полномочий

1. Режим «не пушу / жду заказ» снят. Право на push в `origin/main` есть только у Chief и только через гейт из §4. Никто не пушит напрямую.
2. Chief принимает все решения по проекту: scope внутри DoD, приоритеты, merge, релиз, откаты. Product Owner не спрашивают и не ждут.
3. Если спека молчит по вопросу — решение принимает Architect через короткий ADR в `docs/adr/NNNN-<slug>.md` (контекст → решение → последствия, ≤ 1 страницы). ADR принят — работа продолжается. Не стопориться на открытых вопросах.
4. Эскалация к Product Owner разрешена только по 4 триггерам:
   - нужны секреты, токены, платные внешние сервисы;
   - переписывание истории `origin/main` (force-push, rebase опубликованного);
   - лицензионные/юридические вопросы (заимствование кода, смена лицензии);
   - обнаружен факт, делающий DoD недостижимым в текущей архитектуре.
   Всё остальное — зона Chief.
5. Этот файл — единственный источник правды о целях релиза.

---

## 1. Миссия

Довести RustTraycer до тега `v1.0.0` + GitHub Release с бинарями, полностью соответствующего Definition of Done (§2). Проект = pure-Rust desktop-аналог Traycer: `rt-host` (демон) + `rt-gui` (egui-клиент) + `rt-cli`, петля «folder → Task → Agent → chat → transcript переживает рестарт», спеки в `docs/`.

**Scope-границы (жёсткие):**
- **Входит:** всё из `docs/*-v0.md` + принятая волна 0002 (worktrees + CHECKLIST, git-files RPC, Git-панель в GUI, харнесс `cli.codex`).
- **НЕ входит** (post-v1, не трогать даже частично): PTY, terminal mux, A2A, cloud sync, live collab/CRDT, permissions ladder, `AGENTS.md`-selection. Заглушки `pty.rs` / `mux.rs` остаются `// reserved, no impl, no deps`.
- Wire-формат протокола (`docs/protocol-v0.md`) — источник правды. Изменения провода только через версионированную правку спеки + ADR, camelCase-конверт и коды ошибок не переопределять.

---

## 2. Definition of Done v1.0.0

Релиз не объявляется, пока каждый пункт не подтверждён ссылкой на CI-ран, коммит или тест.

**Сборка и качество**
- `rust-toolchain.toml` закреплён (стабильный канал; апгрейд edition/MSRV — только через ADR).
- `cargo build --workspace --release` зелёный на CI.
- `cargo fmt --check` чистый.
- `cargo clippy --workspace --all-targets -- -D warnings` чистый, без единого `#[allow]` (исключения — только ADR).
- 0 `unwrap()` / `expect()` вне `#[cfg(test)]`. Ошибки — через `?`, `thiserror`, явный `tracing::error!`. Паника допустима только в `main()` при фатальном старте.
- Нет `_ =` подавления `Result`, нет проглоченных ошибок в spawn'ах tokio.
- Нет `TODO` / `FIXME` / `unimplemented!` в поставляемом коде.
- Структурированный `tracing` на всех границах: RPC-вход/выход, supervisor turns, storage-ошибки, lifecycle host.

**Тесты**
- `cargo test --workspace` зелёный на CI.
- Покрытие (cargo-llvm-cov) ≥ 70 % строк суммарно по `rt-host`, `rt-storage`, `rt-runtime`, `rt-protocol`. GUI — smoke-уровень, в метрику не входит.
- Интеграционные E2E (автотесты):
  1. README-цикл: start host → workspace.add → task.create → agent.create → agent.send через `cli.generic` (фейковый агент-скрипт) → transcript в SQLite;
  2. тот же цикл через `cli.codex` (мок бинаря);
  3. рестарт host: Task/Agent/Message на месте, `hostId` тот же, Running → Error;
  4. второй `agent.send` при Running → `agent_busy`;
  5. pid-lock: второй инстанс host не встаёт;
  6. handshake: реджект несовместимой версии;
  7. worktree-изоляция по спеке 0002;
  8. `files.tree`/`files.read`: лимит 1 MiB → `file_too_large`, бинарь → `file_binary`.
- `cargo audit` (или `cargo deny check advisories`) — 0 critical/high.

**CI/CD**
- `ci.yml`: fmt → clippy → test → build, триггер на PR и push в main. Матрица: `ubuntu-latest` обязательно; `macos-latest` — по ADR-001.
- `release.yml`: по тегу `v*` — сборка release-бинарей, `tar.gz` + sha256, GitHub Release.
- Ветка `main` защищена процессом §4.

**Продукт и документация**
- `rt-cli doctor` — диагностика (host жив/мёртв, порт, версия, путь к БД, харнессы).
- Graceful shutdown: SIGTERM/`rt-cli stop` → WAL, снятие pid.json.
- Решение по `agent.cancel` принято Architect (ADR) и реализовано или мотивированно отложено.
- `README.md` соответствует реальности.
- `CHANGELOG.md` (Keep a Changelog), версии крейтов `1.0.0`.
- Спеки в `docs/` синхронизированы с кодом.
- Тег `v1.0.0` + Release + smoke скачанного бинаря на CI.

---

## 3. Роли

| Агент | Владение (запись) |
|---|---|
| Architect | `docs/`, `docs/adr/` |
| Core | `rt-host`, `rt-storage`, `rt-protocol`, `rt-runtime` (кроме адаптеров) |
| Integration | `rt-runtime` (адаптеры), `rt-cli` |
| UI | `rt-gui` |
| Reviewer | ничего (read-only) |
| Chief | корень репо, `.github/`, `CHANGELOG.md` |

Одна задача = одна ветка `task/NNNN-<slug>` от актуального `main`. Два писателя в один файл параллельно — запрещено.

---

## 4. Процесс

1. Одна задача = один worktree = одна ветка `task/NNNN-<slug>`.
2. Conventional Commits, американский английский.
3. Тиммейт → self-check → Reviewer → `APPROVE` → Chief мержит в `main` → CI зелёный. CI упал — стоп-линия.
4. `REJECT` → максимум 3 итерации, потом перепланирование.
5. Фаза 0 легализует волну 0002 на `origin/main`.
6. Версии из `Cargo.toml` — закон. Крупные крейты без ADR нельзя. llvm-cov/audit — можно без ADR.
7. Сбой плана — СТОП и перепланирование. Паттерн в `tasks/lessons.md`.
8. Деструктивные операции — цель в брифе/PR до выполнения.

---

## 5. Фазы

- **0 Sync & Baseline** — директива, ADR-001, toolchain+CI ubuntu, легализация 0002, CI зелёный на `origin/main`.
- **1 Hardening** — unwrap→0, thiserror, tracing, shutdown, лимиты, clippy-стена в конце.
- **2 Feature completion** — дельта код↔спеки, cancel ADR, doctor, capability matrix.
- **3 QA** — E2E, покрытие ≥70 %, audit, feature freeze.
- **4 Release** — release.yml, CHANGELOG, bump 1.0.0, тег, Release.
- **5 Отчёт** — `[RELEASE v1.0.0]` Product Owner.

---

## 6–8

Стандарты кода, отчётность и старт — как в исходной директиве Product Owner от 2026-08-19. Релиз не объявляется без чек-листа §2 с доказательствами.
