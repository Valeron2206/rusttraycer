# ADR-001 — Target platforms for v1.0.0

Status: accepted (Chief, 2026-08-19).
Phase: 0. Directive §5 / CI matrix. Not CI config (task/0005-ci).
Amendment (2026-08-19, task/0033-adr-rfd): rfd pin vs GUI override. Decision unchanged.

## Context

DoD requires `ubuntu-latest` and leaves `macos-latest` to this ADR.
Default proposal: Linux x86_64 required; macOS aarch64 only if egui/rfd need no code changes; Windows out of v1.0.
A local CI draft that includes Windows is not law.

Workspace dependency pins (Linux-oriented):

```
eframe = { ..., features = ["default_fonts", "glow", "x11"] }
rfd    = { ..., features = ["gtk3"] }   # workspace.dependencies only
```

`rt-gui` **does not use** that workspace `rfd` pin. Crate override (fact @ 952ba8e):

```
rfd = { version = "0.15", default-features = false, features = ["xdg-portal", "async-std"] }
```

So the shipped GUI file-dialog backend is **xdg-portal**, not gtk3. The workspace `gtk3` feature is unused by `rt-gui`. CI still installs `libgtk-3-dev` (leftover / other crates); that does not make gtk3 the GUI backend.

Those eframe flags (`x11`) are still Linux. Shipping or CI-testing macOS/Windows needs Cargo target-cfg (and likely more). That is a code/build change, so the “macOS if no edits” gate fails.

`kill(pid, 0)` pid-lock is already portable. Host/CLI without GUI could run elsewhere; the **release** is the desktop loop (host + egui).

## Decision

1. **v1.0.0 supported and CI-tested platform: Linux x86_64 only** (`ubuntu-latest`).
2. **macOS aarch64 (and Intel): post-v1.** Not in `ci.yml` / `release.yml` matrix. No promise in README.
3. **Windows: post-v1.** Ignore any local windows-latest draft.
4. Release artifacts: one Linux x86_64 tarball (+ SHA256SUMS). No macOS/Windows binaries in `v1.0.0`.
5. Do not change eframe/rfd features in this ADR’s task. Platform enablement is a later ADR + UI/Core task. GUI rfd stays `xdg-portal` + `async-std` unless a later ADR says otherwise.

## Consequences

- Phase 0 CI can go green on ubuntu alone; macos job is not a blocker.
- Coverage, audit, E2E run on Linux. That is enough for DoD.
- A user may build on macOS at their own risk; we do not test or support it.
- Revisit after v1.0.0: first macOS aarch64 (target-cfg for eframe/rfd), then Windows.

Rejected: “CI macos without claiming support” (still needs feature flags or red jobs). Rejected: Windows in v1.0 to match a local draft.
