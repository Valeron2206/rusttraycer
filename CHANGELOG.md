# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.2.0] — 2026-08-27

Visual-parity release for official HT chrome against Traycer Desktop `desktop-v1.1.10`. Workspace crate version 2.2.0. Tag `v2.2.0` after CI (Chief). `v2.1.2` stays put.

### Added

- HT chrome match: theme tokens (0130), chrome IA (0133), white header plate (0138), header seam (0141). STAR 0143 pair-parity **yes**, scope HT only. Reference: `desktop-v1.1.10`. Report: [design-parity-report.md](docs/design-parity-report.md).

### Out of scope

- Start Page / History have no ours pair. Chat, permission ladder, and Acts 01–03 still need live official frames.
- This release does not claim product-wide visual indistinguishability.

## [2.1.2] — 2026-08-20

Cycle 1 dogfood patch. Protocol 1.9. Storage 0001–0011 (`0011_shells.sql`). Workspace crate version 2.1.2. Tag `v2.1.2` after CI (Chief). `v2.1.1` stays put.

### Fixed

- `git.commit` accepts `GIT_AUTHOR_*` / `GIT_COMMITTER_*` from the host process env (DF-002). No `git config`.
- `host.doctor` reports `cli.generic` available without `RUSTTRAYCER_GENERIC_CMD` (DF-003).
- `shell.resume` restores a PTY after host restart (DF-004).
- Agents pane `ScrollArea`; search Enter/Escape; stash composer clear + apply-replace (DF-005/006/007).
- `policy.set` workspace scope sends `workspaceId` xor `agentId` (DF-008).

### Docs

- Directive v3 + dogfood-log. Cycle-1 sessions: 11. Coverage 8/8. Parity-watch: Latest desktop still **desktop-v1.1.10**. `desktop-v1.2.0-rc.1` recorded, not a `Cxx` trigger.
- Cycle-1 hygiene ([c1-hygiene.md](docs/c1-hygiene.md)): `cargo audit` 0 vulnerabilities.

## [2.1.1] — 2026-08-19

Docs and license only. Protocol 1.9. Tag `v2.1.0` stays at `e151919`.

### Fixed

- README claimed PTY / mux / A2A / sync were stubs and pointed install at `v1.0.0`. It now matches the v2.1 matrix.

### Docs

- Dual license MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`).
- Workspace crate version 2.1.1.

## [2.1.0] — 2026-08-19

Complete DoD. Protocol 1.9. Storage 0009 + 0010. Zero `missing`/`partial` in the parity matrix. Tag `v2.1.0` at `e151919`.

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

[2.2.0]: https://github.com/Valeron2206/rusttraycer/releases/tag/v2.2.0

[2.1.2]: https://github.com/Valeron2206/rusttraycer/releases/tag/v2.1.2

[2.1.1]: https://github.com/Valeron2206/rusttraycer/releases/tag/v2.1.1

[2.1.0]: https://github.com/Valeron2206/rusttraycer/releases/tag/v2.1.0

[2.0.0]: https://github.com/Valeron2206/rusttraycer/releases/tag/v2.0.0
