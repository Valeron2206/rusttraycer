# Dogfood log

Law: [directive-v3.md](directive-v3.md). Cycle 1: **2026-08-19 → 2026-08-26 YEKT**.

Ids: `DF-NNN` (001…). Category: `bug` | `friction` | `idea`. Severity: `P0` | `P1` | `P2`.
Status: `open` | `tasked` | `closed`. Empty coverage = no live session yet.

## Findings

| id | category | severity | zone | session | status | note |
|---|---|---|---|---|---|---|
| DF-001 | bug | P1 | connect/host | 0104 | closed | host 0102 at 32957 / df-0102-home stayed up through 0104 retry; search/stash/metrics reached (RPC as GUI client) |
| DF-002 | bug | P1 | git.commit | 0102-session1 | closed | resolved: commit works with env identity, no git config. 0105 (`93ed298`) host accepts GIT_AUTHOR_NAME/EMAIL + GIT_COMMITTER_*; retested 0105-int on :40481. First commit 87476362. |
| DF-003 | friction | P2 | doctor / harness | 0102-session1 | open | `rt-cli doctor` reports `cli.generic` unavailable when `RUSTTRAYCER_GENERIC_CMD` is unset. `agent.create` with `provider=cli.generic` still succeeds. Directive says generic is always available. Reconfirmed 0102-cont and 0112 `host.doctor`: still `available=false` / `RUSTTRAYCER_GENERIC_CMD unset`. |
| DF-004 | friction | P1 | terminal/resume | 0108 / 0119 | closed | resolved 0113; 0119 retest on 141087b: list+resume+write after restart (`:34009`→`:45927`). |
| DF-005 | bug | P1 | canvas/agents | 0109 | tasked | left Агенты pane has no ScrollArea; policy «Спросить» and «Создать агента» clipped at 1280x800. Fix in 0109. |
| DF-006 | bug | P2 | search | 0109 | tasked | Enter only on lost_focus (no-op while focused); results Window has no .open/Escape dismiss. Fix in 0109. |
| DF-007 | bug | P2 | stash | 0109 | tasked | В stash left composer uncleared; apply_stash appended. Live concat `0109 stash draft`+old body. Fix in 0109. |

## Coverage — epic → last live session

| Epic / surface | Last session | Harness | Date (YEKT) |
|---|---|---|---|
| Ladder ask | 0109 | n/a (pane clipped) | 2026-08-20 |
| Ladder Yolo | 0112 | cli.generic | 2026-08-20 |
| Write + commit | 0112 (env identity) | cli.generic | 2026-08-20 |
| Terminal + resume | 0115 (live PTY create+input; resume 0113 tasked, live retest 0119) | cli.generic | 2026-08-20 |
| Artifacts | 0112 | cli.generic | 2026-08-20 |
| A2A-loop | — | — | — |
| Sync push/pull | 0102-session1 + 0108 (push this task) | cli.generic | 2026-08-20 |
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


### STAR 0112 — 2026-08-20 YEKT — Integration / artifacts + yolo

Harness: **cli.generic** (create succeeded; doctor `available=false` — DF-003, not new). `cli.claude` / `cli.codex` bins absent.

Drove RPC as client `cli` (crate 2.1.1) against hostId `01a01b47-e863-71d3-bd2d-e885cf484d7a` (`RUSTTRAYCER_HOME=/workspace/df-0102-home`). `:40481` was gone when this session started (graceful shutdown 03:09:19Z); same hostId already listening on `:41299` (pid 130299, binary `wt-phase0` 2.1.1, GIT_AUTHOR/COMMITTER in process env). This session did not start or restart the host.

Passed:
- handshake: accepted `artifact.create` 1.4, `artifact.export` 1.9, `files.write`/`git.stage`/`git.commit` 1.2, `policy.*` 1.1, `agent.create` 1.9. rejected empty
- `workspace.add` `/workspace/wt-0112-v3-df-session-artifacts-yolo`
- `task.create` title `STAR 0112 artifacts + yolo`
- `agent.create` provider `cli.generic`
- `worktree.ensure` product branch `rt/7ef13b93` under host data dir
- `policy.set` workspace scope, `yolo=true`, mode `allow-always`
- `host.doctor` yolo=true; generic/claude/codex all `available=false`
- `artifact.create` kind=spec title `0112 yolo artifacts`
- `artifact.export` format=pdf → 200, filename `<id>.pdf`, 654 bytes starting `%PDF-1.4` (no filesystem path field)
- `files.write` this log into the product worktree (workspaceId + agentId + worktreeId)
- `git.stage` + `git.commit` (env identity; no git config)

