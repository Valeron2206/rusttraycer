# Traycer Desktop visual-parity reference environment

STAR 0126 + 0128 + 0134. This file pins the official Traycer Desktop Linux AppImage that RustTraycer v2.2.0 uses as the Visual Parity reference. It records only values that were measured on this machine. Updating the reference is only via parity-watch.

## Pinned build

| Field | Measured value |
|---|---|
| GitHub `releases/latest` resolved tag | `desktop-v1.1.10` |
| Release name | Traycer Desktop 1.1.10 |
| Release id | `366210358` |
| Published (UTC) | 2026-08-06T12:53:36Z |
| Asset | `traycer-desktop-linux-x86_64.AppImage` |
| Asset size (bytes) | `184400695` |
| AppImage sha256 | `f9a4a5a97d510a96cab95d23ef11013100c0abf233a6f082e5767fa6c4097236` |
| GitHub asset digest | `sha256:f9a4a5a97d510a96cab95d23ef11013100c0abf233a6f082e5767fa6c4097236` (matches the file) |
| App self-report | updater: "Update for version 1.1.10 is not available (latest version: 1.1.10)"; crashpad `_version=1.1.10`; bundled CLI staged as `1.1.10` |

Pinned download URL (do not use `/releases/latest/download/...` after this pin):

```
https://github.com/traycerai/traycer/releases/download/desktop-v1.1.10/traycer-desktop-linux-x86_64.AppImage
```

The floating hint URL `https://github.com/traycerai/traycer/releases/latest/download/traycer-desktop-linux-x86_64.AppImage` currently redirects to that same tag. Re-resolve `releases/latest` only during a parity-watch pass.

The AppImage lives **outside** git, at `/workspace/ref-traycer/traycer-desktop-linux-x86_64.AppImage`. Do not commit the binary.

## Display

Prior dogfood used `DISPLAY=:5`. The same Xvfb was already running and was reused:

```
Xvfb :5 -screen 0 1280x800x24 -ac +extension GLX +render -noreset
```

`xdpyinfo` / `xwininfo -root` on `:5` measured **1280x800**, depth 24. The mapped Traycer client window was **1280x719** at `+0+24` (XFWM title bar above it). `rt-gui` was also on this display; the captured frames show the Traycer window in front.

Do not start a second Xvfb unless `:5` is gone. If you must start one, use `1280x800x24`.

## How to launch

FUSE device `/dev/fuse` exists on this box, but the first successful paint used `--appimage-extract` (allowed only so the AppImage can run without a FUSE mount). Extract once under `/workspace/ref-traycer/` (not in this worktree):

```bash
mkdir -p /workspace/ref-traycer
cd /workspace/ref-traycer
# download the pinned URL above, then:
sha256sum traycer-desktop-linux-x86_64.AppImage
# expect f9a4a5a97d510a96cab95d23ef11013100c0abf233a6f082e5767fa6c4097236
chmod +x traycer-desktop-linux-x86_64.AppImage
./traycer-desktop-linux-x86_64.AppImage --appimage-extract
```

Launch (measured flags that actually painted the welcome screen):

```bash
export APPIMAGE=/workspace/ref-traycer/traycer-desktop-linux-x86_64.AppImage
export APPDIR=/workspace/ref-traycer/squashfs-root
export HOME=/workspace/ref-traycer/home
export DISPLAY=:5
export LIBGL_ALWAYS_SOFTWARE=1
"$APPDIR/AppRun" --no-sandbox --disable-gpu --enable-software-rasterizer --disable-dev-shm-usage --in-process-gpu
```

Notes from this run:

- Electron `--no-sandbox` is required in this container.
- Without `APPIMAGE` / `APPDIR`, AppRun still opened a window titled `Traycer` but the renderer logged many `[app-protocol] fetch failed` / `net::ERR_FAILED` lines and the client area stayed black. Setting those two env vars made `app://renderer/` load (`[host-runtime] startup complete`, `hostCardinality: 'zero'`).
- GPU on Xvfb reported `gl=none,angle=none` / `skiaBackendType: None`. Software-rasterizer flags were required for a painted frame.
- Isolated `HOME=/workspace/ref-traycer/home` keeps official-app state out of the RustTraycer home. Do not write `~/.rusttraycer` for this reference.
- A later attempt to run the AppImage binary through FUSE was not used for the committed screenshots.

System libraries already present (Debian 13): `libnss3`, `libgtk-3-0t64`, `libasound2t64`, `libxkbcommon0`, `libgbm1`, `libxss1`, `libdrm2`. No extra runtime packages were required for AppRun itself. `x11-apps` (`xwd`) and `netpbm` (`xwdtopnm`) were installed only to grab frames.

## How screenshots are taken

Observe-only X11 dumps of the running window. No clicks, no typed credentials, no asar/asset extraction.

