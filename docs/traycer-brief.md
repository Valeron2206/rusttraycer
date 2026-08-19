# Traycer — сверка для RustTraycer (2026-08-19)

Эталон: **Traycer Desktop 1.1.x**, не IDE-extension.
Снято с живых источников 2026-08-19 (не по памяти):

- Docs: https://docs.traycer.ai/ · index https://docs.traycer.ai/llms.txt
- Changelog Desktop: https://docs.traycer.ai/changelog.md
- GitHub latest Desktop: [desktop-v1.1.10](https://github.com/traycerai/traycer/releases/tag/desktop-v1.1.10) (2026-08-06). Host latest: 1.1.11 (2026-08-07).
- Agents catalog: https://docs.traycer.ai/agents-and-models/coding-agents.md
- Terminal vs shell: https://docs.traycer.ai/concepts/terminal-agents-vs-terminals.md
- Pricing / cloud sync: https://docs.traycer.ai/account/pricing.md
- Install / platforms: https://docs.traycer.ai/install.md

Предыдущая сверка: 2026-08-17 (инварианты №1–16 ниже **закон**, не переписывать).

RustTraycer аналог **Desktop**, не extension. Extension loop (Plan → Handoff → Verify / YOLO, Phases, Epic boards) живёт в `/extension/*` и **не копируется**.

---

## Инварианты (закон, 2026-08-17)

1. UI ≠ Host. Live FS/git/PTY/агенты в host.
2. Durable vs live. Chat/Task можно потом sync. PTY/worktree/terminal transcript — нет. Clone-not-migrate.
3. hostId каноничен.
4. Agent ≠ harness ≠ interface ≠ shell. Четыре типа.
5. Worktree — изоляция. Local / new / existing.
6. Artifacts переживают транскрипты.
7. A2A = reference ⊃ transcript ⊃ delivery.
8. BYOA first. Свой inference — отдельный provider.
9. Не IDE. File tree + diff + open in editor.
10. Capability matrix по харнессам, не один trait.
11. Три плоскости версий.
12. Yjs/CRDT только если будет live collab. MVP = sqlite.
13. Terminal resume через session id провайдера, не scrollback.
14. AGENTS.md и workspace agent-selection guide.
15. Permission ladder на каждый turn с edit/exec.
16. UI говорит Task, protocol Traycer ещё говорит epic. У нас только Task.

Desktop loop: folder → Task → agents → worktree → files/diff → artifacts → child agents.

---

## 1.1.x — что добавилось / уточнилось у эталона

Каждый пункт — факт docs или GitHub. Для матрицы (0038): missing / partial / shipped / out-of-scope-by-ADR.

### Платформы

- Desktop: macOS 12+ (Apple Silicon + Intel), 64-bit Linux (AppImage / .deb / .rpm), **Windows x64**, WSL. Источник: [install](https://docs.traycer.ai/install.md); Windows — [changelog](https://docs.traycer.ai/changelog.md) («Traycer Desktop comes to Windows»).
- Старый brief (17.08) платформы не фиксировал; v1 RustTraycer = Linux x86_64 only (ADR-001).

### Coding agents (каталог docs, не changelog-слухи)

Таблица [Agents & Models](https://docs.traycer.ai/agents-and-models/coding-agents.md) на 2026-08-19:

| Agent | Chat | Terminal |
|---|---|---|
| Claude Code | yes | yes |
| Codex | yes | yes |
| OpenCode | yes | yes |
| Traycer (inference) | yes | no |
| Cursor | yes | no |
| Grok, Qwen Code, Kiro, Droid, Kimi, Copilot, Kilo Code, OpenRouter, Amp, Devin, Pi | yes | no |

Changelog (не в этой таблице на день сверки): **Hermes** (Nous), **Oh My Pi**. Источник: [changelog.md](https://docs.traycer.ai/changelog.md). В матрице помечать «changelog-only, catalog lag».

Первый публичный релиз Desktop называл только Claude / Codex / Cursor / OpenCode / Traycer Inference. Остальные — 1.1.x.

Capability: Terminal interface только у Claude/Codex/OpenCode. A2A **delivery** на Terminal — только Claude Code ([terminal-agents-vs-terminals](https://docs.traycer.ai/concepts/terminal-agents-vs-terminals.md)). Это уточняет инвариант 7/10, не отменяет.

### Canvas / GUI (1.1.x)

Источник: [changelog](https://docs.traycer.ai/changelog.md) + Desktop 1.1.10 notes.

- Split view: любые две вкладки side-by-side (Chrome-style, #594).
- Search: artifacts, workspaces, Tasks по branch / folder / PR.
- Chats panel → **Agents**; Settings › Agents → **Agent selection** (гайд **глобальный**, не per-workspace — changelog + commit drop per-workspace guide).
- Несколько аккаунтов на один provider, switch per conversation, usage per profile.
- Agent **roles** (app + CLI).
- Steer mid-turn: ⌘Enter (не все harness).
- Fork conversation (в т.ч. пока вопрос ждёт ответа).
- Named agents; remembered model / reasoning / Fast per harness.
- Prompt stash (images too), restore across chats/windows (1.1.10, #911).
- Drag agents into tiles; drop files/folders/artifacts into composer as @.
- Resource monitor (CPU/memory, stop agent + children).
- Notification center + hooks (URL или command, filter by severity).
- Context usage chip + compact conversation.
- Start chat / terminal **without** a folder first; terminals **outside** a Task.
- Worktree cleanup (stale / open-or-merged PR / landed), configurable branch prefix.
- **Epic PR View** restored in 1.1.10 (#870): review PR (checks, commits, conversations, files, local diffs). LinkedIn 1.1.10 совпадает.
- Navigation Back/Forward inside a Task; command palette; `traycer://`; themes; summon hotkey.

### Artifacts

- Типы: specs, tickets, stories, reviews ([panels/artifacts](https://docs.traycer.ai/panels/artifacts.md), index).
- Export any artifact as **Markdown or PDF** (changelog).
- Comments on artifacts; Sharing panel for Task collaboration access.
- Artifacts survive transcript (инвариант 6) — без изменения.

### Permissions / YOLO

- Desktop changelog: **full access = default** for new conversations.
- YOLO Mode документирован в **extension** (`/extension/tasks/yolo-mode.md`), не в Desktop nav. Инвариант 15 остаётся законом для нас; форму Desktop-лестницы решать ADR / E2, не копировать extension YOLO blindly.

### Plan / Phase / Epic

- **Epic Mode удалён из Desktop** в 1.1.10 (`feat(gui-app,protocol,cli): remove Epic Mode` #749). UI = Task. Инвариант 16 для нас ещё жёстче: epic не копируем.
- Phase / Plan / Review / Epic boards / workflows / MCP / Handlebars templates — **только extension docs**. Не Desktop loop.
- Directive E8: аналог «Phase & Review Workflow» на Desktop — отдельный ADR-0004, не автокопия extension.

### Agent selection / AGENTS.md

- Desktop: [Settings › Agent selection](https://docs.traycer.ai/settings/agents.md) — глобальные инструкции, какой harness/model/effort для delegated work.
- Per-workspace agent-selection guide **снят** (changelog / #631).
- AGENTS.md page живёт в **extension** (`/extension/tasks/agents-md.md`). Инвариант 14 остаётся законом для RustTraycer (читать workspace AGENTS.md), даже если Desktop это вынес в settings.

### Sync / teams / telemetry (облако эталона)

- Планы: BYOA $0 local-only (нет cloud sync / sharing / Traycer credits); Sync $10 cloud sync + device switch + team collab; Lite/Pro/Ultra + inference credits ([pricing](https://docs.traycer.ai/account/pricing.md)).
- Sharing / Organizations — paid Desktop, не local-first default.
- Changelog: «Traycer now collects product usage data… events are linked to your Traycer account.» (analytics). **Не копируем** — ADR-0008.
- Sentry упоминается в Desktop commits (crash reporter init). Тоже ADR-0008.

### Host / CLI / ops

- Desktop сам поднимает Host; CLI bundled; sign-in once (app+CLI).
- Compatibility: newer app + older Host, fallback or ask to update.
- CLI: host control, auth, workspace, worktree, agents ([cli/commands](https://docs.traycer.ai/cli/commands.md)).
- Diagnostics log level in Settings.
- Public repo MIT (changelog).

### A2A / child agents (уточнение 1.1.x)

- Child agents из New Conversation dialog; nested Claude subagents as tree.
- A2A agents require **full access** (1.1.10, #895).
- Reference ⊃ transcript ⊃ delivery — подтверждено [A2A](https://docs.traycer.ai/concepts/agent-to-agent.md) и terminal-vs-shell page. Delivery local to same Host + same user.

---

## Что не менять при паритете

- Инварианты №1–16.
- Не копировать: managed sync, teams, paid plans, Sentry/PostHog/analytics, live collab/CRDT, extension Phases/Epic/YOLO-as-extension, credentials in host.db.
- Epic Mode Desktop снят эталоном — не воскрешать у нас.

Desktop loop (актуально 1.1.10): folder (optional at start) → Task → N agents (Chat и/или Terminal) + plain Terminals → worktree → files/diff/PR → artifacts → child agents / A2A.