No new DF. DF-003 reconfirmed, not closed. No origin push. Session checkout `task/0112-v3-df-session-artifacts-yolo` stays at `ec709c5` unless noted; product commit is the session commit.


### Session 0108 — 2026-08-20 YEKT — Integration / STAR 0108 (terminal + resume)

Harness: **cli.generic** (used). Handshake client=`cli`, crate 2.1.1. Host reused `df-0102-home` (`hostId` `01a01b47-e863-71d3-bd2d-e885cf484d7a`), first on `:40481`, then `rt-cli stop` + `rt-cli start` (same home, same `GIT_AUTHOR_*` / `GIT_COMMITTER_*`, no `git config`). New bind `:41299`. `cli.claude` / `cli.codex` bins still absent.

Passed:
- Task + `worktree.ensure` (product minted `rt/c26ebd40` under host data dir; session checkout `task/0108-v3-df-session-pty` at `/workspace/wt-0108-v3-df-session-pty`)
- `shell.create` with `taskId` (C32–C36): live PTY `ptyId` `01a01d24-5099-7313-bc39-21b2ec894280`, cwd session workspace
- `pty.write` input `echo df-0108-pty` — marker written, unique string observed
- `sync.push` backup to peer `:38805` (`/workspace/df-0102-peer`) **before** restart: this task only (`taskIds` filter). Full-host push conflicted with existing 0102 task on peer. 1 task / 1 agent imported. Secret not required.
- host restart: `rt-cli stop` + `start`, same home, same hostId, port `:40481` → `:41299`

Resume failed (DF-004, not faked):
- `shell.list` empty after restart
- `pty.write` old `ptyId` → `pty_dead`
- `pty.open` old `shellId` → `not_found`
- `shell.resume` → `unsupported_method`
- `agent.create interface=terminal provider=cli.generic` → `not_pty`
- chat agent `providerSessionId` remains null

Log written via `files.write` (ladder Yolo). `git.stage` + `git.commit` via RPC (env identity). No origin push. No `git config`.

Findings: DF-004 (P1, new). DF-003 still open (reconfirmed).

### Session 0115 — 2026-08-20 YEKT — Integration / STAR 0115 (live PTY)

Harness: **cli.generic** (create succeeded; doctor `available=false` — DF-003, not new). `cli.claude` / `cli.codex` bins absent (`which claude` / `which codex` empty).

Drove RPC as client `cli` (crate 2.1.1) against hostId `01a01b47-e863-71d3-bd2d-e885cf484d7a` (`RUSTTRAYCER_HOME=/workspace/df-0102-home`) on `:41299` (pid 130299, same process as 0112; `GET /health` 200). This session did not start or restart the host. 0113 not merged — resume not attempted (`shell.resume` / vendor resume not called).

Passed:
- handshake: accepted `shell.create` 1.9, `pty.write` 1.3, `files.write`/`git.stage`/`git.commit` 1.2, `policy.*` 1.1, `agent.create` 1.9. rejected empty
- `workspace.add` `/workspace/wt-0115-v3-df-session-pty`
- `task.create` title `STAR 0115 live PTY`
- `agent.create` provider `cli.generic`
- `worktree.ensure` product branch `rt/469e4b45` under host data dir
- `policy.set` workspace scope, `yolo=true`, mode `allow-always`
- `host.doctor` yolo=true; generic/claude/codex all `available=false` (DF-003 reconfirmed)
- `shell.create` with `taskId` (C32–C36): live PTY `shellId` `01a01d3b-9fc8-7932-9276-22bbd5bc6cb5`, `ptyId` `01a01d3b-9fc8-7932-9276-22c18c25a86f`, cwd `/workspace/wt-0115-v3-df-session-pty`
- `pty.write` input `echo df-0115-pty` — marker observed: WS `pty.data` on that `ptyId` contained the unique string; command output also written to `/tmp/df-0115-marker.txt` (`df-0115-pty`). `shell.list` still shows the live shell.
- second live PTY this session (same host, no restart): `shellId` `01a01d3c-3760-7671-b2e3-9010ffc37f15`, `ptyId` `01a01d3c-3760-7671-b2e3-902119e86994`; `pty.write` base64 `echo df-0115-pty` observed in `/tmp/df-0115-pty.marker`
- `files.write` this log into the session checkout (workspaceId + agentId; no worktreeId)
- `git.stage` + `git.commit` (env identity; no git config)

