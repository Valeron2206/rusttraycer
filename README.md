# RustTraycer

Pure-Rust desktop analog of [Traycer](https://traycer.ai): a local **host** daemon, a thin **egui** GUI, and a **CLI** for process lifecycle.

Loop: start host → add a folder → create a Task → create an agent → chat → transcript survives GUI (and host) restart.

**v2.1.1 supported package:** Linux x86_64 (AppImage + `.deb` + tarball). **macOS aarch64** is build-from-source (CI compile). Windows is out of scope (ADR-0006).

Protocol **1.9**. Storage migrations **0001–0010**.

Shipped (parity matrix): host + GUI + CLI; permission ladder (ask default); write/git without secrets in `host.db`; Agent Terminal + Shell + mux (including terminals without a Task, workspace required); artifacts (Markdown + PDF); A2A + loops; search; multi-account labels; mid-turn steer; self-hosted `rt-sync`; PR view; prompt stash; resource monitor / hooks / drag-to-tile; worktree cleanup; nested `AGENTS.md`; user presets; `logs --follow`.

**Out of scope (ADR, not stubs):** C26 full-access default; C66–C75 named extra harnesses as required, own inference, telemetry, managed cloud, CRDT, extension Phase/Epic/YOLO, Desktop Epic Mode, Windows/WSL package, secrets in `host.db`, sharing/SSO.

Not a matrix gap (not oos-by-ADR): Intel Mac, signed/notarized macOS, `.rpm`, disable-`AGENTS.md` toggle, `cli.generic` steer.

## Install from GitHub Release

Release assets are a Linux x86_64 `tar.gz`, AppImage, `.deb`, plus `SHA256SUMS` (tag `v2.1.1` → filename with `v`). After the tag is published:

```bash
# download rusttraycer-v2.1.1-linux-x86_64.tar.gz and SHA256SUMS from the GitHub Release, then:
sha256sum -c SHA256SUMS
tar -xzf rusttraycer-v2.1.1-linux-x86_64.tar.gz
sudo install -m 0755 rusttraycer-v2.1.1-linux-x86_64/rt-host rusttraycer-v2.1.1-linux-x86_64/rt-cli rusttraycer-v2.1.1-linux-x86_64/rt-gui /usr/local/bin/
```

Binaries: `rt-host`, `rt-cli`, `rt-gui`. The GUI never starts the host.

## Build from source

Needs [Rust 1.85](https://www.rust-lang.org/tools/install) (see `rust-toolchain.toml`). Linux x86_64.

```bash
cargo build --workspace --release
```

## Run

Data dir: `$RUSTTRAYCER_HOME` or `~/.rusttraycer` (`host/pid.json`, `host/host.db`, `host/host.log`).

```bash
# optional providers (doctor reports available=false if unset)
export RUSTTRAYCER_GENERIC_CMD=/path/to/agent    # stdin: {"messages":[...]} + EOF; stdout: text
# export RUSTTRAYCER_GENERIC_ARGS='["--flag"]'   # optional JSON array
export RUSTTRAYCER_CLAUDE_CMD=claude             # or a mock binary
export RUSTTRAYCER_CODEX_CMD=codex

# start host (execs rt-host; writes pid.json). GUI does not spawn it.
rt-cli start          # after cargo: ./target/release/rt-cli start

# other terminal
rt-gui                # ./target/release/rt-gui

rt-cli doctor         # JSON: paths, pid alive, host.doctor if reachable
rt-cli stop           # SIGTERM, idempotent
```

`rt-cli` commands: **start**, **stop**, **doctor**, **status**, **logs** (`--follow`), **reset-db** (`reset-db` needs `--yes`; refuses if the host is running), **sync** (push/pull; secret via env only).

### Generic agent mock (README cycle)

```bash
cat > /tmp/rt-echo.sh << 'EOF'
#!/bin/sh
python3 -c 'import json,sys; m=json.load(sys.stdin)["messages"];
print(next(x["content"] for x in reversed(m) if x["role"]=="user"))'
EOF
chmod +x /tmp/rt-echo.sh
export RUSTTRAYCER_GENERIC_CMD=/tmp/rt-echo.sh
rt-cli start
```

Then in the GUI: add a folder → Task → pick a harness from `host.doctor` (generic/claude/codex) → send. N agents per Task. New agents default to **ask** (not Traycer full-access). Transcript is in `host.db` after restart.

## Layout

| Crate | Role |
|---|---|
| `rt-protocol` | Wire types, RPC, handshake `{major,minor}` (1.0–1.9) |
| `rt-host` | Daemon: HTTP/WS on `127.0.0.1`, supervisor, git/worktree, PTY/mux, artifacts, A2A, search, steer, accounts, stash, `rt-sync`, `pr.get` |
| `rt-storage` | `host.db` (rusqlite, migrations 0001–0010) |
| `rt-runtime` | Adapters: `cli.generic`, `cli.claude`, `cli.codex` |
| `rt-cli` | `start` / `stop` / `doctor` / `status` / `logs --follow` / `reset-db` / `sync` |
| `rt-gui` | eframe + egui: tasks, harness picker, N agents, ladder, search, PR view, stash, steer, sync URL, user presets, git panel, Stop |

## Specs

What shipped vs drafts: [`docs/v2-delta.md`](docs/v2-delta.md). v2.1 close: [`docs/v21-complete-v2.md`](docs/v21-complete-v2.md). Historical 1.0 note: [`docs/v1-delta.md`](docs/v1-delta.md).

| Doc | What it is |
|---|---|
| [directive-v2.md](docs/directive-v2.md) | Release goals / DoD |
| [parity-matrix.md](docs/parity-matrix.md) | Traycer Desktop → RustTraycer statuses |
| [adr/0001-target-platforms.md](docs/adr/0001-target-platforms.md) | Linux x86_64 package; macOS aarch64 = source/CI |
| [adr/0002-agent-cancel.md](docs/adr/0002-agent-cancel.md) | Cancel contract |
| [adr/0003-sync-approach.md](docs/adr/0003-sync-approach.md) | Export/import min; `rt-sync` must in v2.1; no managed cloud |
| [adr/0005-git-push-no-secrets.md](docs/adr/0005-git-push-no-secrets.md) | No tokens in `host.db` |
| [adr/0006-platforms-v2.md](docs/adr/0006-platforms-v2.md) | AppImage + `.deb`; Windows oos |
| [adr/0008-no-telemetry.md](docs/adr/0008-no-telemetry.md) | No vendor telemetry |
| [protocol-v0.md](docs/protocol-v0.md) | Wire envelope (methods live at 1.1–1.9) |
| [architecture-v0.md](docs/architecture-v0.md) | Crate map (see v2-delta) |

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option. Copyright (c) 2026 Valeriy Khalikov.
