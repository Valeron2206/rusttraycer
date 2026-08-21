# Design-parity report — official ↔ ours (STAR 0132)

Observe-only. Docs only. No asar. Host `:45927` not touched. Base `c6a6a22` (merge STAR 0131).

This report pairs **live official frames** in `docs/reference-screens/` with **our** `rt-gui` frames in `docs/parity-shots/ours/` (theme 0130 / shots 0131). Criterion: composition, palette, typography, density. A delta is **accepted** only if it is already on the list in [`design-parity-v1.md` §4](design-parity-v1.md#4-accepted-deviations-not-bugs). Anything else is an **open miss**, not a new accepted deviation.

## 0. Resolution class

Both sets were taken on an Xvfb-class 1280-wide framebuffer (official `:5` / `:3`; ours `:8`, 1280×800×24). Crops keep their own height — do not mix coordinate systems (`design-parity-v1.md` §1.1).

| Set | Typical frame | Notes |
|---|---|---|
| Official full desktop | 1280×800 | XFWM 24 px + client + Plank on some frames |
| Official client | 1280×719 | **W**, **SG** |
| Official header crops | 1280×100 **HA**, 1280×88 **HT**, 1280×90 **OH** | Product chrome + GTK menu |
| Ours window | 1280×800 | eframe client; `ours/chrome.png` is a 1280×40 product-nav crop |
| Ours chat crop | 400×680 | Right pane only |

Same **class** = 1280-wide Xvfb. Same **crop** is required before scoring a pair. A 1280×40 nav strip is not the same crop as a 1280×88 official header (menu + tabs + avatar).

## 1. Honest pairs

Pair only where the **surface** matches. Official Settings / onboarding / welcome / host-error are not our Tasks / Canvas / Host diagnostics.

| Ours | Pixels | Official | Pixels | Pair? |
|---|---|---|---|---|
| [`ours/chrome.png`](parity-shots/ours/chrome.png) | 1280×40 | [`logged-in-header-avatar.png`](reference-screens/logged-in-header-avatar.png) **HA** | 1280×100 | **strip only** — both are the top signed-in bar, different crop height |
| [`ours/chrome.png`](parity-shots/ours/chrome.png) | 1280×40 | [`logged-in-header-tabs-avatar.png`](reference-screens/logged-in-header-tabs-avatar.png) **HT** | 1280×88 | **strip only** — same caveat |
| [`ours/tasks.png`](parity-shots/ours/tasks.png) | 1280×800 | — | — | **no pair** (Acts 01–03 / Start Page / History never captured live) |
| [`ours/empty.png`](parity-shots/ours/empty.png) | 1280×800 | — | — | **no pair** |
| [`ours/canvas.png`](parity-shots/ours/canvas.png) | 1280×800 | — | — | **no pair** |
| [`ours/chat.png`](parity-shots/ours/chat.png) | 400×680 | — | — | **no pair** (official chat transcript never captured) |
| [`ours/host.png`](parity-shots/ours/host.png) | 1280×800 | [`logged-in-host-error.png`](reference-screens/logged-in-host-error.png) **HE** | 1280×800 | **not a pair** — same pixel class, different surface (ours = Host diagnostics + YOLO banner, host online; **HE** = signed-in reconnect modal) |
| — | — | **W** / **D** welcome | 1280×719 / 1280×800 | **no ours frame** |
| — | — | **S** / **SG** / **SS** / **P** / **AG** / **A** / **K** Settings acts | mix | **no ours frame** (our GUI has Host diagnostics, not those pages) |
| — | — | **OA** / **OH** onboarding | 1280×800 / 1280×90 | **no ours frame** |

`docs/parity-shots/README.md` already states the same pairing. This report does not invent extra pairs.

## 2. Scored pair — chrome ↔ **HA** / **HT**

Compare structure, tokens, and rhythm — not a 1:1 overlay (`§4.1`).

### Composition

Official **HT**: GTK File/Edit/View/Window/Help; back/forward + layers; **Start Page** inactive tab; **Settings** active white tab + `+`; utility cluster (gauge, overflow, gear, history, bell); **ZA** avatar disc.

Ours `chrome.png`: Lucide layers + **RustTraycer** wordmark; **Задачи** / open-task title / **Host**; search field; metrics chip; teal **онлайн** pill. No File menu, no Start Page / Settings tabs, no `+`, no avatar.

| Delta | In §4? |
|---|---|
| eframe / no XFWM + no GTK File menu | **yes — §4.5** Native window chrome |
| 40 px product nav (spec target tab strip is 37 px) | **yes — §4.5** (`rt-gui` keeps its own 40 px nav) |
| RustTraycer wordmark + Lucide instead of Traycer mark | **yes — §4.6** Brand mark and fonts |
| Three nav buttons (Задачи / task / Host) instead of Start Page / Settings / `+` tab strip | **no — open miss.** §3.2 names this as *today → target*, not as an accepted forever-stand-in |
| Search field in the nav | **no — open miss.** Official header has no search |
| Metrics chip + **онлайн** pill instead of gauge / bell / ZA avatar | **no — open miss** for avatar + utility cluster placement. A metrics chip itself is our product (`§3.12` resource monitor still **needs live** for the *open panel*) |

**Composition: miss** (IA of the bar is not the official tab strip). Height/native chrome/wordmark are the only §4 covers.

### Palette

Both sides are signed-in **light**. Ours uses the 0130 tokens in `theme.rs` (`#FFFFFF` header, `#F6F9F8` page on full frames, `#0F0F0F` / `#666666` type, `#257174` accent on **онлайн**). Official **HA**/**HT** are white / page-wash / hairline grayscale; accent teal is not the header language (it lives on Settings toggles, frame **SG** / **A**).

| Delta | In §4? |
|---|---|
| Light tokens vs official light chrome | match (pipette law is §2.1; 0130 applied them) |
| Accent on the status pill | **no — open miss** as chrome language (accent is specified for toggles / selected nav, not as the online pill) |
| YOLO / offline banner fills on other ours frames | ours, called out in `theme.rs` / spec §2.1 / §3.8 — not this pair |

**Palette: hold on the light plate; accent-as-status is an open miss.**

### Typography

Official live default is **Figtree 15** (frame **A**). Ours is **Inter** OFL 15 (`theme.rs` `SIZE_UI`). Wordmark is RustTraycer, not Traycer.

| Delta | In §4? |
|---|---|
| Inter-class 15 instead of Figtree 15 | **yes — §4.6** |
| epaint hinting / hairline vs Chromium | **yes — §4.1** |

**Typography: accepted.**

### Density

Official tab strip ~37 px under a separate GTK menu. Ours packs wordmark + three navs + search + chips into 40 px.

| Delta | In §4? |
|---|---|
| 40 px nav vs ~37 px official tab row | **yes — §4.5** |
| Extra controls in that 40 px (search + status) | **no — open miss** (density of *widgets*, not the bar height) |

**Density: height accepted; widget packing is an open miss.**

## 3. Unpaired ours frames (not scored)

Do not grade these against a Settings page, welcome canvas, or **HE**.

- **tasks** / **empty** — our task list and archive empty card. Official Acts 01–03 / Start Page *page* / History were never captured (`§1.1`, `§5`).
- **canvas** / **chat** — agents column + empty transcript + composer. Official chat / ladder / panel stack stay **needs live** (`§3.9`–`§3.10`).
- **host** — Host diagnostics from `pid.json` (hostId `01a01b47-…`, pid 229448, `http://127.0.0.1:45927`), YOLO policy banner, **онлайн**. Not **HE**.

Light page + Inter + 0130 tokens are visible on all 1280×800 ours frames. That is theme application, not a pair.

## 4. What is not a pair (do not promote later)

- `ours/host.png` ↔ **HE**
- `ours/chrome.png` ↔ **S** / **SG** (Settings desktop is not a 40 px rusttraycer crop)
- `ours/empty.png` ↔ **W** (archive-empty card ≠ signed-out welcome)
- Any ours frame ↔ **OA** / **OH** / **P** / **AG** / **A** / **K**

## 5. Conclusion

**Not at pair-parity.** The only honest pair is the header strip. On that pair, typography and native/40 px chrome are §4. Composition (tab-strip IA, search in nav, no avatar) and accent-as-status / widget packing are **open misses** — they are *not* added to §4 by this report.

Unpaired ours frames cannot fail or pass a pair they do not have. Official Settings / welcome / onboarding / **HE** remain without an ours counterpart.

No asar. No new screenshots in this change. Host `:45927` not restarted.

## 6. Pointers

- Spec + §4 list: [`docs/design-parity-v1.md`](design-parity-v1.md)
- Official frames: [`docs/reference-screens/`](reference-screens/)
- Ours frames + capture notes: [`docs/parity-shots/README.md`](parity-shots/README.md)
- Theme tokens: `crates/rt-gui/src/theme.rs` (STAR 0130 `a605806`)
