# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