```bash
# window geometry
DISPLAY=:5 xwininfo -name Traycer
# client window (1280x719)
DISPLAY=:5 xwd -id <Traycer-window-id> -out /tmp/traycer-window.xwd
# full framebuffer (1280x800)
DISPLAY=:5 xwd -root -out /tmp/display5-root.xwd
xwdtopnm /tmp/traycer-window.xwd > /tmp/traycer-window.ppm
xwdtopnm /tmp/display5-root.xwd > /tmp/display5-root.ppm
# then encode PNG (PIL Image.save)
```

Committed frames (our files; taken 2026-08-21 09:37 YEKT / 04:37 UTC):

| Path | Pixels | Bytes | sha256 | What it shows |
|---|---|---|---|---|
| [docs/reference-screens/welcome-sign-in.png](reference-screens/welcome-sign-in.png) | 1280x719 | 494483 | `a553d300635a76bf5affd9ba2fbffda6518554b89dd5b22c344078088fbfc3c0` | Official window: File/Edit/View/Window/Help menu, white logo, heading "Welcome to Traycer", single **Sign in** button on a dark field. |
| [docs/reference-screens/display5-1280x800-desktop.png](reference-screens/display5-1280x800-desktop.png) | 1280x800 | 502395 | `f66bb1afa5bf64c4eef95e2252d0b7c54bda99b1c6bff89c20efd510c918f505` | Same moment, full `:5` desktop: XFWM title bar "Traycer", the welcome/sign-in client area, Plank dock (Chrome / Traycer / terminal). |

## Logged-in session (STAR 0128)

PO signed in (GitHub/Google device flow). Live logged-in capture is **DISPLAY=:3**, Electron HOME `/workspace/ref-traycer/home-d3`. Avatar initials **ZA**.

`:5` + HOME `/workspace/ref-traycer/home` is the older 0126 Welcome / Sign in session. Keep those frames. Do not use `:5` for logged-in chrome.

### Official host (process, not our RustTraycer host)

Do **not** touch RustTraycer host `:45927` (`hostId` `01a01b47-e863-71d3-bd2d-e885cf484d7a`).

Official host was started as a **foreground/background process**, not a systemd user service:

```bash
HOME=/workspace/ref-traycer/home-d3 \
  /workspace/ref-traycer/home-d3/.traycer/cli/bin/traycer host start --json
```

`traycer host start --help` describes this as the supervisor used by launchd/systemd. There is no "no-service" flag. `--no-bootstrap` skips start and was not used. `traycer host service install` was **not** run.

Measured after start (2026-08-21 09:57 YEKT / 04:57 UTC):

| Field | Value |
|---|---|
| Process running | yes (`traycer host status`: "Traycer host is running") |
| PID | `339885` |
| Version | `1.1.10` |
| WebSocket | `ws://127.0.0.1:43873/rpc` |
| Official hostId | `f9654c95-c528-489c-894c-a9bf379bf94b` |
| Data dir | `/workspace/ref-traycer/home-d3/.traycer/host` |
| systemd service | **not** installed (`E_SERVICE_INSTALL_FAILED` / `SYSTEMD_USER_UNREACHABLE`) |

`traycer host doctor` reports `SERVICE_NOT_REGISTERED` and `SYSTEMD_USER_UNREACHABLE`. Expected on this box. Do not install the OS service. Do not enable systemd. Do not click **Retry** in the GUI unless this process is already running.

After the process was up, the systemd modal on `:3` **dismissed itself**. The GUI painted the signed-in onboarding intro (ACT 05), then Settings (see frames below). Settings **is** in this branch.

### Logged-in frames (our files; 2026-08-21 YEKT)

| Path | Pixels | Bytes | sha256 | What it shows |
|---|---|---|---|---|
| [docs/reference-screens/logged-in-host-error.png](reference-screens/logged-in-host-error.png) | 1280x800 | 70892 | `f87f38728640242d2b5d302433bcd632069b69c38d36eda52824c08bf8fae8f6` | Signed-in white canvas, ZA avatar, official-host systemd modal (before process start). |
| [docs/reference-screens/logged-in-header-avatar.png](reference-screens/logged-in-header-avatar.png) | crop | 3801 | `ac06fe1fbabc8e860d7029e7ae8a47cb3c55e725c5ae2cf859fca1181de24b6d` | File/Edit/View/Window/Help + ZA avatar on the white signed-in header. |
| [docs/reference-screens/logged-in-onboarding-act05.png](reference-screens/logged-in-onboarding-act05.png) | 1280x800 | 625018 | `28dfb01bfb6e81a4b774ee5df9c7551e839fb987f1f2f7f3fac3d294e388f212` | After official host process: ACT 05 Delegation, "Tell Traycer how to choose", Agent selection guide modal, Skip intro. v1.1.10. |
| [docs/reference-screens/logged-in-onboarding-header.png](reference-screens/logged-in-onboarding-header.png) | 1280x90 | 37562 | `a8b537f5d33f53c7895945614536295f6d3f9fc7ddbab116db7827eec778db57` | Intro header: traycer wordmark + Skip intro / Esc. |

