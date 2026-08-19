# ADR-0003 — Sync approach

Status: accepted (task/0038-parity-matrix-adr, 2026-08-19).
Source: `docs/directive-v2.md` E9 / §0; brief invariants 2, 3, 12.
Applies for the whole v2 line.

## Context

Traycer Desktop sells Cloud Sync (device switch, teams) on paid plans. RustTraycer is local-first. Managed SaaS is an escalation trigger. Live collab / CRDT were already rejected for MVP (brief №12). Durable entities (Task / Agent / Message / Artifact) may move between hosts; live PTY, worktrees, and terminal scrollback must not.

## Decision

1. **Minimum (E9):** export / import of durable entities. Clone-not-migrate: the copy is a new host's data; **both `hostId`s stay canonical**.
2. **Goal (same epic):** optional self-hosted `rt-sync` between user-owned hosts. Not a Traycer cloud.
3. **Never sync:** PTY, worktree directories, terminal scrollback, in-flight turns.
4. **Never in v2:** live collab, Yjs/CRDT, Traycer-managed sync / org SSO / seat billing.
5. Proposing a managed-cloud sync server is an escalation to the Product Owner (directive §0).

Rejected: copy Traycer Sync $10; rejected: CRDT “just for comments”.

## Consequences

- Parity rows for Cloud Sync / teams / device-switch SaaS: **out-of-scope-by-ADR** → this file.
- Export/import + optional `rt-sync`: **missing → E9 / Ф6**.
- Brief №2 and №12 stay law; tests land with E9 (and E4 must not persist PTY into the export).
