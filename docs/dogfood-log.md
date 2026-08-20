# Dogfood log

Law: [directive-v3.md](directive-v3.md). Cycle 1: **2026-08-19 → 2026-08-26 YEKT**.

Ids: `DF-NNN` (001…). Category: `bug` | `friction` | `idea`. Severity: `P0` | `P1` | `P2`.
Status: `open` | `tasked` | `closed`. Empty coverage = no live session yet.

## Findings

| id | category | severity | zone | session | status | note |
|---|---|---|---|---|---|---|
| DF-001 | bug | P1 | connect/host | 0104 | closed | host 0102 at 32957 / df-0102-home stayed up through 0104 retry; search/stash/metrics reached (RPC as GUI client) |
| DF-002 | bug | P1 | git.commit | 0102-session1 | closed | resolved: commit works with env identity, no git config. 0105 (`93ed298`) host accepts GIT_AUTHOR_NAME/EMAIL + GIT_COMMITTER_*; retested 0105-int on :40481. First commit 87476362. |
| DF-003 | friction | P2 | doctor / harness | 0102-session1 | open | `rt-cli doctor` reports `cli.generic` unavailable when `RUSTTRAYCER_GENERIC_CMD` is unset. `agent.create` with `provider=cli.generic` still succeeds. Directive says generic is always available. Reconfirmed 0102-cont: `host.doctor` + `rt-cli doctor` still `available=false` / `RUSTTRAYCER_GENERIC_CMD unset`. |
| DF-005 | bug | P1 | canvas/agents | 0109 | tasked | left Агенты pane has no ScrollArea; policy «Спросить» and «Создать агента» clipped at 1280x800. Fix in 0109. |
| DF-006 | bug | P2 | search | 0109 | tasked | Enter only on lost_focus (no-op while focused); results Window has no .open/Escape dismiss. Fix in 0109. |
| DF-007 | bug | P2 | stash | 0109 | tasked | В stash left composer uncleared; apply_stash appended. Live concat `0109 stash draft`+old body. Fix in 0109. |

## Coverage — epic → last live session

| Epic / surface | Last session | Harness | Date (YEKT) |
|---|---|---|---|
| Ladder ask | 0109 | n/a (pane clipped) | 2026-08-20 |
| Ladder Yolo | 0102-session1 | cli.generic | 2026-08-19 |
| Write + commit | 0102-session1 + 0105-int (commit via env identity) | cli.generic | 2026-08-19 |
| Terminal + resume | — | — | — |
| Artifacts | — | — | — |
| A2A-loop | — | — | — |
| Sync push/pull | 0102-session1 | cli.generic | 2026-08-19 |
| Search / steer / stash | 0109 | cli.generic (send blocked) | 2026-08-20 |

## Sessions

0104 attempted 2026-08-19 YEKT, harness n/a, blocked on host.

0104 retry 2026-08-19 YEKT ~23:30+: host 0102 (`32957`, `df-0102-home`) stayed up. No pixel clicks — no `rt-gui` binary and `cargo run -p rt-gui` not realistic (empty target). Drove same RPCs as GUI: `pid.json` → `GET /health` → handshake(`client=gui`, crate 2.1.1, 1.8 sync + 1.9 search/stash) → `host.ping` → `search.query` → `stash.list`/`stash.add` → `GET /metrics`. Search `q=dogfood` hit 1 open task (0102 dogfood title). Stash empty then 1 draft after add. Metrics: `rusttraycer_up 1`, 1 idle agent, 1 open task (chip would show agents=1, rss/rpc —). DF-001 closed. No new DF. Harness n/a (RPC-only).

0109 2026-08-20 YEKT ~08:10–08:22: live window vs 0104 RPC. Harness cli.generic attempted. Built cargo build -p rt-gui. Binary /workspace/rusttraycer/target/debug/rt-gui. First launch panicked missing libxkbcommon-x11.so (box env). DISPLAY=:5 RUSTTRAYCER_HOME=/workspace/df-0102-home. :40481 died mid-session; waited, did not spawn. Same hostId 01a01b47-e863-71d3-bd2d-e885cf484d7a on :41299 (pid.json 130299). GUI connected via pid.json. Window left running.

