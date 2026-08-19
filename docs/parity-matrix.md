# Parity matrix — Traycer Desktop 1.1.10 → RustTraycer v2

Law: `docs/traycer-brief.md` (2026-08-19), `docs/directive-v2.md`, ADR-0001/0002/0008 + this packet (0003–0007).
Our code facts: v1.0 Linux host+gui+cli; `artifact.create` is a **stub** (0036: handshake `rejected.unsupported`, RPC `unsupported_method`) → **missing / E5**, not partial.

Status: `shipped` | `partial` | `missing` | `out-of-scope-by-ADR`.
Wave is the directive phase that **starts** the work (Ф1–Ф7). Out-of-scope rows have no wave.

Sources (live, 2026-08-19):
- https://docs.traycer.ai/ · https://docs.traycer.ai/changelog.md
- https://github.com/traycerai/traycer/releases/tag/desktop-v1.1.10
- https://docs.traycer.ai/agents-and-models/coding-agents.md

| ID | Capability | Source | Status | Epic | Wave | Invariant |
|---|---|---|---|---|---|---|
| C01 | Local host owns FS/git/agents; GUI is thin | brief №1; docs hosts | shipped | — | — | 1 |
| C02 | GUI does not spawn host | architecture-v0; rt-gui `//! No host spawn` | shipped | — | — | 1 |
| C03 | Durable chat transcript in host SQLite | storage-v0; e2e README cycle | shipped | — | — | 2 |
| C04 | `hostId` canonical | storage-v0; restart e2e | shipped | — | — | 3 |
| C05 | Host harness allowlist `cli.generic`/`cli.claude`/`cli.codex` | host `agent.create`; doctor | shipped | — | — | 8, 10 |
| C06 | Host N agents per Task | host `agent.create`; no UNIQUE(task_id) | shipped | — | — | 4 |
| C07 | `agent.cancel` + GUI Stop | ADR-0002 | shipped | — | — | — |
| C08 | `worktree.ensure`/`get`/`list` | git-files-v1; host worktree.rs | shipped | — | — | 5 |
| C09 | `git.status` / `git.diff` read-only | git-files-v1 | shipped | — | — | 9 |
| C10 | `files.tree` / `files.read` (1 MiB, binary reject) | protocol-v0; e2e | shipped | — | — | 9 |
| C11 | CLI `start` / `stop` / `doctor` | rt-cli | shipped | — | — | — |
| C12 | Linux x86_64 build + CI ubuntu | ADR-001 | shipped | — | — | — |
| C13 | Loopback `/rpc` `/health` `/ws` | host router | shipped | — | — | 1 |
| C14 | Handshake per-method `{major,minor}` | protocol-v0; brief №11 | shipped | — | — | 11 |
| C15 | `cli.generic` BYOA stdin JSON | runtime-adapters-v0 | shipped | — | — | 8 |
| C16 | Worktree isolation (files not leaked) | e2e 0002 | shipped | — | — | 5 |
| C17 | pid-lock / second host refused | e2e | shipped | — | — | 3 |
| C18 | GUI harness picker (host allowlist + caps, not hardcode) | Desktop agents panel; directive E1 | partial | E1 | Ф1 | 10 |
| C19 | GUI N agents on a Task + switch + turn status | Desktop; host already N | partial | E1 | Ф1 | 4 |
| C20 | Split view (two tabs side-by-side) | changelog; 1.1.9 #594 | missing | E1 | Ф1 | — |
| C21 | Task / workspace / artifact search (branch, folder, PR) | changelog | missing | E1 | Ф1 | — |
| C22 | Canvas tabs / workspace sub-tabs | docs tasks-and-workspace-folders | missing | E1 | Ф1 | — |
| C23 | Permission ladder ask → allow-once → allow-always → deny | Desktop agents.md Supervised/Auto-accept/Full; brief №15 | missing | E2 | Ф1 | 15 |
| C24 | Persistent policy per agent/workspace | directive E2 | missing | E2 | Ф1 | 15 |
| C25 | Explicit Yolo (visible bypass, not extension YOLO) | directive E2; not `/extension/tasks/yolo-mode` | missing | E2 | Ф1 | 15 |
| C26 | Full-access default (Traycer 1.1.x changelog) | changelog | missing | E2 | Ф1 | 15 |
| C27 | `files.write` / patch-apply behind ladder | directive E3 | missing | E3 | Ф2 | 9, 15 |
| C28 | Diff review apply/revert in GUI | directive E3; git-diff panel | missing | E3 | Ф2 | 9 |
| C29 | Open-in-editor (we are not an IDE) | brief №9 | missing | E3 | Ф2 | 9 |
| C30 | `git.commit` + stage/unstage | directive E3 | missing | E3 | Ф2 | 9 |
| C31 | `git.push` via system git, no stored creds | directive E3; ADR-0005 | missing | E3 | Ф2 | 9 |
| C32 | Agent Terminal interface (PTY + Task context) | terminal-agents-vs-terminals; Claude/Codex/OpenCode | missing | E4 | Ф3 | 4, 13 |
| C33 | Plain Shell entity (PTY, not an agent) | panels/terminals; brief №4 | missing | E4 | Ф3 | 4 |
| C34 | Terminal mux | directive E4; pty.rs/mux.rs stubs | missing | E4 | Ф3 | 4 |
| C35 | Resume via provider session id, not scrollback | brief №13; terminal-agents-vs-terminals | missing | E4 | Ф3 | 13 |
| C36 | Chat transcript ≠ terminal scrollback (tested) | brief №1, №2 | missing | E4 | Ф3 | 2, 13 |
| C37 | Terminals outside a Task / start without folder | changelog | missing | E4 | Ф3 | 4 |
| C38 | Artifacts first-class (spec/ticket/story/review) | panels/artifacts | missing | E5 | Ф4 | 6 |
| C39 | `artifact.create` (today stub: unsupported) | 0036 audit; protocol leftover | missing | E5 | Ф4 | 6 |
| C40 | Artifact survives transcript delete | brief №6 | missing | E5 | Ф4 | 6 |
| C41 | Artifact viewer + comments | panels/artifacts; comments | missing | E5 | Ф4 | 6 |
| C42 | Export artifact Markdown/PDF | changelog | missing | E5 | Ф4 | 6 |
| C43 | A2A reference (any agent) | concepts/agent-to-agent; brief №7 | missing | E6 | Ф4 | 7 |
| C44 | A2A transcript read (capability) | same | missing | E6 | Ф4 | 7 |
| C45 | A2A delivery (capability; Terminal inbox Claude-only at Traycer) | same; 1.1.10 full-access for A2A | missing | E6 | Ф4 | 7 |
| C46 | Child agents in a Task | directive E6; New Conversation child chats | missing | E6 | Ф4 | 7 |
| C47 | Loops with max-iterations / stop / budget (infinite loop = P0) | directive E6 | missing | E6 | Ф4 | 7 |
| C48 | Switch harness/model on same agent; transcript stays | directive E7; ADR-0007 | missing | E7 | Ф5 | 8, 11 |
| C49 | Named model profiles (harness+params) | directive E7; ADR-0007 | missing | E7 | Ф5 | 8 |
| C50 | Remembered model / effort / fast per harness | changelog | missing | E7 | Ф5 | 8 |
| C51 | Multi-account per provider (switch per conversation) | changelog | missing | E7 | Ф5 | 8 |
| C52 | Agent roles | changelog; CLI | missing | E7 | Ф5 | 4 |
| C53 | Mid-turn steer (⌘Enter) | changelog; 1.1.9 | missing | E7 | Ф5 | — |
| C54 | Read workspace `AGENTS.md` | brief №14; extension page (Desktop uses settings) | missing | E8 | Ф5 | 14 |
| C55 | Agent-selection guide (global + optional `.traycer/…`) | settings/agents; ADR-0004 | missing | E8 | Ф5 | 14 |
| C56 | Local workflow presets planning/review/debug/document | directive E8; ADR-0004 | missing | E8 | Ф5 | 14, 16 |
| C57 | Export/import durable entities (clone-not-migrate) | directive E9; ADR-0003 | missing | E9 | Ф6 | 2, 3 |
| C58 | Self-hosted `rt-sync` | directive E9; ADR-0003 | missing | E9 | Ф6 | 2 |
| C59 | `GET /metrics` (loopback only) | directive E10; ADR-0008 (not vendor) | missing | E10 | Ф6 | — |
| C60 | CLI `status` / `logs` / `reset-db` | directive E10 | missing | E10 | Ф6 | — |
| C61 | Linux AppImage + .deb | install.md; ADR-0006 | missing | E10 | Ф6 | — |
| C62 | macOS aarch64 | install.md; ADR-0006 | missing | E10 | Ф6 | — |
| C63 | Resource monitor / notification hooks / prompt stash / drag-to-tile | changelog; 1.1.10 notes | missing | E1 | Ф1 | — |
| C64 | Epic PR View (checks, commits, files, local diffs) | 1.1.10 #870 | missing | E3 | Ф2 | 9 |
| C65 | Worktree cleanup / PR context / branch prefix | changelog | partial | E1 | Ф1 | 5 |
| C66 | Named extra harnesses (Grok, Amp, Hermes, Oh My Pi, …) as **required** ports | coding-agents.md; changelog; ADR-0007 | out-of-scope-by-ADR | — | — | 8 |
| C67 | Own inference engine | brief №8; ADR-0007 | out-of-scope-by-ADR | — | — | 8 |
| C68 | Sentry / PostHog / product analytics | changelog analytics; ADR-0008 | out-of-scope-by-ADR | — | — | — |
| C69 | Managed Cloud Sync / device-switch SaaS / teams / paid plans | pricing.md; ADR-0003 | out-of-scope-by-ADR | — | — | 2 |
| C70 | Live collab / CRDT / Yjs | brief №12; ADR-0003 | out-of-scope-by-ADR | — | — | 12 |
| C71 | Extension Phase / Plan / Review / Epic / YOLO-as-extension | /extension/*; ADR-0004 | out-of-scope-by-ADR | — | — | 16 |
| C72 | Desktop Epic Mode | 1.1.10 #749 removed; ADR-0004 | out-of-scope-by-ADR | — | — | 16 |
| C73 | Windows (and WSL) in v2.0.0 | install.md; ADR-0006 | out-of-scope-by-ADR | — | — | — |
| C74 | Store git/provider secrets in host.db | directive §0; ADR-0005 | out-of-scope-by-ADR | — | — | — |
| C75 | Sharing panel / org SSO | panels/sharing; organizations.md; ADR-0003 | out-of-scope-by-ADR | — | — | 2 |

`cli.generic` covers BYOA for C66. Adding a named harness later is Integration, not a matrix gap.

## Invariants 1–16

| # | Invariant | Locked by |
|---|---|---|
| 1 | UI ≠ Host | shipped C01–C02; GUI spawn only `#[cfg(test)]`. **Needs dedicated prod-path test** (no `Command::new(rt-host)` outside tests). |
| 2 | Durable vs live; clone-not-migrate | transcript shipped (C03). PTY-not-durable **needs test in E4**. Export rules **ADR-0003** + test in E9. |
| 3 | hostId canonical | restart e2e (C04, C17). Export **ADR-0003** + test in E9. |
| 4 | Agent ≠ harness ≠ interface ≠ shell | types in protocol today (agent+harness). Interface+shell **E4 tests**. |
| 5 | Worktree isolation | e2e worktree (C16). Cleanup UX missing (C65). |
| 6 | Artifacts survive transcripts | **needs test in E5** (C40). Today stub (C39). |
| 7 | A2A reference ⊃ transcript ⊃ delivery | **needs test in E6** + caps matrix. |
| 8 | BYOA first; inference separate | shipped `cli.generic` (C15). Inference **ADR-0007**. |
| 9 | Not an IDE | architecture; write path **E3**. |
| 10 | Capability matrix per harness | `HarnessCaps` in runtime; GUI picker **E1**. |
| 11 | Three version planes | handshake tests (C14). New fields **minor bump** (ADR-0007). |
| 12 | No CRDT unless live collab | **ADR-0003** (never in v2). |
| 13 | Terminal resume via session id | **needs test in E4** (C35, C36). |
| 14 | AGENTS.md + selection guide | **needs test in E8**; shape **ADR-0004**. |
| 15 | Permission ladder every edit/exec turn | **needs test in E2** (C23–C26). |
| 16 | We say Task, not epic | **ADR-0004**; no Epic Mode. |

## Roadmap (for Chief)

| Wave | Epic | First missing rows |
|---|---|---|
| Ф1 | E1 + E2 foundation | C18–C26, C63, C65 |
| Ф2 | E3 | C27–C31, C64 |
| Ф3 | E4 | C32–C37 |
| Ф4 | E5 then E6 | C38–C47 |
| Ф5 | E7 + E8 | C48–C56 |
| Ф6 | E9 + E10 | C57–C62 |
| Ф7 | harden / release | DoD e2e in directive §5 |

DoD v2.0.0: every row `shipped` or `out-of-scope-by-ADR`. No `partial`/`missing` left.
