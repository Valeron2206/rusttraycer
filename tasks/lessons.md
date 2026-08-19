# Lessons

## README drift on major and minor releases

Missed twice in a row: **v2.0.0** (`ded044c`) and **v2.1.0** (`e151919`).

At both tags the parity matrix already listed PTY, terminal mux, agent-to-agent, and durable sync as shipped. README still called them "stubs only, no impl" and left install examples on an older tag (`v1.0.0`) after the current semver had moved.

Cause: README was not on the release-close gate. Specs, matrix, CHANGELOG, and crate versions moved. The front page did not.

A green matrix or a dated CHANGELOG is not a substitute for reading README.

## Release-close checklist

Run this on every future tag (major or minor) against the tree that will be tagged:

1. Diff README against `docs/parity-matrix.md`.
2. Every `shipped` C-row that README names is not called a stub, missing, or out of scope.
3. README's out-of-scope / stubs section, if present, lists only C26 and C66–C75.
4. Install tag, tarball names, and "supported package" version equal the semver being tagged.
5. A `LICENSE*` file is in the tree, and workspace / crate `Cargo.toml` has `license` set.

Do not close a release until this list is checked.
