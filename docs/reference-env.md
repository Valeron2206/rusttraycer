# Traycer Desktop visual-parity reference environment

STAR 0126. This file pins the official Traycer Desktop Linux AppImage that RustTraycer v2.2.0 uses as the Visual Parity reference. It records only values that were measured on this machine. Updating the reference is only via parity-watch.

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

## Login wall (PO access needed)

The first painted screen is a mandatory **Sign in** wall. There is no email field on this frame — only the button. No credentials were invented. Workspace / home / empty-task chrome was not reached.

This is not an escalation. Product-owner access is required before any deeper official-app screens can be captured. Public pages fetched as a fallback (not a substitute for signed-in chrome):

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
| `/workspace/ref-traycer/home/` | Isolated official-app HOME |
| `/workspace/ref-traycer/logs/` | AppRun stdout/stderr from this session |
