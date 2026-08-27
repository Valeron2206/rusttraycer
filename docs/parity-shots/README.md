# Parity shots — ours after header seam 0141

Observe-only recapture of **our** `rt-gui` after STAR 0141 header seam (`#DFE9E7` under the 40 px tab strip, not a Frame stroke at y=0). White plate and no online pill are unchanged from 0138. Not official Traycer. No `asar`.

## Build / session

| Field | Value |
|---|---|
| Branch | `task/0142-v3-ours-shots-seam` |
| Worktree | `/workspace/wt-0142-v3-ours-shots-seam` |
| Base | `6fb741a8305caef658b39f87b7db7ca241b39d06` (merge STAR 0141; contains `3698378`) |
| Seam commit | `3698378` `feat(gui): draw header seam under the tab strip` |
| Binary | `cargo build -p rt-gui --release` in this worktree → `/workspace/wt-0142-v3-ours-shots-seam/target/release/rt-gui` |
| Display | Xvfb `:12` 1280×800×24 (not `:10` 0135 gui, not `:11` 0139 gui, not `:3` official) |
| Window | `RustTraycer` 1280×800+0+0 |
| `RUSTTRAYCER_HOME` | `/workspace/df-0102-home` |
| hostId | `01a01b47-e863-71d3-bd2d-e885cf484d7a` |
| host pid | **35281** (live OUR host; not restarted) |
| rpcUrl | `http://127.0.0.1:43811` |
| Capture | 2026-08-27 22:03–22:08 YEKT (17:03–17:08 UTC). `ffmpeg -f x11grab` from `:12`. |
| Policy | Did not rebase. Did not merge/push. Did not start or kill official host 339885. Did not reuse `:10` 0135 or `:11` 0139 gui. No asar. |

## Pairing

Official frames live under `docs/reference-screens/`. Pair only where the surface honestly matches. Official Start Page / History from STAR 0134 live on `88633cd`, **not** on this `6fb741a` base.

| Ours | Pixels | Official pair |
|---|---|---|
| [`ours/chrome.png`](ours/chrome.png) | 1280×40 | [`logged-in-header-tabs-avatar.png`](../reference-screens/logged-in-header-tabs-avatar.png) **HT** — Start Page / Settings / `+` / avatar strip. Ours is the 40 px white plate; official is 88 px (menu + tabs). Seam under the strip is **0141** (`3698378`): `#DFE9E7` on the last pixel of the crop (y=39), not the y=0 window join that 0138's Frame stroke painted. Official has no «онлайн» pill either; ours dropped the pill in 0138 (`4b14af4`). |
| [`ours/tasks.png`](ours/tasks.png) | 1280×800 | no official pair on this base |
| [`ours/empty.png`](ours/empty.png) | 1280×800 | no official pair |
| [`ours/canvas.png`](ours/canvas.png) | 1280×800 | no official pair |
| [`ours/chat.png`](ours/chat.png) | 400×680 | no official pair |
| [`ours/host.png`](ours/host.png) | 1280×800 | **not** [`logged-in-host-error.png`](../reference-screens/logged-in-host-error.png). Host online. Host diagnostics + YOLO banner. |

## What each ours frame shows

- **chrome** — 40 px white plate (`#FFFFFF` fill). `#DFE9E7` hairline is the **bottom** row of the crop (y=39), under the tab strip (0141). y=0 is white, not the seam. Back/forward, **Start Page** tab, **Settings** tab, **+**, utility cluster, **RT** avatar. **No «онлайн» pill** (removed 0138). Mouse parked off the bar.
- **tasks** — open-task list (STAR 0139/0135/0134/0131/0128 rows), filter Открытые / Архив, Новая задача. Same white-plate chrome with under-strip seam, no pill. No YOLO banner here (no selected-agent policy on Start Page).
- **empty** — Архив filter: card «нет архивных». Same chrome plate + under-strip seam.
- **canvas** — task `STAR 0128 L187 exact wording`, agents + canvas/chat split, YOLO banner «Yolo — лестница разрешений не вызывается.» Chrome plate, seam under strip, no pill.
- **chat** — right-pane crop 400×680 at (880, 80): вид Канвас, плитка `cli.generic`, Чат tab, empty transcript, composer «Написать сообщение…».
- **host** — Host diagnostics from `pid.json`: **status онлайн** (0138 moved the label off chrome into this page), hostId / pid **35281** / `:43811` / `startedAt` 2026-08-27T16:13:51.386Z. YOLO banner still visible (selected-task policy), not a host-down banner. Settings tab selected. Same under-strip seam.

## Pixel check (chrome.png)

Sampled after recapture, before commit:

| y | `#DFE9E7` count | `#FFFFFF` count |
|---|---|---|
| 0 | 0 | 1258 |
| 39 | 1280 | 0 |

0139/0138 plate crop had the hairline at y=0 (Frame stroke). This recapture refuses that: seam is only at the bottom of the 40 px strip.

## Not captured

- **Host-error / reconnect banner** — `/health` 200. Status «онлайн» is a Host-page field, not a chrome pill.
- **Populated chat** — transcript empty.
- **Official Settings acts** — our Settings tab opens Host diagnostics, not Appearance/Providers/Keybindings.
