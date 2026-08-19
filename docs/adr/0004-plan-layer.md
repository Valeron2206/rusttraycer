# ADR-0004 — Plan layer (E8)

Status: accepted (task/0038-parity-matrix-adr, 2026-08-19).
Source: `docs/directive-v2.md` E8; brief №14, №16; Desktop 1.1.10 (`remove Epic Mode` #749).
Applies for the whole v2 line.

## Context

The 2026-08-17 brief forbade copying the IDE-extension loop (Plan → Handoff → Verify / YOLO). Desktop 1.1.x later grew search, artifacts, and a PR view — and **removed Epic Mode**. Extension docs still have Phases / Plan / Review / Epic / YOLO. Directive E8 asks Architect to pick a local plan layer, not to clone the extension.

## Decision

**Do not copy** extension Phases, Plan Mode, Review Mode, Epic boards, mermaid workflow engines, or YOLO-as-extension-automation.

What we **do** locally (E8):

1. Read workspace `AGENTS.md` and use it as agent context (brief №14).
2. Optional `<workspace>/.traycer/agent-selection-guide.md` plus a later global agent-selection setting (Desktop's Settings › Agent selection).
3. Named **local workflow presets**: `planning` / `review` / `debug` / `document` — templates for a Task or agent, not shared boards.
4. “Review” on Desktop = artifact type + git diff / line comments (E5 + existing git panel), not extension Review Mode.

UI and protocol say **Task** only (brief №16). We never resurrect Epic Mode.

Rejected: extension Phase UI; rejected: Epic boards / real-time ticket collab.

## Consequences

- Extension Phase/Epic/YOLO and Desktop Epic Mode: **out-of-scope-by-ADR** → this file.
- AGENTS.md + presets: **missing → E8 / Ф5**.
