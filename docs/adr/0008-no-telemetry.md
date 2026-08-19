# ADR-0008 — No telemetry

Status: proposed (task/0035-adr-0008-no-telemetry).
Source: `docs/directive-v2.md` §1 (do not copy Sentry/PostHog) and DoD §5 (no outbound network except explicit user action).
Applies for the whole v2 line. Not deferred.

## Context

Traycer Desktop ships managed-cloud analytics (Sentry / PostHog and similar). RustTraycer is local-first. Copying their telemetry would need a DSN, a vendor SDK, and hidden HTTP — all of which violate the v2 boundary and add a sixth escalation trigger (managed SaaS).

v1 already has no phone-home: host binds loopback (`/rpc`, `/health`, `/ws` only). Agent turns talk to a **user-chosen** local CLI (`cli.generic` / `cli.claude` / `cli.codex`). That is not product telemetry.

The spec is silent on crash reporters, usage pings, and “anonymous” metrics. Close it now so later epics cannot sneak a client in.

## Decision

**No product telemetry, ever, in v2.** Out of scope permanently — not a later epic.

1. No Sentry, PostHog, Segment, OpenTelemetry exporters, crash-pad, or any analytics/crash SDK. No DSN, no `*-telemetry*` crate, no build feature that phones home.
2. The host, GUI, and CLI make **no hidden network calls**. Outbound traffic is allowed only as a direct consequence of an explicit user action: send a turn (spawn the chosen harness), open a folder, apply a write the user approved, `git push` they confirmed, export/import they started. Loopback HTTP/WS to the local host is not “outbound”.
3. `GET /metrics` (when E10 adds it) stays on loopback, unauthenticated to the local process, and is **not** shipped to a vendor. Structured `tracing` stays local (stderr / `host.log`).
4. Reviewer rejects any PR that adds a vendor SDK, a hard-coded URL, a background upload, or an env var whose only job is a telemetry endpoint.
5. Self-hosted sync (E9) and provider CLIs are not telemetry: the user starts them, and they use system credentials (env / keyring / git credential helper), never a baked-in DSN.

Rejected: “optional opt-in analytics”; rejected: crash-only Sentry; rejected: “we will add it after v2.0”.

## Consequences

- Parity matrix row for Traycer’s Sentry/PostHog: **out-of-scope-by-ADR** → this file.
- Managed-cloud analytics remains an escalation trigger if anyone later proposes it (directive §0).
- Tests/review for DoD “no outbound except user action” use this ADR as the law.
