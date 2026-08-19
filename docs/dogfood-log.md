# Dogfood log

Law: [directive-v3.md](directive-v3.md). Cycle 1: **2026-08-19 → 2026-08-26 YEKT**.

Ids: `DF-NNN` (001…). Category: `bug` | `friction` | `idea`. Severity: `P0` | `P1` | `P2`.
Status: `open` | `tasked` | `closed`. Empty coverage = no live session yet.

## Findings

| id | category | severity | zone | session | status | note |
|---|---|---|---|---|---|---|
| DF-001 | friction | P2 | doctor / harness | 0102-session1 | open | `rt-cli doctor` reports `cli.generic` unavailable when `RUSTTRAYCER_GENERIC_CMD` is unset. `agent.create` with `provider=cli.generic` still succeeds. Directive says generic is always available. Reconfirmed 0102-cont: `host.doctor` + `rt-cli doctor` still `available=false` / `RUSTTRAYCER_GENERIC_CMD unset`. |
| DF-002 | bug | P1 | git.commit | 0102-session1 | open | `git.commit` returns `git_identity`: requires `user.email` / `user.name` in git config. No host RPC to set identity. Session could not commit via product; did not set git config outside host. Assigned Core 0105 — not retested this continuation. |

## Coverage — epic → last live session

| Epic / surface | Last session | Harness | Date (YEKT) |
|---|---|---|---|
| Ladder ask | — | — | — |
| Ladder Yolo | 0102-session1 | cli.generic | 2026-08-19 |
| Write + commit | 0102-session1 (write yes; commit blocked DF-002) | cli.generic | 2026-08-19 |
| Terminal + resume | — | — | — |
| Artifacts | — | — | — |
| A2A-loop | — | — | — |
| Sync push/pull | 0102-session1 | cli.generic | 2026-08-19 |
| Search / steer / stash | — | — | — |

## Sessions

### Session 1 — 2026-08-19 YEKT — Integration / STAR 0102 (host + worktree + sync backup)

Harness: **cli.generic** (used). `cli.claude` unavailable (`claude` not found). `cli.codex` unavailable (`codex` not found).

Coverage this session:
- host start via `rt-cli start` (`RUSTTRAYCER_HOME=/workspace/df-0102-home`); `rt-cli status` alive; `GET /health` 200
- Task + `worktree.ensure` (product minted branch `rt/3fd5ef18` under host data dir; session checkout remains `task/0102-v3-df-session1-host`)
- `sync.push` to a second host on `127.0.0.1` (peer `RUSTTRAYCER_HOME=/workspace/df-0102-peer`): 1 task / 1 agent imported. Secret not required (0094).
- log written via `files.write` (ladder Yolo). `git.stage` ok. `git.commit` blocked — DF-002.

Findings: DF-001 (P2), DF-002 (P1). Product blocked the commit step. No origin push.

### Session 1 continuation — 2026-08-19 YEKT — Integration / STAR 0102 (no-commit extra RPC)

Same host process (no restart). `cli.claude` / `cli.codex` bins still absent — unused. No `git.commit`, no `git config`, no origin push, no `~/.rusttraycer`.

Passed:
- `GET /health` 200 on `http://127.0.0.1:32957` (pid 38485, `RUSTTRAYCER_HOME=/workspace/df-0102-home`)
- peer still 200 on `:38805`
- `rt-cli status` alive; `rt-cli logs --lines`; `rt-cli doctor`
- RPC `handshake`, `host.ping`, `host.doctor`, `workspace.list`, `task.list` (`status` required per spec), `task.get`, `agent.get`, `worktree.list`, `worktree.get`, `git.status` (workspace + product wt), `git.diff`, `files.read` of `docs/dogfood-log.md` (workspace + `worktreeId`), `files.write` (this note; left uncommitted)
- product worktree `rt/3fd5ef18` still clean; dirty log lives on session checkout `task/0102-v3-df-session1-host`

Host also has a second workspace from STAR 0103 (`wt-0103-v3-df-session-core`, branch `rt/b2ca7c77`) — observed, not a finding.

**Новых находок нет.** DF-001 still open (reconfirmed). DF-002 not retested (Core 0105).

## Parity-watch — cycle 1

Checked **2026-08-19** (Architect, STAR 0101).

Sources: https://github.com/traycerai/traycer/releases · https://docs.traycer.ai/changelog.md

| Source | Latest | vs matrix 1.1.10 |
|---|---|---|
| `desktop-v*` (GitHub, marked Latest) | **desktop-v1.1.10** (2026-08-06) | same |
| docs.traycer.ai/changelog | Desktop 1.1.10 notes; no newer Desktop heading | same |
| `host-v1.1.11` | Host-only tag (2026-08-07). Not `desktop-v*`. | not a matrix trigger |

**Нет дельты.** No new `Cxx`. Matrix stays 0 `missing`/`partial`.
