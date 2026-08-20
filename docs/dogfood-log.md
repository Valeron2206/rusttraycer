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

## Coverage — epic → last live session

| Epic / surface | Last session | Harness | Date (YEKT) |
|---|---|---|---|
| Ladder ask | — | — | — |
| Ladder Yolo | 0112 | cli.generic | 2026-08-20 |
| Write + commit | 0112 (env identity) | cli.generic | 2026-08-20 |
| Terminal + resume | — | — | — |
| Artifacts | 0112 | cli.generic | 2026-08-20 |
| A2A-loop | 0117 (loop.start maxIterations=2 stopped error GENERIC_CMD; not a new DF) | cli.generic | 2026-08-20 |
| Sync push/pull | 0102-session1 | cli.generic | 2026-08-19 |
| Search / steer / stash | 0104 | n/a (RPC-only) | 2026-08-19 |

## Sessions

0104 attempted 2026-08-19 YEKT, harness n/a, blocked on host.

0104 retry 2026-08-19 YEKT ~23:30+: host 0102 (`32957`, `df-0102-home`) stayed up. No pixel clicks — no `rt-gui` binary and `cargo run -p rt-gui` not realistic (empty target). Drove same RPCs as GUI: `pid.json` → `GET /health` → handshake(`client=gui`, crate 2.1.1, 1.8 sync + 1.9 search/stash) → `host.ping` → `search.query` → `stash.list`/`stash.add` → `GET /metrics`. Search `q=dogfood` hit 1 open task (0102 dogfood title). Stash empty then 1 draft after add. Metrics: `rusttraycer_up 1`, 1 idle agent, 1 open task (chip would show agents=1, rss/rpc —). DF-001 closed. No new DF. Harness n/a (RPC-only).

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


### Session 0117 — 2026-08-20 YEKT — Integration / STAR 0117 (live A2A-loop)

Harness: **cli.generic**. Handshake `client=cli` crate 2.1.1. Host reused `df-0102-home` (`hostId` `01a01b47-e863-71d3-bd2d-e885cf484d7a`) on `:41299`. This session did not stop or restart the host. Resume not attempted.

Passed:
- `workspace.add` `/workspace/wt-0117-v3-df-session-a2a` (session checkout `task/0117-v3-df-session-a2a` at `bd9b061`)
- `task.create` `STAR 0117 live A2A-loop` (`01a01d3e-3d72-7761-8f99-d0af51a7fe3a`)
- `agent.create` parent generic `01a01d3e-3d73-7803-8449-4001af31a9d9` + child generic `parentId` + child claude `parentId`
- `worktree.ensure` product branch `rt/af31a9d9`
- `a2a.deliver` parent→generic → `no_inbox` (caps; known)
- `a2a.deliver` parent→claude → ok `messageId` `01a01d3e-b8d1-70b0-85c4-d5449504c224`
- `loop.start` maxIterations=2 `loopId` `01a01d3e-b8d3-7970-be65-4a6e15e4ca08`; `loop.get` immediately `status=stopped` `reason=error` iteration=0 turns=0. host.log: `loop send failed ... RUSTTRAYCER_GENERIC_CMD unset`. Not infinite. Limit not reached. Not faked.

Log written via `files.write`. `git.stage` + `git.commit` via RPC (env identity). No origin push. No `git config`. 0108 `62357c6` / 0111 `79a1c67` / 0115 `5661824` not moved.

**Новых находок нет.** Loop died on GENERIC_CMD (DF-003), not a new DF per 0117 STAR. DF-003 still open.

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