Settings was captured after restarting **only** the Desktop on `:3` (same HOME `home-d3`). Official host pid `339885` was left running. Retry was not clicked.

| Path | Pixels | Bytes | sha256 | What it shows |
|---|---|---|---|---|
| [docs/reference-screens/logged-in-settings.png](reference-screens/logged-in-settings.png) | 1280x800 | 115718 | `c35841e25690f1920c9c44f99a816599111e52e1db00a816aacd93cbf3142fe9` | Signed-in Settings / General: Start Page + Settings tabs, left nav (General/Host/Diagnostics), ZA avatar. systemd modal gone. |


## Login wall (0126, still on :5)

The first painted screen on `:5` is a mandatory **Sign in** wall. There is no email field on that frame — only the button. That session stayed signed out. Public pages fetched as a fallback (not a substitute for signed-in chrome):

- https://docs.traycer.ai/install — install table still points at `/releases/latest/download/...`; Linux AppImage note is `chmod +x` then run.
- https://docs.traycer.ai/changelog — Desktop changelog (sign-in is browser/PKCE, shared with the bundled CLI). No newer Desktop tag than `desktop-v1.1.10` was advertised by GitHub `releases/latest` at pin time.

Official-app log after paint: `[host-runtime] startup complete` with `hostCardinality: 'zero'` / `hasLocalHost: false`. Later `HOST_NOT_READY` appeared while the welcome/sign-in screen stayed up.

## Legal hygiene

- Launch and observe the official app on screen only.
- `--appimage-extract` is only so AppRun can start without a FUSE mount.
- Do **not** open `app.asar`, and do **not** extract or copy CSS, code, icons, fonts, or other brand assets from the squashfs tree.
- Screenshots of the running window are our files and may be committed.
- The AppImage binary stays out of git.

## Updating the reference is only via parity-watch

