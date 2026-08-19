# ADR-0002 — agent.cancel

Status: accepted (Chief, 2026-08-19).
Closes protocol-v0.md §9 (open: whether MVP needs cancel).
Normative detail: `docs/agent-cancel-v0.md` (already written and implemented in the pre-phase0 tree).

## Context

DoD v1.0.0 requires this question closed: implement cancel **or** defer with a reason.
`agent.send` is one inflight turn; without cancel the user waits up to the 10 min timeout.
The contract in `agent-cancel-v0.md` is already the working design: RPC 1.0, idempotent ok, status `idle` (not `error`), process-group kill, no send queue.

## Decision

**Accept `agent-cancel-v0.md` as the v1.0 law.** Do not defer.

1. `agent.cancel` `{ agentId }` is a handshake-negotiated method `{major:1, minor:0}`.
2. No inflight → `ok { cancelled: false }`, not an error. Inflight → kill child, flush partial assistant Messages, `status = idle`, WS `agent.status idle`.
3. Cancel ≠ failure. Host restart still maps leftover `running` → `error` (storage-v0).
4. No send queue. `agent.send` while running remains `agent_busy`.
5. Runtime exposes cancel/kill of that turn’s process group (`cli.generic` and named harnesses the same way).

If the current `main` snapshot is missing the implementation, Phase 2 (directive) ports it from the old tree against this ADR + `agent-cancel-v0.md`. No second protocol.

## Consequences

- GUI Stop button is in scope (already specified in agent-cancel §5 / gui-ia).
- protocol-v0 §9 is closed: cancel is in, not postponed.
- Rejected: omit cancel until timeout; rejected: `status=error` or a `cancelled` enum variant; rejected: queueing send.