Live clicks: Search `dogfood` → 4 tasks (0111, 0108, 0103, 0102) + artifact 0112. Enter did not navigate; Escape/outside click left the dropdown pinned until q cleared (DF-006). Metrics chip `метрики 7/—/—` then `10/—/—`; click is no-op (decorative, not a DF; 0093 did not promise a popover). Opened 0108 canvas; toast `not_found: worktree 01a01d24-436c-72e0-8986-cf9bf9f0042a` (host, not UI DF). Yolo banner on (0112 / task-level, not toggled). Stash «Черновики» opened; 2 items (new 0109 draft + 0104 draft); no 1.9 degrade toast. «В stash» with `0109 stash draft` concatenated composer to `0109 stash draft0109 dogfood: sleep 20 then reply ok` and stored that (DF-007). Ladder ask not set — DF-005 pane no-scroll; policy «Спросить» and «Создать агента» below the fold at 1280x800. Provider combo all `(недоступен)`. Send and Ctrl+Enter toast `internal: RUSTTRAYCER_GENERIC_CMD unset` (DF-003 family, no running agent, steer not proven — not a new UI DF). DF-001 not reopened. Code fixes this session: one ScrollArea on agents sidebar; search Enter while focused navigates first hit; Escape + click-outside dismiss; composer clears after stash add; apply-from-palette replaces instead of append. Not pushed.


### Session 1 — 2026-08-19 YEKT — Integration / STAR 0102 (host + worktree + sync backup)

Harness: **cli.generic** (used). `cli.claude` unavailable (`claude` not found). `cli.codex` unavailable (`codex` not found).

Coverage this session:
- host start via `rt-cli start` (`RUSTTRAYCER_HOME=/workspace/df-0102-home`); `rt-cli status` alive; `GET /health` 200
- Task + `worktree.ensure` (product minted branch `rt/3fd5ef18` under host data dir; session checkout remains `task/0102-v3-df-session1-host`)
- `sync.push` to a second host on `127.0.0.1` (peer `RUSTTRAYCER_HOME=/workspace/df-0102-peer`): 1 task / 1 agent imported. Secret not required (0094).
- log written via `files.write` (ladder Yolo). `git.stage` ok. `git.commit` blocked — DF-002.

Findings: DF-003 (P2), DF-002 (P1). Product blocked the commit step. No origin push.

### Session 1 continuation — 2026-08-19 YEKT — Integration / STAR 0102 (no-commit extra RPC)

Same host process (no restart). `cli.claude` / `cli.codex` bins still absent — unused. No `git.commit`, no `git config`, no origin push, no `~/.rusttraycer`.

Passed:
- `GET /health` 200 on `http://127.0.0.1:32957` (pid 38485, `RUSTTRAYCER_HOME=/workspace/df-0102-home`)
- peer still 200 on `:38805`
- `rt-cli status` alive; `rt-cli logs --lines`; `rt-cli doctor`
- RPC `handshake`, `host.ping`, `host.doctor`, `workspace.list`, `task.list` (`status` required per spec), `task.get`, `agent.get`, `worktree.list`, `worktree.get`, `git.status` (workspace + product wt), `git.diff`, `files.read` of `docs/dogfood-log.md` (workspace + `worktreeId`), `files.write` (this note; left uncommitted)
- product worktree `rt/3fd5ef18` still clean; dirty log lives on session checkout `task/0102-v3-df-session1-host`

Host also has a second workspace from STAR 0103 (`wt-0103-v3-df-session-core`, branch `rt/b2ca7c77`) — observed, not a finding.

**Новых находок нет.** DF-003 still open (reconfirmed). DF-002 not retested (Core 0105).

### Session 0105-int — 2026-08-19 YEKT — Integration / STAR 0105 (git.commit env identity)

Host restarted on same home `df-0102-home` after phase0-merge `93ed298`. New bind `:40481`. Identity in host process env only (`GIT_AUTHOR_*` / `GIT_COMMITTER_*`). No `git config`. Handshake client=`cli`. `git.stage` + `git.commit` on session workspace `task/0102-v3-df-session1-host` returned 200 / sha `87476362`. DF-002 closed (resolved: commit works with env identity, no git config). Product wt `rt/3fd5ef18` left clean. No origin push.

## Parity-watch — cycle 1

Checked **2026-08-19** (Architect, STAR 0101).

Sources: https://github.com/traycerai/traycer/releases · https://docs.traycer.ai/changelog.md

| Source | Latest | vs matrix 1.1.10 |
|---|---|---|
| `desktop-v*` (GitHub, marked Latest) | **desktop-v1.1.10** (2026-08-06) | same |
| docs.traycer.ai/changelog | Desktop 1.1.10 notes; no newer Desktop heading | same |
| `host-v1.1.11` | Host-only tag (2026-08-07). Not `desktop-v*`. | not a matrix trigger |

**Нет дельты.** No new `Cxx`. Matrix stays 0 `missing`/`partial`.
