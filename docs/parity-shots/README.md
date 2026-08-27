# Parity shots — ours after chrome plate 0138

Observe-only recapture of **our** `rt-gui` after STAR 0138 header plate (white `BG_HEADER` + hairline, online pill removed). Not official Traycer. No `asar`.

## Build / session

| Field | Value |
|---|---|
| Branch | `task/0139-v3-ours-shots-plate` |
| Worktree | `/workspace/wt-0139-v3-ours-shots-plate` |
| Base | `9de19f3bab3e7c1b45a2926ade5b8ab108d082da` (merge STAR 0138; contains `4b14af4`) |
| Plate commit | `4b14af4` `feat(gui): chrome header plate and drop online pill` |
| Binary | `cargo build -p rt-gui --release` in this worktree → `/workspace/wt-0139-v3-ours-shots-plate/target/release/rt-gui` |
| Display | Xvfb `:11` 1280×800×24 (not `:10` 0135 gui, not `:3` official) |
| Window | `RustTraycer` 1280×800+0+0 |
| `RUSTTRAYCER_HOME` | `/workspace/df-0102-home` |
| hostId | `01a01b47-e863-71d3-bd2d-e885cf484d7a` |
| host pid | **35281** (live OUR host; not restarted) |
| rpcUrl | `http://127.0.0.1:43811` |
| Capture | 2026-08-27 21:36–21:40 YEKT (16:36–16:40 UTC). `ffmpeg -f x11grab` from `:11`. |
| Policy | Did not rebase. Did not merge/push. Did not start or kill official host 339885. Did not reuse `:10` 0135 gui. No asar. |

## Pairing

Official frames live under `docs/reference-screens/`. Pair only where the surface honestly matches. Official Start Page / History from STAR 0134 live on `88633cd`, **not** on this `9de19f3` base.

| Ours | Pixels | Official pair |
|---|---|---|
| [`ours/chrome.png`](ours/chrome.png) | 1280×40 | [`logged-in-header-tabs-avatar.png`](../reference-screens/logged-in-header-tabs-avatar.png) **HT** — Start Page / Settings / `+` / avatar strip. Ours is the 40 px white plate (0138); official is 88 px (menu + tabs). Official has no «онлайн» pill either; ours dropped the pill in 0138 (`4b14af4`). |
| [`ours/tasks.png`](ours/tasks.png) | 1280×800 | no official pair on this base |
| [`ours/empty.png`](ours/empty.png) | 1280×800 | no official pair |
| [`ours/canvas.png`](ours/canvas.png) | 1280×800 | no official pair |
| [`ours/chat.png`](ours/chat.png) | 400×680 | no official pair |
| [`ours/host.png`](ours/host.png) | 1280×800 | **not** [`logged-in-host-error.png`](../reference-screens/logged-in-host-error.png). Host online. Host diagnostics + YOLO banner. |

## What each ours frame shows

- **chrome** — 40 px white plate (`#FFFFFF` fill, `#DFE9E7` hairline): back/forward, **Start Page** tab, **Settings** tab, **+**, utility cluster, **RT** avatar. **No «онлайн» pill** (removed 0138). Mouse parked off the bar.
- **tasks** — open-task list (STAR 0135/0134/0131/0128 rows), filter Открытые / Архив, Новая задача. Same white-plate chrome, no pill. No YOLO banner here (no selected-agent policy on Start Page).
- **empty** — Архив filter: card «нет архивных». Same chrome plate.
- **canvas** — task `STAR 0128 L187 exact wording`, agents + canvas/chat split, YOLO banner «Yolo — лестница разрешений не вызывается.» Chrome plate, no pill.
- **chat** — right-pane crop 400×680 at (880, 80): вид Канвас, плитка `cli.generic`, Чат tab, empty transcript, composer «Написать сообщение…».
- **host** — Host diagnostics from `pid.json`: **status онлайн** (0138 moved the label off chrome into this page), hostId / pid **35281** / `:43811` / `startedAt` 2026-08-27T16:13:51.386Z. YOLO banner still visible (selected-task policy), not a host-down banner. Settings tab selected.

## Not captured

- **Host-error / reconnect banner** — `/health` 200. Status «онлайн» is a Host-page field, not a chrome pill.
- **Populated chat** — transcript empty.
- **Official Settings acts** — our Settings tab opens Host diagnostics, not Appearance/Providers/Keybindings.