Do not bump this pin because a local download of `/releases/latest` moved. Architect parity-watch (cycle check of `desktop-v*` on GitHub plus https://docs.traycer.ai/changelog) is the only path that may retarget the tag, URL, sha256, and screenshots. If latest is still `desktop-v1.1.10`, leave this file and the PNGs alone.

## Out of tree (not committed)

| Path | Role |
|---|---|
| `/workspace/ref-traycer/traycer-desktop-linux-x86_64.AppImage` | Pinned binary |
| `/workspace/ref-traycer/squashfs-root/` | Extracted AppRun tree (runtime only) |
| `/workspace/ref-traycer/home/` | Isolated official-app HOME (`:5` welcome session) |
| `/workspace/ref-traycer/home-d3/` | Signed-in official-app HOME (`:3`) |
| `/workspace/ref-traycer/logs/` | AppRun / official-host stdout/stderr |

### Extra live crops (same :3 session)

| [docs/reference-screens/logged-in-settings-general.png](reference-screens/logged-in-settings-general.png) | crop/window | 89694 | `34c2d1e95d814449de6538947aab21da427e5603943a8ea670156cd4274cc736` | Current live :3 window: Settings / General (client 1280x719), ZA, tabs Start Page + Settings. |
| [docs/reference-screens/logged-in-header-tabs-avatar.png](reference-screens/logged-in-header-tabs-avatar.png) | crop/window | 9758 | `5dd3e7da8bed973407f42f8c5068d7824e330c98acfb5e3f1f2103fada09a4ce` | Live header: File menu, Start Page / Settings tabs, + tab, ZA avatar. |
| [docs/reference-screens/logged-in-settings-sidebar.png](reference-screens/logged-in-settings-sidebar.png) | crop/window | 16090 | `b91b224945588d576286a5a72180af97734ee248f761dd06b067dfb3a0cb8be5` | Live Settings sidebar: General (selected), Appearance, Providers, Notifications, Agent selection, Keybindings, Shell, Worktrees, Host, Diagnostics. |

Providers, Agent selection, Appearance, and Keybindings were captured (frames below). Start Page was not opened as its own page. Acts 01–03 were not on screen.

### Settings pages (File → Settings, 2026-08-21)

File → Settings opens the Settings tab (already live). It does **not** replay onboarding Acts 01–06. Skip / Start building were not clicked. Start Page tab click did not leave Settings.

Acts 01–03 (tasks/sidebar/agents/artifacts, layout/split/terminal, handoff/bubbles) were **not** on screen. Act 05 onboarding frame above remains the only intro-act capture.

| [docs/reference-screens/logged-in-act04-providers.png](reference-screens/logged-in-act04-providers.png) | 1280x800 | 116626 | `addfaba00087ef491223b078fa5775785e698279c86ca2563717dd1d4b944b50` | Settings → Providers (act04). Codex selected, bundled v0.146.0. |
| [docs/reference-screens/logged-in-act05-agent-selection.png](reference-screens/logged-in-act05-agent-selection.png) | 1280x800 | 103073 | `fce76224ab0ecc891a915cb0103e96a94bcfbb843b2203372ce07a11afe36fdb` | Settings → Agent selection (act05 settings page). Guide markdown saved. |
| [docs/reference-screens/logged-in-act06-appearance.png](reference-screens/logged-in-act06-appearance.png) | 1280x800 | 101328 | `9ee5e40a1bf1724019a110a6f69c2d82905bfcfbaacbfbaf92cfb6d80d234e4c` | Settings → Appearance (act06 theme). System + Traycer Green, Figtree 15px. |
| [docs/reference-screens/logged-in-act06-keybindings.png](reference-screens/logged-in-act06-keybindings.png) | 1280x800 | 95567 | `afdc4f1361744b6958444aac085694dfe8a2ec697d424bd65bc14f7604c646a0` | Settings → Keybindings (act06 shortcuts). Ctrl+1–9 / Ctrl+N / tabs. Cmd+K not in the first screenful. |


## STAR 0134 — official missing surfaces (Start Page, History)

Same live signed-in session as 0128: **DISPLAY=:3**, HOME `/workspace/ref-traycer/home-d3`, avatar **ZA**, official Desktop 1.1.10, client `1280x719+0+24`. Official host process **339885** (`ws://127.0.0.1:43873/rpc`, hostId `f9654c95-c528-489c-894c-a9bf379bf94b`) was left running. Restart host / Retry / Skip intro / Start building were **not** clicked. `:5` Welcome session was not used.

0128 left the GUI on Settings and recorded that a Start Page tab click did not leave Settings. 0134 retried a **precise click on the Start Page tab label** (not File → Settings, not the header gear) at framebuffer `(240, 70)`. That click activated the Start Page *page*. Capture method: `ffmpeg -f x11grab -video_size 1280x800 -i :3 -frames:v 1` plus `/tmp/xlenv/click_xy.py` on DISPLAY=:3.

Settings / welcome / host-error frames already in this file are **not** the missing surfaces.

### Captured (2026-08-21 ~11:00 YEKT / 06:00 UTC)

| Path | Pixels | Bytes | sha256 | What it shows |
|---|---|---|---|---|
| [docs/reference-screens/logged-in-start-page.png](reference-screens/logged-in-start-page.png) | 1280x800 | 74717 | `d35051611987331e39e60693da8d3a267e07baf5cab7c97d262d47ec14113f7f` | Official Start Page *page* (Start Page tab active). Greeting "Good morning" / "What's on your mind?"; composer "Ask Traycer anything. @ mention for context"; GPT-5.6-Sol / High / Full access; Switch to Terminal; cursor + Add folder; Most recent / Filter / Select (empty list). Banner "Update installed — restart host to finish." Settings tab still present, inactive. ZA. |
| [docs/reference-screens/logged-in-history.png](reference-screens/logged-in-history.png) | 1280x800 | 80080 | `3ddb69b8ee052503fc1de56e5bcd161d244a70211399e2f58e0f866b4aa0aa07` | History modal after clicking the header **clock** (left of bell + ZA). Title History; search "Search by title, repo, branch, or PR"; Most recent / Filter / Select / Refresh; empty state **"No tasks yet"**. Pop-out + close. |

### Still needs live (honest — surfaces did not open)

- **Task conversation / chat thread** (`logged-in-task.png` / `logged-in-chat.png`): not captured. History is empty ("No tasks yet"). The Start Page composer is the chat *input* on Start Page, not a separate task/chat frame. Ctrl+N (Keybindings: New task) opened another Start Page tab ("Where shall we start?"), not a task thread. `+` tab stayed on Start Page.
- **Ladder / Epic** (`logged-in-ladder.png`): not captured. File → **Open Epic in New Window…** opened a picker titled "Open Epic in New Window" with **"No Epics yet."** Close only. No epic/task to open. That empty picker was **not** committed as a ladder frame.
- **Onboarding Acts 01–03**: still gone. File → Settings does not replay them. View menu is Reload / Force Reload / zoom / Full Screen only (no Home / Start / History). File menu is New Window, Open Epic in New Window…, Close Tab, Settings…, Sign Out, Quit.

Do not treat Settings, welcome/Sign in, or the host-error/systemd modal as substitutes for the missing task/chat/ladder frames.
