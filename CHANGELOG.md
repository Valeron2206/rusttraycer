# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.1.0] — 2026-08-19

Complete DoD. Protocol 1.9. Storage 0009 + 0010. Zero `missing`/`partial` in the parity matrix. Tag is Chief's after APPROVE 0097+0098.

### Added

- Task / workspace / artifact search (`search.query`).
- Terminals and Shell without a Task (workspace required).
- Artifact PDF export (`format=pdf` → 200).
- Multi-account labels per provider (credentials stay in env/keyring).
- Mid-turn steer (`agent.steer`; `cli.claude` + `cli.codex`).
- Self-hosted `rt-sync` (`sync.push` / `sync.pull`).
- Resource monitor, notification hooks, prompt stash, drag-to-tile.
- PR view via system `gh`.
- Worktree cleanup (`worktree.gc`) and branch prefix.
- Nested `AGENTS.md`, user presets, `rt-cli logs --follow`.

### Out of scope (unchanged)

- C26, C66–C75: telemetry, managed cloud, CRDT, Windows/WSL package, secrets in `host.db`, extra harnesses as required, own inference, sharing/SSO.

## [2.0.0] — 2026-08-19

Parity release against Traycer Desktop 1.1.x (local-first). Protocol 1.8. Linux x86_64 packages; macOS aarch64 compile/CI only. Tag `v2.0.0` at `ded044c`.

### Added

- Permission ladder (ask default, explicit Yolo). Write path: `files.write`, diff apply/revert, `git.commit` / `git.push` without stored credentials.
- Agent Terminal, Shell, mux; resume via provider session id.
- Artifacts (spec/ticket/story/review), comments, Markdown export; artifacts survive transcript delete.
- Agent-to-agent (reference / transcript / delivery), child agents, loops with max-iterations.
- Same-`agentId` harness/model switch, named model profiles, remembered model/effort/fast.
- Agent roles, workspace `AGENTS.md`, selection guides, planning/review/debug/document presets.
- Durable export/import (clone-not-migrate). `GET /metrics` (loopback). CLI `status` / `logs` / `reset-db`.
- GUI: harness picker, N agents, split view, artifact/A2A/sync chrome.
- Linux AppImage and `.deb` beside the existing tarball. `macos-latest` CI compile.

### Out of scope / later (not in 2.0.0)

- Windows/WSL packages, `.rpm`, signed macOS dmg, PDF export, `rt-sync`, terminals outside a Task, PR view, multi-account, mid-turn steer, search, telemetry, managed cloud.

## [1.0.0] — 2026-08-19

First production release. Linux x86_64 only ([ADR-001](docs/adr/0001-target-platforms.md)).

### Added

- Local host daemon (`rt-host`): loopback HTTP/WS, handshake per-method `{major,minor}`, SQLite `host.db`.
- Thin GUI (`rt-gui`, eframe/egui): Task list, chat canvas, host doctor screen, git status/diff panel, isolate-to-worktree, Stop → `agent.cancel`. One agent per Task, provider hard-coded `cli.generic` (no picker).
- CLI (`rt-cli`): `start`, `stop`, `doctor`.
- Host/RPC/doctor: harnesses `cli.generic` (stdin JSON messages / stdout text), `cli.claude`, `cli.codex`; N agents per Task on the host.
- `agent.cancel` (RPC 1.0): idempotent; inflight kill → `idle`; no send queue ([ADR-0002](docs/adr/0002-agent-cancel.md)).
- Worktrees per agent (`worktree.ensure` / `get` / `list`); host owns `git worktree add`.
- Read-only `files.tree` / `files.read` and `git.status` / `git.diff`.
- `GET /health` without a session. Routes: `/rpc`, `/health`, `/ws` (no `GET /metrics`).
- Graceful host shutdown (SIGTERM / `rt-cli stop`): WAL, `host.going_away`, pid.json.
- CI on `ubuntu-latest` (fmt, clippy, test) plus cargo-audit. rust-toolchain 1.85.

### Out of scope (not in 1.0.0)

- macOS and Windows (post-v1).
- PTY, terminal multiplexer, agent-to-agent, cloud sync, live collab.
- `files.write`, `git.commit` / `git.push`.
- CLI `status` / `logs` / `reset-db` (not implemented; use `doctor` + the log file path).
- `GET /metrics`. GUI picker / N agents in the canvas (host already allows both).

[1.0.0]: https://github.com/Valeron2206/rusttraycer/releases/tag/v1.0.0

[2.1.0]: https://github.com/Valeron2206/rusttraycer/releases/tag/v2.1.0

[2.0.0]: https://github.com/Valeron2206/rusttraycer/releases/tag/v2.0.0
