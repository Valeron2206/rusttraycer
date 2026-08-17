# RustTraycer

Pure-Rust desktop analog of [Traycer](https://traycer.ai): local host + thin GUI for Tasks and coding agents.

MVP loop: start host → add a folder → create a Task → create an agent → chat → transcript survives GUI restart.

## Run

```bash
cargo build -p rt-host -p rt-cli -p rt-gui
export RUSTTRAYCER_GENERIC_CMD=/path/to/your-agent   # any CLI that reads {"messages":[...]} on stdin
./target/debug/rt-cli start
./target/debug/rt-gui
```

Stop the host with `rt-cli stop`. GUI never spawns the host.

## Layout

| Crate | Role |
|---|---|
| `rt-protocol` | Wire types / RPC |
| `rt-host` | Local daemon (HTTP/WS, SQLite) |
| `rt-storage` | `host.db` |
| `rt-runtime` | `cli.generic` adapter |
| `rt-cli` | start / stop / doctor |
| `rt-gui` | eframe + egui client |

Specs live in [`docs/`](docs/).
