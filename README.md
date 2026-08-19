# RustTraycer

Pure-Rust desktop analog of [Traycer](https://traycer.ai): a local **host** daemon, a thin **egui** GUI, and a **CLI** for process lifecycle.

Loop: start host → add a folder → create a Task → create an agent → chat → transcript survives GUI (and host) restart.

**v2.0.0 supported package:** Linux x86_64 (AppImage + `.deb` + tarball). **macOS aarch64** is build-from-source (CI compile). Windows is not in v2.0.

Out of scope (stubs only, no impl): PTY, terminal mux, agent-to-agent, cloud sync.

## Install from GitHub Release

Release assets are a Linux x86_64 `tar.gz`, AppImage, `.deb`, plus `SHA256SUMS` (tag `v1.0.0` → filename with `v`). After the tag is published:

```bash
# download rusttraycer-v1.0.0-linux-x86_64.tar.gz and SHA256SUMS from the GitHub Release, then:
sha256sum -c SHA256SUMS
tar -xzf rusttraycer-v1.0.0-linux-x86_64.tar.gz
sudo install -m 0755 rusttraycer-v1.0.0-linux-x86_64/rt-host rusttraycer-v1.0.0-linux-x86_64/rt-cli rusttraycer-v1.0.0-linux-x86_64/rt-gui /usr/local/bin/
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

`rt-cli` commands: **start**, **stop**, **doctor**, **status**, **logs**, **reset-db** (`reset-db` needs `--yes`; refuses if the host is running).

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
| `rt-protocol` | Wire types, RPC, handshake `{major,minor}` |
| `rt-host` | Daemon: HTTP/WS on `127.0.0.1`, supervisor, git/worktree |
| `rt-storage` | `host.db` (rusqlite, migrations) |
| `rt-runtime` | Adapters: `cli.generic`, `cli.claude`, `cli.codex` |
| `rt-cli` | `start` / `stop` / `doctor` / `status` / `logs` / `reset-db` |
| `rt-gui` | eframe + egui: tasks, harness picker, N agents, ask-default ladder, git panel, Stop |

## Specs

Canonical notes for v1.0 vs older drafts: [`docs/v1-delta.md`](docs/v1-delta.md).

| Doc | What it is |
|---|---|
| [directive-v1.md](docs/directive-v1.md) | Release goals / DoD |
| [adr/0001-target-platforms.md](docs/adr/0001-target-platforms.md) | Linux x86_64 only |
| [adr/0002-agent-cancel.md](docs/adr/0002-agent-cancel.md) | Cancel is in v1.0 |
| [agent-cancel-v0.md](docs/agent-cancel-v0.md) | Cancel RPC/WS contract |
| [git-files-v1.md](docs/git-files-v1.md) | files RO, git.status/diff, worktree |
| [protocol-v0.md](docs/protocol-v0.md) | Wire envelope (see v1-delta for added methods) |
| [architecture-v0.md](docs/architecture-v0.md) | Original crate map (MVP-era; see v1-delta) |

## License

See the repository license file if present; otherwise treat as the project's declared license on GitHub.
