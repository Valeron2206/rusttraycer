# ADR-0006 — Target platforms for v2.0.0

Status: accepted (task/0038-parity-matrix-adr, 2026-08-19).
Source: `docs/directive-v2.md` E10; ADR-001 (v1 Linux x86_64); GUI rfd = xdg-portal (ADR-001 amendment).
Supersedes ADR-001 **only** for the v2 line. v1.0 remains Linux x86_64.

## Context

Traycer Desktop 1.1.10 ships macOS (arm64/x64), Linux (AppImage/deb/rpm), Windows x64 (+ WSL). Our v1.0 is Linux x86_64: eframe features `x11`, `rt-gui` rfd override `xdg-portal` + `async-std`. Enabling macOS/Windows is a code/CI change (target-cfg), not a docs flip.

## Decision

1. **v2.0.0 required:** Linux x86_64 (keep ADR-001) and Linux packages **AppImage + .deb** (E10). CI stays `ubuntu-latest` as the gate.
2. **v2.0.0 target:** **macOS aarch64**. Land target-cfg for eframe/rfd in E10. Intel Mac is best-effort, not DoD.
3. **v2.0.0 out: Windows.** Reason: current GUI pins (`x11`, `xdg-portal`) and no `windows-latest` CI. Traycer has Windows; we take it in **v2.x** after macOS, not in the 2.0.0 tag.
4. `.rpm` is optional, not DoD. WSL is not a supported target.

Rejected: “Windows in 2.0.0 to match Traycer install table”; rejected: drop Linux packages.

## Consequences

- Windows / WSL rows: **out-of-scope-by-ADR** for v2.0 → this file (revisit v2.x).
- AppImage, .deb, macOS aarch64: **missing → E10 / Ф6**.
