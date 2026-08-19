# E3 — Write path (v2), Ф2

Для: Core (host/protocol), UI (git/files). Integration не в этом срезе.
От: Architect. Дата: 2026-08-19. Не код.
База: brief №9, №15; ADR-0005; [e2-ladder-v2](e2-ladder-v2.md) `kind=edit`; matrix C27–C31, C64; directive E3; [git-files-v1](git-files-v1.md).
Протокол: minor bump **1.2** на новых методах. 1.0 / 1.1 не ломать. Конверт camelCase. Storage: **миграции 0004 нет** (новых таблиц нет). 0001–0003 не трогать.

## Закон

1. Мы не IDE (brief №9): file tree + diff + **open in editor**. Встроенного редактора нет.
2. Каждый **agent** turn с edit/exec — лестница (brief №15). Host-mediated write = `kind=edit` из e2-ladder-v2. Тот же `agent.approval` / `approval.respond`. Новых ladder-методов нет.
3. `git.push` — только **system git**. Кредов в `host.db` / pid.json / наших конфигах нет (ADR-0005, C74). Нет поля PAT в Settings. Не можем без секрета — эскалация PO, не обход.
4. GUI **не** спавнит `git` и **не** пишет файлы сам. I/O у host (brief №1).
5. Имя на проводе и в UI — **Task**, не epic (brief №16, ADR-0004).

## Решение по C64 (закон)

**C64 Epic PR View — later, не Ф2.**

