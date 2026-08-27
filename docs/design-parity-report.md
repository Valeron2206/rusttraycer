# Design-parity report — official ↔ ours (STAR 0140)

Observe-only. Docs only. No asar. Host `:45927` not touched. Base `49ab043` (merge STAR 0139; parent of this report). Official **SP** / **HY** from 0134 are on this base (0134 is an ancestor — they were already in-tree on `9de19f3`). Spec §3.13 / §3.14 from 0136 are on this base.

This report pairs **live official frames** in `docs/reference-screens/` with **our** `rt-gui` frames in `docs/parity-shots/ours/` (theme 0130 / chrome IA 0133 / plate 0138 / ours shots 0139). Criterion: composition, palette, typography, density — and only on **honest pairs**. A delta is **accepted** only if it is already on the list in [`design-parity-v1.md` §4](design-parity-v1.md#4-accepted-deviations-not-bugs). Anything else is an **open miss**, not a new accepted deviation. §4 is not expanded here.

**Pair-parity** (Chief): every considered official surface has an honest ours pair **and** the scored pairs have no open miss. Default scope includes **HT**, **HA**, **SP**, **HY**. Missing ours for **SP** / **HY** is enough for **нет**. Do not claim **да** just because the **HT** chrome plate is clean.

## 0. What changed since 0137

0137 (`945302c`, merge `d9b7ea0`, base `2b8c08d`) scored chrome ↔ **HT** and said pair-parity **нет**. Open misses then: (1) header plate `#F6F9F8` vs official `#FFFFFF` + seam (hairline only on the top row of a page-wash bar); (2) extra «онлайн» pill in the utility cluster; (3) **SP** no ours frame; (4) **HY** no ours frame.

0138 (`4b14af4`, merge `9de19f3`) set `header_frame` fill `BG_HEADER` `#FFFFFF` + stroke `HAIRLINE_HEADER` `#DFE9E7` and removed the chrome pill (status «онлайн» moved onto the Host page). 0139 (`4cfe9a9`, merge `49ab043`) reshot ours on that binary. This pass re-reads the 0139 PNGs (ffmpeg → raw RGB, not the 0137 prose and not the 0139 README). 0137 misses are **not** assumed closed.

0139 README L25 said official **SP** / **HY** live on `88633cd` and are **not** on the 0139 worktree base `9de19f3`. That is wrong: 0134 (`c19ad12` / merge `88633cd`) is an ancestor of `9de19f3` and of `49ab043`. The files are in-tree. Pairing still requires an ours frame of the *same surface*. We do not have one. This report does not repeat that line.

## 1. Resolution class

Both sets were taken on an Xvfb-class 1280-wide framebuffer (official `:5` / `:3`; ours 0139 `:11`, 1280×800×24 — not the 0135 `:10` gui). Crops keep their own height — do not mix coordinate systems (`design-parity-v1.md` §1.1).

| Set | Typical frame | Notes |
|---|---|---|
| Official full desktop | 1280×800 | XFWM 24 px + client + Plank on some frames (**SP**, **HY**, **HE**, **S**) |
| Official client | 1280×719 | **W**, **SG** |
| Official header crops | 1280×100 **HA**, 1280×88 **HT**, 1280×90 **OH** | Product chrome + GTK menu |
| Ours window | 1280×800 | eframe client; `ours/chrome.png` is a 1280×40 product-nav crop |
| Ours chat crop | 400×680 | Right pane only |

Same **class** = 1280-wide Xvfb. Same **crop** is required before scoring a pair. A 1280×40 nav strip is not the same crop as a 1280×88 official header (menu + tabs + avatar). State the height delta; do not overlay.

## 2. Sources on this base

| Role | SHA (short) | What |
|---|---|---|
| Report base | `49ab043` `49ab0439ebdc166ee3c4a16f040450663e73c2ee` | `origin/main` after 0139 merge |
| Ours shots | `4cfe9a9` | `docs/parity-shots/ours/{chrome,tasks,empty,canvas,chat,host}.png` + README. **No** `start-page.png`, **no** `history.png` |
| Plate + drop pill | `4b14af4` | `header_frame` fill `BG_HEADER` + stroke `HAIRLINE_HEADER`; chrome «онлайн» pill removed |
| Chrome IA | `87254b4` | tab strip Start Page / Settings / `+` / avatar |
| Official **SP** / **HY** | `c19ad12` / merge `88633cd` | `logged-in-start-page.png`, `logged-in-history.png` — in-tree on `9de19f3` and on `49ab043` (0134 is an ancestor) |
| Official **HA** / **HT** / **HE** | 0128 set (still in-tree) | `logged-in-header-avatar.png`, `logged-in-header-tabs-avatar.png`, `logged-in-host-error.png` |
| Spec | `48b873a` (0136 merge) on this base | §3.2 chrome, §3.13 Start Page, §3.14 History empty, §4 accepted deviations |
| Previous report | `945302c` / merge `d9b7ea0` | STAR 0137; pair-parity **нет** |
| Theme tokens | `a605806` | `crates/rt-gui/src/theme.rs` |

## 3. Honest pairs

Pair only where the **surface** matches. Do not invent a pair because chrome is visible at the top of a full window. Do not pair **SP** to `tasks.png`. Do not pair **HA** to `tasks` / `empty`. Do not pair **HY** to `empty.png`.

| Ours | Pixels | Official | Pixels | Pair? |
|---|---|---|---|---|
| [`ours/chrome.png`](parity-shots/ours/chrome.png) | 1280×40 | [`logged-in-header-tabs-avatar.png`](reference-screens/logged-in-header-tabs-avatar.png) **HT** | 1280×88 | **yes — strip only.** Same signed-in tab strip (Start Page / Settings / `+` / utilities / avatar). Crop heights differ (40 vs 88): official includes GTK File menu (~27 px) + ~37 px tabs + seam; ours is the 40 px product nav alone. |
| [`ours/chrome.png`](parity-shots/ours/chrome.png) | 1280×40 | [`logged-in-header-avatar.png`](reference-screens/logged-in-header-avatar.png) **HA** | 1280×100 | **no.** **HA** is menu + empty white header + **ZA** disc. No tab labels (left-of-center below the menu is `#FFFFFF`, no type — 0× `#0F0F0F` on the frame). Ours chrome *is* the tab strip. Not the same crop, not the same IA. |
| — | — | [`logged-in-start-page.png`](reference-screens/logged-in-start-page.png) **SP** | 1280×800 | **official-only / ours missing.** No `ours/start-page.png`. Do **not** pair to `tasks.png` (that is the task list «Задачи», not greeting + composer). The official file **is** on this base. |
| — | — | [`logged-in-history.png`](reference-screens/logged-in-history.png) **HY** | 1280×800 | **official-only / ours missing.** No `ours/history.png`. History empty is live in spec §3.14; we did not shoot it. The official file **is** on this base. |
| [`ours/tasks.png`](parity-shots/ours/tasks.png) | 1280×800 | — | — | **ours-only.** Task / chat / ladder still **needs live** on the official side. |
| [`ours/empty.png`](parity-shots/ours/empty.png) | 1280×800 | — | — | **ours-only.** Archive-empty card «нет архивных» ≠ **HY** «No tasks yet» and ≠ **SP**. |
| [`ours/canvas.png`](parity-shots/ours/canvas.png) | 1280×800 | — | — | **ours-only.** |
| [`ours/chat.png`](parity-shots/ours/chat.png) | 400×680 | — | — | **ours-only.** Official chat transcript never captured. |
| [`ours/host.png`](parity-shots/ours/host.png) | 1280×800 | [`logged-in-host-error.png`](reference-screens/logged-in-host-error.png) **HE** | 1280×800 | **not a pair.** Same pixel class, different surface: ours = Host diagnostics + YOLO banner, host **online** (`pid` 35281, `:43811`); **HE** = signed-in reconnect / systemd modal. |
| — | — | **W** / **D** welcome | 1280×719 / 1280×800 | **no ours frame** |
| — | — | **S** / **SG** / **SS** / **P** / **AG** / **A** / **K** Settings acts | mix | **no ours frame** (our Settings tab opens Host diagnostics, not those pages) |
| — | — | **OA** / **OH** onboarding | 1280×800 / 1280×90 | **no ours frame** |

`docs/parity-shots/README.md` already refuses **host**→**HE**. This report does not invent extra pairs and does not treat the 0139 README «SP/HY not on `9de19f3`» line as fact.

## 4. Scored pair — chrome ↔ **HT**

Compare structure, tokens, and rhythm — not a 1:1 overlay (`§4.1`). Pipette on the 0139 PNGs (ffmpeg → raw RGB24), not on the 0137 report and not on the 0139 README prose.

### Composition

Official **HT**: GTK File/Edit/View/Window/Help (`#F6F5F4` y=0…26); back/forward + layers; **Start Page** inactive (`#F6F9F8`); **Settings** active white tab + `+`; utility cluster (gauge, overflow, gear, history, bell); **ZA** avatar disc `#EAEAEA`. Hairline `#DFE9E7` **under** the strip (**HT** y=64, 1067 px), then page `#F6F9F8` from y=65.

Ours `chrome.png`: back/forward; **Start Page** active white tab (`#FFFFFF` through ~x=180); **Settings** inactive `#F6F9F8` (x≈181–284) + close ×; `+` `#F6F9F8` (x≈293–320); utility cluster on the right; **RT** avatar `#EAEAEA` ~19×20 at x=1244–1262, y=11–30. **No «онлайн» pill.** No File menu. No search field. No RustTraycer wordmark in this crop.

| Delta | In §4? |
|---|---|
| eframe / no XFWM + no GTK File menu | **yes — §4.5** Native window chrome |
| 40 px product nav (spec target tab strip is 37 px; official crop is 88 because it also has the menu) | **yes — §4.5** (`rt-gui` keeps its own 40 px nav) |
| Start Page / Settings / `+` tab strip | **closed vs 0132 / still closed.** Matches §3.2 target IA. Active vs inactive tab is session state (ours Start Page, official Settings) — not a miss |
| Search field in the nav | **closed vs 0132.** Official header has no search; ours chrome has none |
| Avatar ~19×20 `#EAEAEA`, initials `#666666` | **closed vs 0132.** Size/fill match §3.2 / **HA** disc. Initials **RT** vs **ZA** are identity, not a token miss |
| Utility cluster on the right | **closed vs 0132** for *placement*. Lucide-class stand-ins vs official gauge/bell — §3.2 «Lucide-class stand-ins» + §4.6 |
| Extra **онлайн** pill (~85×26 `#EAEAEA` at x≈1151–1235 in 0137) | **closed vs 0137.** Chrome frame: `#EAEAEA` long runs are the RT disc only (x=1244–1262). Right-of-utilities field is `#FFFFFF`. 0× `#257174`. Status «онлайн» is a Host-page field (`ours/host.png`), not a chrome widget |
| Header plate `bg.header` `#FFFFFF` (bar / between-tabs / right toolbar); `#F6F9F8` only on the inactive Settings tab and `+` | **closed vs 0137 miss (1) plate.** 44 604 px `#FFFFFF` on the 40×1280 crop. Official strip + avatar toolbar are `#FFFFFF`; inactive tab is the page wash — same token split as §2.1 / §3.2 |
| Hairline `#DFE9E7` **under** the strip | **no — leftover of 0137 miss (1) seam.** On `chrome.png` `#DFE9E7` is a **full-width TOP row y=0** (1280 px) plus the left/right edges (x=0 / x=1279). Bottom of the crop y=39 is 1250× `#FFFFFF` + 28× `#F6F9F8` (`+`) + 2 edge px — not an under-strip seam. On the *full* frames (`tasks` / `empty` / `canvas` / `host`) the chrome/page join is y=39 white plate → y=40 `#090909` (1280 px) → y=41 `#E6EFEC` (1280 px, `hairline.sidebar` hex) → y=42 `#F6F9F8` page. **Zero** `#DFE9E7` under the bar. Official sandwich is white strip → `#DFE9E7` → page. Not §4.1 hairline *coverage* (that is epaint vs Chromium, not a missing seam) |

**Composition: plate and pill closed vs 0137; under-strip hairline still a miss.**

### Palette

Both sides are signed-in **light**. Ours uses the 0130 / 0138 tokens (`#FFFFFF` header plate, `#F6F9F8` inactive tab / page, `#0F0F0F` / `#666666` type, `#EAEAEA` avatar, `#DFE9E7` hairline hex — but the hairline is not in the official place). Official **HT** is white header / page-wash inactive tab / hairline grayscale. Accent teal is not the official header language (0× `#257174` on **HT** and on ours chrome).

| Delta | In §4? |
|---|---|
| Light tokens vs official light chrome | match on the shared tokens we actually use (page, active tab, type, avatar disc, hairline *hex*) |
| Accent on a status pill | **closed vs 0132 / still closed.** Chrome frame: 0× `#257174`. No pill |
| Header plate `#FFFFFF` vs official `#FFFFFF` | **closed vs 0137.** Wrong-token wash is gone |
| Hairline token present, not under the bar | **open miss** (placement — see composition). Wrong sandwich, not an epaint gamma issue |
| YOLO / offline banner fills on other ours frames | ours, called out in `theme.rs` / spec §2.1 / §3.8 — not this pair |

**Palette: plate wash closed; under-strip seam still an open miss. Accent-as-status stays closed.**

### Typography

Official live default is **Figtree 15** (frame **A**). Ours is **Inter** OFL 15 (`theme.rs` `SIZE_UI`). Tab labels are the official English strings («Start Page», «Settings»). Wordmark is not in the chrome crop. No «онлайн» copy on the strip.

| Delta | In §4? |
|---|---|
| Inter-class 15 instead of Figtree 15 | **yes — §4.6** |
| epaint hinting / hairline vs Chromium | **yes — §4.1** (hinting / coverage — not the missing under-strip row) |

**Typography: accepted** on the scored strip (face/size).

### Density

Official tab strip ~37 px under a separate GTK menu. Ours is 40 px. Official packs icons only on the right; ours now packs the same cluster **without** the 85×26 pill.

| Delta | In §4? |
|---|---|
| 40 px nav vs ~37 px official tab row | **yes — §4.5** |
| Extra **онлайн** control in that 40 px | **closed vs 0137.** Widget packing of the scored strip matches the official cluster (icons + avatar) |

**Density: height accepted; extra-widget packing closed.**

## 5. Unpaired ours frames (not scored)

Do not grade these against a Settings page, welcome canvas, **SP**, **HY**, or **HE**.

- **tasks** — our open-task list (STAR 0135/0134/0131/0128 rows), filter Открытые / Архив, «Новая задача». White-plate chrome at the top; that does **not** make this an **SP** pair. Used only to read the chrome/page join (y=40…42).
- **empty** — Архив filter: card «нет архивных». Not History empty.
- **canvas** / **chat** — agents column + empty transcript + composer. Official chat / ladder / panel stack stay **needs live** (`§3.9`–`§3.10`).
- **host** — Host diagnostics from `pid.json` (hostId `01a01b47-…`, pid 35281, `http://127.0.0.1:43811`), YOLO policy banner, status **онлайн** as a *page* field (0138 moved it off chrome). Not **HE**.

Light page + Inter + 0130 tokens + 0133 tab strip + 0138 white plate are visible on all 1280×800 ours frames. That is theme + chrome application, not a pair.

## 6. What is not a pair (do not promote later)

- `ours/host.png` ↔ **HE**
- `ours/tasks.png` ↔ **SP**
- `ours/empty.png` ↔ **HY** (or **W**)
- `ours/chrome.png` ↔ **HA** (different crop / no tabs on **HA**)
- `ours/chrome.png` ↔ **S** / **SG** (Settings desktop is not a 40 px rusttraycer crop)
- Any ours frame ↔ **OA** / **OH** / **P** / **AG** / **A** / **K**

## 7. Open misses (outside §4)

On the only honest pair (**HT** ↔ `ours/chrome.png`):

1. **Under-strip hairline / seam** (leftover of 0137 miss 1). Official: white plate → `#DFE9E7` under the tabs → page. Ours chrome crop: `#DFE9E7` on y=0 (and the side edges); y=39 is white. Full frames: white plate → `#090909` → `#E6EFEC` → page. No `#DFE9E7` at the join. Not §4.1.

0137 chrome misses that **this** pass treats as closed (pixels on `ours/chrome.png` + the full-frame join):

- **Header plate `#FFFFFF`.** Bar / between-tabs / right toolbar are white. `#F6F9F8` only on the inactive Settings tab and `+` — the official inactive-tab token, not a page-wash bar.
- **Extra «онлайн» pill.** Gone. `#EAEAEA` on the strip is the RT avatar only.

Coverage (blocks pair-parity even if the **HT** seam were clean):

2. **SP** is live in spec §3.13 and in-tree as `logged-in-start-page.png` — **no ours Start Page contents frame**.
3. **HY** is live in spec §3.14 and in-tree as `logged-in-history.png` — **no ours History empty frame**.

Do not add any of these to §4.

## 8. Conclusion

**Pair-parity: нет.**

The only honest pair is the header strip **HT** ↔ `ours/chrome.png`. On that pair, the 0138 plate and dropped pill close the 0137 chrome misses for fill and extra widget. Typography, native/40 px chrome, Inter-class face, and the 0133 tab-strip IA stay match or §4. The under-strip `#DFE9E7` seam is still an **open miss**.

**SP** and **HY** are first-class official surfaces on this base (in-tree since 0134; 0139 README L25 was wrong) and have no ours pair. That alone is **нет**. **HA** has no honest ours frame. `host` ≠ **HE**.

Unpaired ours frames cannot fail or pass a pair they do not have. Official Settings / welcome / onboarding / **HE** remain without an ours counterpart.

No asar. No new screenshots in this change. Host `:45927` not restarted. §4 not edited. Code / official screens / ours PNGs not touched.

## 9. Pointers

- Spec + §4 list: [`docs/design-parity-v1.md`](design-parity-v1.md) (§3.2, §3.13, §3.14, §4)
- Official frames: [`docs/reference-screens/`](reference-screens/)
- Ours frames + capture notes: [`docs/parity-shots/README.md`](parity-shots/README.md)
- Previous report: STAR 0137 `945302c` (merge `d9b7ea0`)
- Plate: STAR 0138 `4b14af4`
- Ours reshoot: STAR 0139 `4cfe9a9`
- Theme tokens: `crates/rt-gui/src/theme.rs` (STAR 0130 `a605806`)
- Chrome IA: STAR 0133 `87254b4`