Resume not attempted (0113 not merged). No host restart. 0111 worktree/branch not touched. No origin push.

**Новых находок нет.** DF-003 still open (reconfirmed). DF-004 (0108 P1 terminal/resume) is not on this base and was not re-opened here.



### Session 0119 — 2026-08-20 YEKT — Integration / STAR 0119 (PTY resume on 141087b)

Harness: **cli.generic**. Handshake `client=cli`, `shell.resume` 1.10. Rebuilt `rt-host` from `/workspace/wt-phase0` (0115 merge `141087b` ancestor). `rt-cli stop`+`start` same `df-0102-home` / hostId `01a01b47-e863-71d3-bd2d-e885cf484d7a`. Bind after rebuild `:34009`. Mid-session restart `:45927`. 0118 tip `defbded` not moved. No origin.

Passed:
- session checkout `task/0119-v3-df-session-resume` at `141087b`
- create `shellId` `01a01d45-9833-7832-9620-b15050669c97` `ptyId` `01a01d45-9833-7832-9620-b16f5472ca5c`
- write `echo df-0119b-pre` → `/tmp/df-0119b-pre.marker`
- restart same home
- `shell.list` by taskId still has the shell
- `shell.resume` new `ptyId` `01a01d45-faf2-7df3-8051-92d99ed2d77f`
- write `echo df-0119b-post` → `/tmp/df-0119b-post.marker`

DF-004 closed. One sha.

## Parity-watch — cycle 1

Checked **2026-08-19** (Architect, STAR 0101).

Sources: https://github.com/traycerai/traycer/releases · https://docs.traycer.ai/changelog.md

| Source | Latest | vs matrix 1.1.10 |
|---|---|---|
| `desktop-v*` (GitHub, marked Latest) | **desktop-v1.1.10** (2026-08-06) | same |
| docs.traycer.ai/changelog | Desktop 1.1.10 notes; no newer Desktop heading | same |
| `host-v1.1.11` | Host-only tag (2026-08-07). Not `desktop-v*`. | not a matrix trigger |

**Нет дельты.** No new `Cxx`. Matrix stays 0 `missing`/`partial`.

### Mid-cycle — 2026-08-20 YEKT (Architect, STAR 0110)

Sources: https://github.com/traycerai/traycer/releases · https://docs.traycer.ai/changelog.md

| Source | Latest | vs matrix 1.1.10 |
|---|---|---|
| `desktop-v*` (GitHub, marked Latest) | **desktop-v1.1.10** (2026-08-06) | same |
| `desktop-v1.2.0-rc.1` | prerelease (2026-08-19 21:36 UTC). Desktop tag exists; GitHub Latest is still 1.1.10. | not a matrix trigger |
| docs.traycer.ai/changelog | still 1.1.10 notes; no 1.2.0 heading | same |
| `host-v1.1.11` | Host-only tag (2026-08-07). No `desktop-v1.1.11`. | not a matrix trigger |
| `host-v1.2.0-rc.1` / `cli-v1.2.0-rc.1` | prerelease, paired with desktop rc | not a matrix trigger |

Triage: Cxx only when changelog or a non-rc `desktop-v*` ships a capability (0101 rule; host-only / prerelease stay in the report). Opening `missing` against an RC would start the one-cycle clock before the product exists.

Watch (do not open Cxx until `desktop-v1.2.0` or a changelog heading): Devices & Sessions + OTP; communication graph; remote-host / multi-host; monitors & notifying shells; chat-sync v2 / chat sharing (likely C69/C75); HuggingFace harness (C66); usage analytics (C68); Diffs 1.3.1 in-place edit; `agent.fork@1.0`; MCP/plugins/skills settings; read-only terminal access for agents.

**Нет дельты.** No new `Cxx`. Matrix stays 0 `missing`/`partial`.

### Hygiene + draft 2.1.2 — 2026-08-20 YEKT (Architect, STAR 0116)

`cargo audit`: 0 vulnerabilities (2 unmaintained warnings via eframe/egui 0.31). Note: [c1-hygiene.md](c1-hygiene.md). No lockfile change.

Parity recheck (same sources as 0110): Latest desktop still **desktop-v1.1.10**. `desktop-v1.2.0-rc.1` still prerelease. **Нет дельты.** No new `Cxx`.

CHANGELOG `[2.1.2] — Unreleased` drafted. README install stays `v2.1.1`. Tag / crate bump / assets after DoD.
