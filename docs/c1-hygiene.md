# Cycle 1 hygiene

STAR 0116. Law: [directive-v3.md](directive-v3.md) §4. Checked **2026-08-20 YEKT** on `origin/main` `bd9b061`.

Crate version 2.1.2 (STAR 0122). Tag + assets after CI (Chief).

## cargo audit

`cargo audit` on `Cargo.lock` (503 crates). Advisory DB: 1217 entries.

| Class | Count | Action |
|---|---|---|
| vulnerability | **0** | no P0 |
| unmaintained (allowed warning) | 2 | stay; transitive through `eframe`/`egui` 0.31 |

Warnings (not vulns):

- [RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436) `paste 1.0.15` via `metal` → `wgpu-hal` → `eframe 0.31` (`rt-gui`)
- [RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192) `ttf-parser 0.25.1` via `ab_glyph` / `epaint` → `egui 0.31` (`rt-gui`)

CI already runs `cargo audit` (`.github/workflows/ci.yml` job `audit`, toolchain 1.85). Job unchanged.

## Dependency review

No lockfile or pin change in this STAR. Patch/minor bumps go through the normal gate; major needs an ADR. None proposed.

Workspace pins (unchanged): `eframe`/`egui` 0.31, `rusqlite` 0.32, `axum` 0.8, `tokio` 1, `clap` 4, `thiserror` 2, `directories` 6, `tower` 0.5, `tower-http` 0.6.

The unmaintained crates are behind `eframe` 0.31. A 0.31 → 0.32 egui bump is a 0.x break — not this cycle.

Toolchain **1.85** (`rust-toolchain.toml`). Change only via ADR.

No flaky-test quarantine opened here.

## Parity

Latest `desktop-v*` = **desktop-v1.1.10** (GitHub Latest). `desktop-v1.2.0-rc.1` exists; **no new `Cxx`**. `host-v1.1.11` host-only. See [dogfood-log.md](dogfood-log.md).
