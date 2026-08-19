# ДИРЕКТИВА v2: RustTraycer → полный паритет с Traycer Desktop

**Кому:** Chief
**Видимость:** Architect, Core, UI, Reviewer, Integration
**От:** Валерий (Product Owner)
**Статус:** Директива v1 закрыта (тег `v1.0.0` в origin — проверено). Эта директива активна с момента получения.
**Режим:** Полная автономия, как в v1. Роли, гейты, STAR-брифы, эскалационные триггеры и стандарты кода из `docs/directive-v1.md` §3, §4, §6 продолжают действовать без изменений. Нумерация задач продолжается (следующая — 0034). Ниже — только новое.

---

## 0. Дополнения к полномочиям

1. Сохрани эту директиву как `docs/directive-v2.md` первым коммитом Фазы 0.
2. К четырём эскалационным триггерам v1 добавляются два:
   - подключение managed-cloud / внешних SaaS (sync-серверы, коллаборация, телеметрия) — ко мне с конкретным предложением;
   - **хранение любых кредов** (git push, API-ключи провайдеров) в host.db или конфигах — запрещено; допустимы только системные механизмы (git credential helper, env, keyring). Если фича без хранения секрета невозможна — эскалация.
3. Всё остальное решаешь сам через ADR-механизм Architect.

---

## 1. Миссия

Довести RustTraycer до **функционального паритета с актуальным Traycer Desktop** (сейчас линейка 1.1.x) в рамках local-first философии проекта. Целевой релиз — **`v2.0.0`**. Эталон паритета — живой продукт: docs.traycer.ai + github.com/traycerai/traycer (release notes), а не только наш `docs/traycer-brief.md` от 2026-08-17 — продукт с тех пор ушёл вперёд.

**Философская граница (нарушать нельзя):** мы — local-first. Всё, что у Traycer живёт в их облаке (managed sync, team boards, real-time co-editing, платные планы, Sentry/PostHog-телеметрия), у нас либо реализуется self-hosted/локально, либо мотивированно выводится за скоуп через ADR. Телеметрию **не копируем вообще** — зафиксируй это ADR-ом сразу. «Тот же класс продукта» превращается в «тот же продукт по возможностям на одной машине».

---

## 2. Верифицированная база (origin/main, тег v1.0.0)

Есть: 3 харнесса в host (`cli.generic`/`cli.claude`/`cli.codex`), host принимает N агентов на Task, `agent.cancel`, `worktree.*`, `git.status`/`git.diff` (read-only), Stop и git-панель в GUI, CI (fmt/clippy/test/audit) + release workflow, покрытие 90%+ по host/storage/protocol, Linux x86_64.

Разрыв с эталоном (из `docs/v1-delta.md` + живого продукта): GUI держит 1 агента и только `cli.generic` без пикера; файлы read-only, нет `files.write`, `git.commit/push`; нет PTY/терминала/mux/Shell-сущности; нет artifacts как продукта; нет A2A и loops; нет permission ladder и Yolo; нет sync; нет model profiles / unified context; нет AGENTS.md-гайда; нет `/metrics`, расширенного `rt-cli`; нет macOS/Windows. Замечен `artifact.create` в rt-protocol — статус выясняется в Фазе 0 (реализация или задел).

---

## 3. Реестр эпиков паритета

Каждый эпик: сначала спека от Architect (`docs/<epic>-v2.md`), потом код. Инварианты `traycer-brief.md` №1–16 — закон для всех эпиков.

**E1 — Canvas parity (GUI).** Пикер харнесса при создании агента (allowlist из host, свойства из capability matrix, не хардкод), N агентов на Task в canvas (host уже умеет), переключение и статусы turn'ов, вкладки/суб-вкладки воркспейсов, split view. Самый дешёвый эпик — host-сторона готова.

**E2 — Permission ladder + Yolo.** Каждый turn с edit/exec проходит лестницу: ask → allow-once → allow-always(scope) → deny; персистентные policy per agent/workspace; Yolo = явный осознанный bypass с индикацией в UI. Ladder — предусловие E3 и E4 (brief №15).

**E3 — Write path.** `files.write`/patch-apply от агента через ladder; diff-ревью в GUI с apply/revert; open-in-editor (brief №9 — мы не IDE); `git.commit` + stage/unstage. `git.push` — только через системный git без хранения кредов, форма — ADR.

**E4 — Terminal.** PTY оживает: Shell как четвёртая сущность (brief №4), у агента интерфейсы Chat **и** Terminal, mux поверх PTY, resume через session id провайдера — не через scrollback (brief №13). Live-state PTY — отдельно, НЕ в `messages` и не durable (brief №1, №2). Chat transcript ≠ terminal scrollback — тестом.

**E5 — Artifacts.** Artifacts как first-class: создаются из turn'ов, переживают удаление транскрипта (brief №6 — тестом), таблица в storage (миграция 0003+), viewer в GUI, привязка к Task.

**E6 — A2A + Loops.** Reference ⊃ transcript ⊃ delivery строго по capability matrix (brief №7): любой агент может быть reference; чтение транскрипта и delivery — уже способности, зависящие от харнесса. Child agents в Task. Автоматические loops (дебаты/взаимное ревью) — **обязательно** с max-iterations, стоп-условиями и бюджетом turn'ов: бесконечный цикл двух агентов = P0-дефект.

**E7 — Model UX.** Аналог unified context: переключение харнесса/модели внутри одного агента с сохранением transcript (наша база это позволяет — transcript в SQLite, backend подставляется). Model profiles (именованные пресеты harness+параметры). Слот под native-провайдер по BYOA-модели (brief №8) — сам inference не делаем.

