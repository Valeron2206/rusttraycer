#!/usr/bin/env bash
# Master-e2e (directive-v2 §5 / docs/f7-release-v2.md smoke).
# Wires in-tree host-API integration tests + rt-cli. No GUI (no headless
# test binary; rt-gui has autotests = false).
#
# Later holes must not fail this job:
#   C37 terminals outside Task — e4 asserts shell.create without taskId is
#     invalid_params (unsupported, not a product feature).
#   C42 PDF — e5 asserts artifact.export format=pdf is invalid_params.
#   C51 multi-account, C53 mid-turn steer, C58 rt-sync daemon, C63 chrome,
#     C64 Epic PR View — not selected; no must-pass cases.
#   e9_sync is C57 export/import, not the C58 rt-sync daemon.
# Git-history migration byte-identity tests are skipped (shallow clone).
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
  -- --skip migrations_0001 --skip 0001_0005_untouched

# §5 worktree path lives in rt-host lib tests (not a later hole).
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
