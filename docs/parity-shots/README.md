# Parity shots — ours after theme 0130

Observe-only captures of **our** `rt-gui` (STAR 0130 tokens) at 1280×800. Not official Traycer. No `asar`. Host `:45927` was not restarted.

## Build / session

| Field | Value |
|---|---|
| Branch | `task/0131-v3-ours-parity-shots` |
| Worktree | `/workspace/wt-0131-v3-ours-parity-shots` |
| Base / `origin/main` | `5da22f96dc85b9c1a52e0828c42c39d5b717d44c` (merge STAR 0130) |
| Theme commit | `a605806` `feat(gui): apply design-parity theme tokens` |
| Binary | `cargo build -p rt-gui --release` in this worktree (`CARGO_TARGET_DIR=/workspace/wt-phase0/target`) |
| Display | Xvfb `:8` 1280×800×24 (fresh; not `:3` official logged-in, not `:5` welcome, not `:6` box desktop) |
| Window | `RustTraycer` 1280×800+0+0 (root dump == client window) |
| `RUSTTRAYCER_HOME` | `/workspace/df-0102-home` |
| hostId | `01a01b47-e863-71d3-bd2d-e885cf484d7a` |
| host pid | **229448** (`rt-host` already live; GUI reconnect via `pid.json` only) |
| rpcUrl | `http://127.0.0.1:45927` |
| Capture | 2026-08-21 10:39–10:42 YEKT (05:39–05:42 UTC). `ffmpeg -f x11grab` (`import`/`scrot` not installed). |
| Policy | Did not restart `:45927`. Did not click Retry/Start. Did not touch official AppImage / asar / `/workspace/ref-traycer`. |

## Pairing

Official frames live under `docs/reference-screens/`. Pair only where the surface honestly matches.

| Ours | Pixels | Official pair |
|---|---|---|
| [`ours/chrome.png`](ours/chrome.png) | 1280×40 | [`logged-in-header-avatar.png`](../reference-screens/logged-in-header-avatar.png), [`logged-in-header-tabs-avatar.png`](../reference-screens/logged-in-header-tabs-avatar.png) — header strip only. [`logged-in-settings.png`](../reference-screens/logged-in-settings.png) is official Settings desktop; not an honest pair for this 40 px rusttraycer chrome crop. |
| [`ours/tasks.png`](ours/tasks.png) | 1280×800 | no official pair (Acts 01–03 / Start Page / History were never captured live) |
| [`ours/empty.png`](ours/empty.png) | 1280×800 | no official pair |
| [`ours/canvas.png`](ours/canvas.png) | 1280×800 | no official pair |
| [`ours/chat.png`](ours/chat.png) | 400×680 | no official pair (chat transcript never captured on official) |
| [`ours/host.png`](ours/host.png) | 1280×800 | **not** [`logged-in-host-error.png`](../reference-screens/logged-in-host-error.png). Host was online. This is Host diagnostics + YOLO policy banner, not a host-error/reconnect modal. |

## What each ours frame shows

- **chrome** — top 40 px nav: wordmark, Задачи / open-task tab / Host, search, metrics chip, green **онлайн**.
- **tasks** — open-task list (STAR 0128 rows), filter Открытые / Архив, Новая задача. Connected chrome, no offline banner.
- **empty** — Архив filter: card «нет архивных».
- **canvas** — task `STAR 0128 L187 exact wording`, agents column, canvas/chat split, YOLO banner «Yolo — лестница разрешений не вызывается.»
- **chat** — right-pane crop: вид Канвас, плитка `cli.generic`, Чат tab, empty transcript («Нет сообщений…»), composer «Написать сообщение…».
- **host** — Host diagnostics from `pid.json`: hostId / pid **229448** / `:45927` / `startedAt` 2026-08-20T03:45:00.849Z. YOLO banner still visible (policy), not a host-down banner.

## Not captured

- **Host-error / reconnect banner** — host `/health` 200, GUI **онлайн**. Did not click Повторить / Start / anything that would spawn a second host. Official `logged-in-host-error.png` has no ours pair in this set.
- **Populated chat** — this task’s transcript is empty on host.
- **Official Settings / Appearance / Providers / Keybindings pages** — our GUI has Host diagnostics, not those official Settings acts.
