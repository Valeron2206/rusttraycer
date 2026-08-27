# Parity shots — ours after chrome IA 0133

Observe-only captures of **our** `rt-gui` after STAR 0133 chrome IA (tab strip Start Page / Settings / `+` / avatar). Not official Traycer. No `asar`.

## Build / session

| Field | Value |
|---|---|
| Branch | `task/0135-v3-ours-shots-chrome` |
| Worktree | `/workspace/wt-0135-v3-ours-shots-chrome` |
| Base | `8ce54bba3ea18c309bf7566618fd549fdcd0d656` (merge STAR 0133; not rebased onto `88633cd` / `48b873a`) |
| Chrome IA | `87254b4` `feat(gui): chrome IA tab strip matching design-parity HT` |
| Binary | `cargo build -p rt-gui --release` in this worktree |
| Display | Xvfb `:10` 1280×800×24 (not `:3` official) |
| Window | `RustTraycer` 1280×800+0+0 |
| `RUSTTRAYCER_HOME` | `/workspace/df-0102-home` |
| hostId | `01a01b47-e863-71d3-bd2d-e885cf484d7a` |
| host pid | **28385** (`rt-host` started 2026-08-27 after stale 229448 died; GUI reconnect via `pid.json`) |
| rpcUrl | `http://127.0.0.1:43549` (binary binds `127.0.0.1:0`; `:45927` was down) |
| Capture | 2026-08-27 21:09–21:12 YEKT (16:09–16:12 UTC). `ffmpeg -f x11grab`. |
| Policy | Did not rebase. Did not merge/push. Did not start or kill official host 339885. No asar. |

## Pairing

Official frames live under `docs/reference-screens/`. Pair only where the surface honestly matches. Official Start Page / History from STAR 0134 live on `88633cd`, **not** on this `8ce54bb` base.

| Ours | Pixels | Official pair |
|---|---|---|
| [`ours/chrome.png`](ours/chrome.png) | 1280×40 | [`logged-in-header-tabs-avatar.png`](../reference-screens/logged-in-header-tabs-avatar.png) **HT** — Start Page / Settings / `+` / avatar strip. Crop heights differ (40 vs 88). |
| [`ours/tasks.png`](ours/tasks.png) | 1280×800 | no official pair on this base |
| [`ours/empty.png`](ours/empty.png) | 1280×800 | no official pair |
| [`ours/canvas.png`](ours/canvas.png) | 1280×800 | no official pair |
| [`ours/chat.png`](ours/chat.png) | 400×680 | no official pair |
| [`ours/host.png`](ours/host.png) | 1280×800 | **not** [`logged-in-host-error.png`](../reference-screens/logged-in-host-error.png). Host online. Host diagnostics + YOLO banner. |

## What each ours frame shows

- **chrome** — 40 px nav: back/forward, **Start Page** tab, **Settings** tab, **+**, utility cluster, green **онлайн**, **RT** avatar.
- **tasks** — open-task list (STAR 0128/0131/0134 rows), filter Открытые / Архив, Новая задача. Chrome IA visible.
- **empty** — Архив filter: card «нет архивных».
- **canvas** — task `STAR 0128 L187 exact wording`, agents + canvas/chat split, YOLO banner.
- **chat** — right-pane crop: вид Канвас, плитка `cli.generic`, Чат tab, empty transcript, composer «Написать сообщение…».
- **host** — Host diagnostics from `pid.json`: hostId / pid **28385** / `:43549` / `startedAt` 2026-08-27T16:06:39.938Z.

## Not captured

- **Host-error / reconnect banner** — `/health` 200, GUI **онлайн**.
- **Populated chat** — transcript empty.
- **Official Settings acts** — our Settings tab opens Host diagnostics, not Appearance/Providers/Keybindings.
