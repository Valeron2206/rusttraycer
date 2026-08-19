#!/usr/bin/env bash
# Master e2e (directive-v2 §5 / docs/v21-complete-v2.md).
# Host-API §5 chain + all v21_* binaries + rt-cli + pack dry-run.
# C37/C42/C51/C53/C58 are must. PDF and rt-sync are not skipped.
# Freeze-hash tests stay (baked sha256). No GUI (rt-gui autotests = false).
set -eu
cd "$(dirname "$0")/.."

cargo test -p rt-host \
  --test e2e_harness \
  --test first_slice \
  --test e2_ladder \
  --test e3_write \
  --test e4_pty \
  --test e5_artifacts \
  --test e6_a2a \
  --test e9_sync \
  --test e10_metrics \
  --test v21_pdf_agents \
  --test v21_search_gc \
  --test v21_c37 \
  --test v21_accounts_steer \
  --test v21_pr_get \
  --test v21_stash \
  --test v21_sync_presets

# §5 worktree path lives in rt-host lib tests.
cargo test -p rt-host --lib worktree

# C60 status / logs / reset-db (no live rt-host binary required).
cargo test -p rt-cli

# C61 pack contract only (no appimagetool, no AppImage/.deb).
stage="$(mktemp -d)"
trap 'rm -rf "${stage}"' EXIT
install -m 0755 /bin/true "${stage}/rt-host"
install -m 0755 /bin/true "${stage}/rt-cli"
install -m 0755 /bin/true "${stage}/rt-gui"
scripts/pack-linux.sh --dry-run "${GITHUB_REF_NAME:-v0.0.0}" "${stage}/dest" "${stage}"