Это GitHub-поверхность (checks, commits, files, local diffs; Desktop 1.1.10 #870), не локальный write path. Нужен remote + сеть + `gh`/`git` auth. В Ф2 нет `git.push` как повседневной привычки ещё, и ADR-0004 запрещает Epic-брендинг.

- В матрице: epic остаётся E3, wave **later** (после C31, не стартует Ф2).
- Когда дойдём: называется **PR view**, не Epic. Checks — через system `gh`/git, без токена в host.db.
- Ф2 **не** делает PR create, review comments, CI checks.

## Что уже есть (не повторять)

| Есть | Где |
|---|---|
| `files.tree` / `files.read` | 1.0 |
| `git.status` / `git.diff` RO | 1.0, git-files-v1 |
| `worktree.ensure` / `get` / `list` | 1.0 |
| Лестница, `policy.*`, `approval.respond`, WS `agent.approval` | 1.1, e2-ladder-v2 |
| GUI git-панель RO + file tree | v1 / Ф1 |

Корень FS/git как в git-files-v1: `worktree.path` если есть `worktreeId` / агент с worktree, иначе `workspace.path`. `..` / escape → `invalid_params`. Не git → `invalid_params` (`not_git` в message) на git-методах.

## Must Ф2

| ID | Что |
|---|---|
| C27 | `files.write` + `files.patch` за лестницей `kind=edit` |
| C28 | Diff-ревью в GUI: apply (= stage / keep) и revert (`git.restore`) |
| C29 | `files.open` — открыть путь во **внешнем** редакторе (`xdg-open` на Linux) |
| C30 | `git.stage` / `git.unstage` / `git.commit` (локально, без сети) |
| C31 | `git.push` = `git push`, ADR-0005 |

## Два контура записи

**A. Harness-direct.** `cli.claude` / `cli.codex` / `cli.generic` уже пишут в worktree сами. Ф2 **не** перехватывает их syscall. Их пишет exec-лестница `agent.send` (Ф1). После turn dirty видно в `git.status` / `git.diff`.

**B. Host-mediated.** GUI или будущий adapter зовут `files.write` / `files.patch`. Всегда `agentId` + лестница `kind=edit`, если не yolo / не `allow-always` / не уже разрешённый этот turn.

Пользовательские git-кнопки (stage / unstage / restore / commit / push) — **явное действие**, не agent turn, **карточки ask нет** (как confirm push в ADR-0005 / ADR-0008). `files.open` — не запись.

Pending-patch таблица не нужна: источник ревью — working tree + `git.diff`.

## Protocol 1.2

Существующие методы остаются на своих minor. Новые — все `{major:1, minor:2}`:

```
files.write     1.2
files.patch     1.2
files.open      1.2
git.stage       1.2
git.unstage     1.2
git.restore     1.2
git.commit      1.2
git.push        1.2
```

Handshake: GUI Ф2 объявляет эти восемь. Клиент без 1.2: RO `git.*` / `files.read` живы, write → `version_mismatch` / не в `accepted`. Host не ломает v1 send и 1.1 policy.

Новые коды (ветки по `code`): `git_identity`, `git_auth`, `git_conflict`, `patch_failed`. Уже есть: `denied`, `approval_expired`, `file_too_large`, `file_binary`, `invalid_params`. В логах и `message` **не** повторять секреты (redact `https://x:y@` / token-like).

`summary` в `agent.approval` для edit: коротко, например `write src/lib.rs` / `patch 3 files`.

### `files.write`

Params:

```json
{
  "workspaceId": "…",
  "worktreeId": null,
  "agentId": "…",
  "path": "src/lib.rs",
  "content": "…"
}
```

`path` относительный, UTF-8. Потолок `content` = как `files.read` (сейчас 256 KiB) → `file_too_large`. Не UTF-8 / бинарь → `file_binary`. Родитель должен существовать (без `mkdir -p`). Overwrite ок, delete этим методом нельзя. Ok: `{ "path": "src/lib.rs", "bytes": 123 }`.

Лестница: как `agent.send` при ask — WS `kind=edit`, ждать `approval.respond`. deny → файла нет. Пока висит approval, повторный write этого агента → `agent_busy`.

### `files.patch`

Params: `{ workspaceId, worktreeId?, agentId, patch }` — один unified diff, UTF-8, потолок как `git.diff` (256 KiB суммарно). Apply строго внутри корня. Конфликт / не применился → `patch_failed`, дерево не «почти применено» (атомарно на сколько позволяет `git apply --check` затем apply). Лестница как у `files.write`.

Ok: `{ "paths": ["src/lib.rs"], "hunks": 2 }`.

Hunk-picker в GUI — не Ф2 (весь patch целиком).

### `files.open`

Params: `{ workspaceId, worktreeId?, path }`. Файл существует, под корнем. Host: Linux `xdg-open` на canonical path (не `$EDITOR` в терминале). Ok: `{ "opened": true }`. Нет редактора / xdg → `internal` с понятным message. GUI не рисует textarea.

### `git.stage` / `git.unstage`

Params: `{ workspaceId, worktreeId?, paths: ["src/lib.rs"] }`. `paths` 1…500. `git add --` / `git restore --staged --`. Ok: тот же shape, что `git.status` после операции (клиент может и сам перечитать).

### `git.restore` (C28 revert)

Params: `{ workspaceId, worktreeId?, paths: […], staged?: false }`.

- tracked: `git restore --worktree` (+ `--staged` если `staged=true`)
- untracked: unlink, только если path под корнем

Это revert в ревью. Ok: `git.status`.

### `git.commit`

Params: `{ workspaceId, worktreeId?, message }`. `message` не пустой, ≤ 4 KiB. Только локальный commit, без `-a` (что в индексе). Нет `--force` / amend / sign flags.

Автор — `user.name` / `user.email` из git config репо или global. Нет → `git_identity`, GUI: «настрой `git config user.email`». Не пишем identity в host.db.

Hooks и `commit.gpgsign` — как у system git; ключи не наши. Падение hook/gpg → `internal` + stderr без секретов.

Ok: `{ "commit": "<sha>", "branch": "…" }`.

### `git.push`

Params: `{ workspaceId, worktreeId?, remote?: "origin", ref?: null }`.

Host: `git push <remote> <ref>` в корне. Default remote `origin`, ref = текущая ветка. **Нет** `--force` / `--force-with-lease` / tags / `--mirror`. Timeout на усмотрение host (минуты, не вечность).

Креды: credential helper / env / OS keyring. Host **не** передаёт `--extra-header` с токеном из наших файлов.

Auth fail → `git_auth`. GUI: «войди в git/gh на машине». Не открываем поле пароля.

Reject/non-fast-forward → `git_conflict`. Успех: `{ "remote": "origin", "ref": "main", "ok": true }`.

Fetch/pull — не Ф2 (тот же ADR, когда понадобится).

## GUI Ф2

- Git-панель: список status, checkbox stage/unstage, поле commit + кнопка, **Push** с confirm (ADR-0008).
- Diff: кнопки **Revert** (→ `git.restore`) и **Stage** (C28 apply = принять в индекс). Нет встроенного редактирования hunk.
- File tree: пункт **Open in editor** → `files.open`.
- Approval card Ф1 уже умеет `kind=edit` — не дублировать.
- Нет Settings «GitHub token». Push fail — текст ошибки + hint про helper/`gh auth`, не форма секрета.
- После mutate сам перечитывает `git.status` / `git.diff` / `files.tree`. Новый WS не обязателен.

## Вне скоупа Ф2

- C64 PR view / checks / PR create
- `files.delete` / rename / watch / mkdir tree
- force push, amend, fetch/pull
- in-app editor, LSP, unsaved buffers
- hunk-level apply
- перехват syscall харнесса
- секреты в БД (C74)
- Epic Mode / extension Phase (C71, C72)

## Приёмка Ф2

1. `files.write` при ask → WS `kind=edit` → deny: файла нет; allow-once: файл есть, mode остаётся ask.
2. Yolo / allow-always: write без карточки.
3. Клиент без 1.2: RO git жив, write не принят.
4. `git.stage` → `git.commit` локально, сеть не нужна, в `host.db` нет колонок token/pat/password.
5. `git.push` зовёт system `git`; auth fail → `git_auth`, БД не меняется секретом.
6. Revert файла возвращает содержимое HEAD (tracked) / удаляет untracked.
7. `files.open` не открывает редактор внутри GUI.
8. C64 в продукте нет (и не называется Epic).
9. `--force` в argv host не формирует.

Core: protocol + host (+ тесты кодов и «нет секрета в sqlite»). UI: панель. Спека закрывает контракт; код — следующие STAR.