**E8 — Workspace intelligence.** Чтение `AGENTS.md` воркспейса + agent-selection guide (brief №14). Workflow-примитивы как локальные пресеты (planning / review / debug / document). Аналог «Phase & Review Workflow» живого Traycer — форма и глубина решаются ADR: brief запрещал копировать extension-loop, но продукт принёс план-слой в Desktop; Architect сверяет и решает окончательно.

**E9 — Sync (self-hosted).** Перенос/синхронизация durable-сущностей (Task/Agent/Message/Artifact) между host-инстансами по правилу clone-not-migrate, hostId каноничен (brief №2, №3). PTY/worktree/scrollback не синкаются — никогда. Реализация: экспорт/импорт как минимум, self-hosted `rt-sync` как цель — ADR. Live collab и CRDT **не делаем** (brief №12). Managed cloud — эскалационный триггер.

**E10 — Ops & platforms.** `GET /metrics`, `rt-cli status/logs/reset-db`, packaging: AppImage + .deb для Linux; macOS aarch64 — целевая (egui кросс-платформенный, rfd/gtk3-зависимость пересмотреть в ADR); Windows — решением ADR (в v2.0 или v2.x). Release-матрица в CI под принятые платформы.

---

## 4. Фазы

Порядок фаз обязателен (зависимости), внутри фазы — параллель по непересекающимся зонам. Перекраивать волны внутри фаз можешь сам.

- **Ф0 — Parity audit & roadmap.** Architect актуализирует `traycer-brief.md` по живым docs.traycer.ai и релизам traycerai/traycer (до текущего 1.1.x включительно), строит **`docs/parity-matrix.md`**: каждая способность эталона → источник → наш статус (shipped / partial / missing / out-of-scope-by-ADR) → эпик → волна. ADR-серия: 0003 sync-подход, 0004 план-слой (E8), 0005 git push без кредов, 0006 платформы v2, 0007 unified context, 0008 «телеметрии нет». Аудит `artifact.create`. Ты утверждаешь roadmap. Выход: матрица + ADR в main.
- **Ф1 — E1 + фундамент E2** (protocol/host-часть ladder). GUI и host — параллельно.
- **Ф2 — E3** (write path целиком, за ladder-ом).
- **Ф3 — E4** (terminal).
- **Ф4 — E5 + E6** (artifacts, затем A2A поверх).
- **Ф5 — E7 + E8.**
- **Ф6 — E9 + E10.**
- **Ф7 — Hardening & Release.** Feature freeze. Полный parity-e2e (§5), добивка покрытия, `cargo audit`, обновление всех спек до v2-состояния (`v2-delta.md` по образцу v1), CHANGELOG, bump до 2.0.0, тег, Release, smoke бинарей. Финальный проход Reviewer + Architect по parity-matrix с доказательствами.

Протокольная дисциплина: любые новые RPC/поля — через bump протокольной версии и handshake-совместимость, три плоскости версий (brief №11). Storage — только миграциями 0003+, без правки 0001–0002.

---

## 5. Definition of Done v2.0.0

- [ ] `docs/parity-matrix.md`: 100% строк в статусе **shipped** или **out-of-scope-by-ADR** (со ссылкой на ADR). Ни одной missing/partial.
- [ ] Инварианты brief №1–16: каждый закреплён автотестом либо ADR-ом — колонка в матрице.
- [ ] **Мастер-e2e в CI** (через host-API; GUI — smoke): чистый host → workspace → Task → два агента разных харнессов → worktree → агент правит файл через ladder (ask→allow) → diff → `git.commit` → artifact создан → транскрипт удалён, artifact жив → child agent получает delivery по A2A → loop двух агентов останавливается по max-iterations → PTY-сессия открыта, host перезапущен, resume по session id → экспорт durable-сущностей и импорт во второй host-инстанс (clone-not-migrate, оба hostId каноничны).
- [ ] Yolo-режим включается явно и виден в UI; ladder-политики переживают рестарт.
- [ ] Секреты нигде не хранятся (проверка Reviewer по всем эпикам E3/E7/E9).
- [ ] Телеметрия отсутствует: ни одного сетевого вызова наружу кроме явных действий пользователя — тестом/ревью.
- [ ] Качество как в v1: clippy `-D warnings` без `#[allow]`, 0 `unwrap()`/`expect()` вне `#[cfg(test)]` в новом и изменённом коде, покрытие новых модулей host/storage/protocol/runtime ≥ 70% (фактическая планка v1 — 90%+ — держать, где достижимо), fmt, audit.
- [ ] CI-матрица и release-артефакты под платформы ADR-0006; README-цикл проходит с чистого клона на каждой заявленной платформе.
- [ ] `docs/` синхронизированы (`v2-delta.md`), CHANGELOG, версии 2.0.0, тег `v2.0.0`, GitHub Release, smoke скачанного бинаря.

---

## 6. Отчётность

Как в v1: `[PHASE N DONE]` с коммитами, CI-статусом и метриками; `[BLOCKER]` — уведомление с планом, не вопрос. Финал — `[RELEASE v2.0.0]`: ссылка на тег и Release, parity-matrix с доказательствами по каждой строке, список out-of-scope-by-ADR с обоснованиями, рекомендации на v2.x.

---

## 7. Старт

Подтверди принятие одним сообщением: план Ф0 + первые задачи `0034…NNNN` с исполнителями. Дальше — полная автономия до `[RELEASE v2.0.0]`.

Не допускай ошибок. Паритет сверяй с живым продуктом и кодом, не по памяти. Сомневаешься в способности эталона — Architect верифицирует по docs/release notes и фиксирует в матрице, меня не спрашиваете.
