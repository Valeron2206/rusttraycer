# ADR-0005 — git.push without stored credentials

Status: accepted (task/0038-parity-matrix-adr, 2026-08-19).
Source: `docs/directive-v2.md` E3 / §0 (no creds in host.db); brief №9.
Applies for the whole v2 line.

## Context

Write-path (E3) includes `git.commit` and, optionally, `git.push`. Storing GitHub / git tokens in `host.db` or app config is forbidden. If a feature cannot work without a stored secret, escalate to the Product Owner.

## Decision

1. `git.push` (and fetch) run the **system `git`** in the workspace or worktree. No libgit2 credential store of our own.
2. Credentials come only from **git credential helper, environment, or OS keyring**. We never write a token, password, or cookie into `host.db`, pid.json, or our config files.
3. No “GitHub token” / “PAT” field in Settings.
4. GUI may confirm the push (explicit user action, ADR-0008). Failure “auth required” tells the user to fix git/gh login on the machine — we do not collect the secret.
5. If a later design needs a stored credential to function → **stop and escalate**.

`git.commit` / stage / unstage stay local and need no network creds.

## Consequences

- Stored-cred settings: **out-of-scope-by-ADR** → this file.
- `git.push` itself: **missing → E3 / Ф2**, implemented only under this rule.
