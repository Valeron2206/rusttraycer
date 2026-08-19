# v2.0 vs v1.0 / v0 specs

The `*-v0.md` files stay. This page lists **what shipped in 2.0.0** that v1 / those drafts still describe as later, and what is still later or out of scope.

Law: [f7-release-v2](f7-release-v2.md). Protocol **1.8**. Storage **0001–0008**. No 0009. Linux x86_64 packages; macOS aarch64 is compile/CI, not a signed package.

Do not treat a v0/v1 “not shipped” line as current if it is listed under Shipped here.

## Shipped (2.0.0)

### Host / protocol / storage

- Handshake minors **1.1–1.8** (policy, write, pty, artifacts, A2A, switch/profiles, guides/roles/presets, `sync.export` / `sync.import`). 1.0 clients keep older methods.
- `GET /metrics` on loopback, sessionless, Prometheus text ([e10-ops-v2](e10-ops-v2.md), C59). Still no vendor scrape (ADR-0008).
- Migrations **0003–0008** (policies, terminal columns, artifacts+comments, loops, model UX, roles/presets). 0001–0002 byte-frozen.
- Permission ladder **ask** default; explicit Yolo (C23–C25). Traycer full-access default is **oos** (C26).
- `files.write`, `git.commit` / stage, `git.push` via system git — no tokens in `host.db` (C27–C31, ADR-0005).
- Agent Terminal + Shell + mux; resume by `provider_session_id`, not scrollback (C32–C36).
- Artifacts spec/ticket/story/review, comments, survive `agent.clear_transcript`; **Markdown** export (C38–C42 MD).
- A2A reference ⊃ transcript ⊃ delivery; child agents; loops with required `maxIterations` (C43–C47).
- `agent.switch` same `agentId`; model profiles; `harness_prefs` (C48–C50).
- Roles, workspace `AGENTS.md`, global + `.traycer` selection guides, four task presets (C52, C54–C56).
- Durable export/import JSON, clone-not-migrate, both `hostId`s canonical (C57).

### GUI / CLI / pack

- Harness picker, N agents, split canvas, Task tabs (C18–C20, C22).
- Diff apply/revert, open-in-editor, ladder chrome, artifact viewer, A2A/loop UI, switch + profiles, roles/presets/guides, export/import files.
- `rt-cli`: `start` / `stop` / `doctor` **plus** `status` / `logs` / `reset-db --yes` (C11, C60).
- Release artifacts: linux-x86_64 tarball + **AppImage** + **.deb** (C61). CI `macos-latest` compile (C62).

## Later (v2.x — may stay missing on the tag)

- C21 Task/workspace/artifact search.
- C37 terminals outside a Task.
- C42 PDF export (`format=pdf` → `invalid_params`).
- C51 multi-account per provider.
- C53 mid-turn steer.
- C58 self-hosted `rt-sync`.
- C63 resource monitor / hooks / stash / drag-to-tile.
- C64 Epic PR View.
- C65 worktree cleanup / branch prefix (core worktree is C08/C16).
- Nested `AGENTS.md`, user-defined presets, disable-detection.
- Intel Mac, signed/notarized macOS package, `.rpm`, `logs --follow`.

## Out of scope (ADR)

C26, C66–C75: full-access default, named extra harnesses as required, own inference, telemetry, managed cloud, CRDT, extension Phase/Epic/YOLO, Desktop Epic Mode, Windows/WSL in 2.0.0, secrets in `host.db`, sharing/SSO.

## File-by-file

| File | How to read it now |
|---|---|
| architecture-v0.md | Crate map still holds. Host/GUI/CLI match 2.0 musts above. |
| protocol-v0.md | Envelope still law. Extra methods live at 1.1–1.8; see epic specs. |
| host-runtime-v0.md | Supervisor/pid.json still law. PTY, artifacts, A2A, switch, guides, sync are implemented. |
| runtime-adapters-v0.md | generic/claude/codex shipped. Native inference still oos. |
| storage-v0.md | 0001 law. 0002–0008 added; do not edit 0001–0008 bytes. |
| gui-ia-v0.md | Three screens still law. v1 “one generic agent” is obsolete. |
| v1-delta.md | Historical 1.0 note. Prefer this file for 2.0. |
| e1–e10 / f7-release-v2.md | Freeze and later table. |
