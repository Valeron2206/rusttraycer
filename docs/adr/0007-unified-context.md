# ADR-0007 — Unified context (harness/model switch)

Status: accepted (task/0038-parity-matrix-adr, 2026-08-19).
Source: `docs/directive-v2.md` E7; brief №8, №11.
Applies for the whole v2 line.

## Context

Traycer keeps a conversation when you change coding agent or model. Our transcript is already SQLite on the host; the missing piece is swapping the backend on the **same** `agentId` instead of cloning. We do not ship native inference (brief №8).

## Decision

1. One Agent = one durable transcript. The user may **switch harness and/or model on that agent**; the host swaps `AgentBackend`; messages stay. Not a clone, not a new `agentId`.
2. **Model profiles** = named presets (`harness` + params: model, effort, fast). Stored locally, no cloud.
3. A `native` / Traycer-Inference **slot** may exist as a provider id for later BYOA; **v2 does not ship an inference engine**.
4. Extra Traycer-named harnesses (Grok, Amp, Hermes, Oh My Pi, …) are **not required** for parity. `cli.generic` + `cli.claude` + `cli.codex` cover BYOA. Adding a named harness is an Integration task, not this ADR.
5. New RPC/fields need a protocol **minor** bump and handshake (brief №11).

Rejected: “new agent per model change”; rejected: ship llama.cpp / our own inference in 2.0.0.

## Consequences

- Switch + profiles: **missing → E7 / Ф5**.
- Own inference; required extra vendor harnesses: **out-of-scope-by-ADR** → this file.
