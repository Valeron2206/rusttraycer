# v1.0 vs v0 specs

The `*-v0.md` files are the original MVP contracts. They stay. This page lists **what shipped in 1.0 that those drafts still describe as “later / only generic / no cancel”**.

Do not treat a v0 “вне MVP” line as current if it is listed here.

## Shipped (code + handshake 1.0)

- Host/RPC/doctor: harnesses `cli.generic` \| `cli.claude` \| `cli.codex` (`agent.create` allowlist). doctor lists all three. Host accepts N agents per Task.
- `agent.cancel` — [ADR-0002](adr/0002-agent-cancel.md), contract [agent-cancel-v0.md](agent-cancel-v0.md).
- Git/files/worktree — [git-files-v1.md](git-files-v1.md): `worktree.*`, `git.status`, `git.diff`; files still read-only.
- GUI: Stop (`agent.cancel`), git panel, isolate-to-worktree. **One agent per Task, only `cli.generic`, no picker.**
- `GET /health` (no session). No `GET /metrics`.
- Platforms: **Linux x86_64 only** — [ADR-001](adr/0001-target-platforms.md).

## Still not shipped (v0 “после MVP” remains true)

- PTY, mux, Shell-as-entity, A2A, cloud, files.write, git.commit/push.
- `GET /metrics`.
- GUI harness picker and N agents in the canvas (host already has both).
- `rt-cli` has **start / stop / doctor** only (no status/logs/reset-db in this tree).
- macOS / Windows.

## File-by-file

| File | How to read it now |
|---|---|
| architecture-v0.md | Crate map and invariants still hold. Host: three harnesses + cancel. GUI still one `cli.generic` per Task. |
| protocol-v0.md | Envelope, versions, entity JSON still law. Extra methods: cancel, worktree.*, git.* (all 1.0). §9 cancel question → ADR-0002. |
| host-runtime-v0.md | Supervisor + pid.json still law. Runtime backends include claude/codex. worktree.rs is implemented (not empty). |
| runtime-adapters-v0.md | generic wire unchanged. claude/codex **are** implemented (no longer “do not implement”). doctor lists all three. |
| storage-v0.md | 0001 still law. 0002 adds `worktrees` (see git-files-v1). |
| gui-ia-v0.md | Three screens still law. Plus Stop and git panel. **v1 GUI: one agent per Task, `cli.generic` only, no picker.** |
| directive-v1.md | Untouched. Release DoD. |
