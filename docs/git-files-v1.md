# git + files + worktree — P3

Для: Core (RPC + git/FS), UI (панели). Architect, 2026-08-17.
Одна страница. Не код. База: protocol-v0, storage-v0, architecture-v1.

Локи Chief: files+ без write; git.status + git.diff RO; worktree = per-agent каталог, host владеет, GUI git не спавнит. Нет commit/push/PTY/A2A. Методы **1.0**, handshake.

---

## 1. Files+

`files.tree` / `files.read` без изменений.

`files.stat` — **не** добавляем: FileEntry из tree уже даёт kind/size/mtime.

Запрещено: `files.write`, watch, delete, rename, search.

Корень обхода: если у агента есть worktree — его `path`, иначе `workspace.path`. GUI передаёт `workspaceId` + опциональный `worktreeId`. Нет worktreeId → workspace root. Escape/`..` → `invalid_params`.

---

## 2. Worktree

Не подмена `workspaces.path`. Отдельная сущность, host создаёт каталог.

```
Worktree { id, workspaceId, agentId, path, branch, createdAt }
```

Один агент — не больше одного worktree. `runLocation`: `local` (нет ряда) | `worktree` (есть ряд). Колонка `agents.run_location` уже есть; значение `worktree` добавляем в CHECK миграцией **0002** (рядом таблица).

```sql
-- 0002
CREATE TABLE worktrees (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id),
  agent_id TEXT NOT NULL UNIQUE REFERENCES agents(id),
  path TEXT NOT NULL UNIQUE,
  branch TEXT NOT NULL,
  created_at TEXT NOT NULL
);
-- agents.run_location CHECK расширить: 'local' | 'worktree'
```

Каталог: `~/.rusttraycer/host/worktrees/<agentId>/` (или рядом с workspace, на усмотрение host). Это **git worktree** если workspace — репо; иначе копия/тот же path запрещён — тогда `worktree.ensure` → `invalid_params` (`not_git`).

### RPC

`worktree.ensure` `{ agentId }` → `Worktree`
- нет агента → `not_found`
- уже есть → тот же ряд, идемпотентно
- workspace не git → `invalid_params` (code в message: `not_git`)
- host делает `git worktree add` (новая ветка от HEAD). GUI **не** вызывает git.

`worktree.get` `{ agentId }` → `Worktree` | `not_found` (local, это не ошибка для GUI: нет панели)

`worktree.list` `{ workspaceId }` → `{ items: [Worktree] }`

Нет: remove, switch branch, «existing worktree» picker. Teardown — не этот срез (orphan dirs чистит doctor later).

---

## 3. Git RO

Корень git: worktree.path если передан `worktreeId` / агент с worktree, иначе workspace.path. Не git → `invalid_params`.

`git.status` `{ workspaceId, worktreeId? }` →

```json
{
  "branch": "main",
  "dirty": true,
  "entries": [{ "path": "src/lib.rs", "status": "modified" }]
}
```

`status`: `modified` | `added` | `deleted` | `untracked` | `renamed`. Потолок 500 entries, `truncated: bool`.

`git.diff` `{ workspaceId, worktreeId?, path? }` →

```json
{
  "files": [{ "path": "src/lib.rs", "patch": "..." }],
  "truncated": false
}
```

`path` опционален (один файл). Патч UTF-8. Суммарно > 256 KiB → `truncated: true`, хвост отрезать. Бинарь → файл с `patch: null` и пропуском, не `file_binary` на весь ответ.

Запрещено: `git.commit`, `git.push`, `git.stage`, fetch.

---

## 4. Handshake / GUI

Новые методы в hello: `worktree.ensure`, `worktree.get`, `worktree.list`, `git.status`, `git.diff`. Все `{major:1,minor:0}`.

GUI: панель Git Diff + status; кнопка «изолировать» → `worktree.ensure` выбранного агента. Дерево файлов после ensure читает `worktreeId`. Не спавнить `git`.

WS не обязателен. После ensure GUI сам перечитывает tree/status.

---

## 5. Не открываем

commit/push, PTY, A2A, files.write, multi-workspace worktree, cloud sync.
