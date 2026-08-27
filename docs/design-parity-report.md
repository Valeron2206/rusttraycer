# Design-parity report — official ↔ ours (STAR 0143)

Observe-only. Docs only. No asar. Host `:45927` not touched. Base `c0f492e` (merge STAR 0142; parent of this report). Official **SP** / **HY** from 0134 are on this base (0134 is an ancestor — they were already in-tree on `6fb741a` and stay in-tree on `c0f492e`). Spec §2.3 `chrome.header-seam`, §3.2 chrome, §3.13 / §3.14, §4 accepted — honor §3.13 / §3.14 only for pairing honesty (do not invent **SP** / **HY** pairs).

This report pairs **live official frames** in `docs/reference-screens/` with **our** `rt-gui` frames in `docs/parity-shots/ours/` (theme 0130 / chrome IA 0133 / plate 0138 / seam 0141 / ours shots 0142). Criterion: composition, palette, typography, density — and only on **honest pairs**. A delta is **accepted** only if it is already on the list in [`design-parity-v1.md` §4](design-parity-v1.md#4-accepted-deviations-not-bugs). Anything else is an **open miss**, not a new accepted deviation. §4 is not expanded here.

**Pair-parity** (Chief, this STAR): scored official surface is **HT** only («Эталон HT»). pair-parity **да** iff the **HT** ↔ `ours/chrome.png` pair has no open miss outside §4. Missing ours for **SP** / **HY** is a **coverage leftover**, not a pair-parity blocker (unlike 0140, where default scope included **HT** / **HA** / **SP** / **HY**). **да** here is **not** whole-product pair-parity.

## 0. What changed since 0140

0140 (`f15dee5`, merge `bef588e`, base `49ab043`) scored chrome ↔ **HT** and said pair-parity **нет**. Open misses then: (1) under-strip hairline / seam (chrome crop had `#DFE9E7` on y=0; full frames were white plate → `#090909` → `#E6EFEC` → page, zero `#DFE9E7` under the bar); (2) **SP** no ours frame; (3) **HY** no ours frame. Plate `#FFFFFF` and the dropped «онлайн» pill were already closed vs 0137.

0141 (`3698378`, merge `6fb741a`) drew `hairline.header` `#DFE9E7` **under** the 40 px tab strip (not a Frame stroke at y=0). 0142 (`2938cd3`, merge `c0f492e`) reshot ours on that binary. This pass re-reads the 0142 PNGs (ffmpeg → raw RGB24), not the 0140 prose and not the 0142 README. 0140 misses are **not** assumed closed.

0142 README L25 said official **SP** / **HY** live on `88633cd` and are **not** on the 0142 worktree base `6fb741a`. That is wrong: 0134 (`c19ad12` / merge `88633cd`) is an ancestor of `6fb741a` and of `c0f492e`. The files are in-tree. Pairing still requires an ours frame of the *same surface*. We do not have one. This report does not repeat that line.

## 1. Resolution class

Both sets were taken on an Xvfb-class 1280-wide framebuffer (official `:5` / `:3`; ours 0142 `:12`, 1280×800×24 — not the 0135 `:10` or 0139 `:11` gui). Crops keep their own height — do not mix coordinate systems (`design-parity-v1.md` §1.1).

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
| Report base | `c0f492e` `c0f492ef00affae6f54b6f67c524c4690f72ced2` | `origin/main` after 0142 merge |
| Ours shots | `2938cd3` | `docs/parity-shots/ours/{chrome,tasks,empty,canvas,chat,host}.png` + README. **No** `start-page.png`, **no** `history.png` |
| Header seam | `3698378` | `hairline.header` `#DFE9E7` under the 40 px tab strip (not a y=0 Frame stroke) |
| Plate + drop pill | `4b14af4` | `header_frame` fill `BG_HEADER` + stroke `HAIRLINE_HEADER`; chrome «онлайн» pill removed |
| Chrome IA | `87254b4` | tab strip Start Page / Settings / `+` / avatar |
| Official **SP** / **HY** | `c19ad12` / merge `88633cd` | `logged-in-start-page.png`, `logged-in-history.png` — in-tree on `6fb741a` and on `c0f492e` (0134 is an ancestor) |
| Official **HA** / **HT** / **HE** | 0128 set (still in-tree) | `logged-in-header-avatar.png`, `logged-in-header-tabs-avatar.png`, `logged-in-host-error.png` |
| Spec | `48b873a` (0136 merge) on this base | §2.3 `chrome.header-seam`, §3.2 chrome, §3.13 Start Page, §3.14 History empty, §4 accepted deviations |
| Previous report | `f15dee5` / merge `bef588e` | STAR 0140; pair-parity **нет** (scope then included **SP** / **HY**) |
| Theme tokens | `a605806` | `crates/rt-gui/src/theme.rs` |

## 3. Honest pairs

Pair only where the **surface** matches. Do not invent a pair because chrome is visible at the top of a full window. Do not pair **SP** to `tasks.png`. Do not pair **HA** to `tasks` / `empty`. Do not pair **HY** to `empty.png`. Chief scoped this pass to **эталон HT** — **HA** / **SP** / **HY** are listed so we do not invent pairs, not because they are scored.

| Ours | Pixels | Official | Pixels | Pair? |
|---|---|---|---|---|
| [`ours/chrome.png`](parity-shots/ours/chrome.png) | 1280×40 | [`logged-in-header-tabs-avatar.png`](reference-screens/logged-in-header-tabs-avatar.png) **HT** | 1280×88 | **yes — strip only. Scored this STAR.** Same signed-in tab strip (Start Page / Settings / `+` / utilities / avatar). Crop heights differ (40 vs 88): official includes GTK File menu (~27 px) + ~37 px tabs + seam; ours is the 40 px product nav alone. |
| [`ours/chrome.png`](parity-shots/ours/chrome.png) | 1280×40 | [`logged-in-header-avatar.png`](reference-screens/logged-in-header-avatar.png) **HA** | 1280×100 | **no — not scored.** **HA** is menu + empty white header + **ZA** disc. No tab labels (left-of-center below the menu is `#FFFFFF`, no type — 0× `#0F0F0F` on the frame). Ours chrome *is* the tab strip. Not the same crop, not the same IA. |
| — | — | [`logged-in-start-page.png`](reference-screens/logged-in-start-page.png) **SP** | 1280×800 | **official-only / not a pair / not scored.** No `ours/start-page.png`. Do **not** pair to `tasks.png` (that is the task list «Задачи», not greeting + composer). The official file **is** on this base. Coverage leftover, not a pair-parity blocker this STAR. |
| — | — | [`logged-in-history.png`](reference-screens/logged-in-history.png) **HY** | 1280×800 | **official-only / not a pair / not scored.** No `ours/history.png`. History empty is live in spec §3.14; we did not shoot it. The official file **is** on this base. Coverage leftover, not a pair-parity blocker this STAR. |
| [`ours/tasks.png`](parity-shots/ours/tasks.png) | 1280×800 | — | — | **ours-only.** Task / chat / ladder still **needs live** on the official side. Used to confirm the chrome/page join. |
| [`ours/empty.png`](parity-shots/ours/empty.png) | 1280×800 | — | — | **ours-only.** Archive-empty card «нет архивных» ≠ **HY** «No tasks yet» and ≠ **SP**. |
| [`ours/canvas.png`](parity-shots/ours/canvas.png) | 1280×800 | — | — | **ours-only.** |
| [`ours/chat.png`](parity-shots/ours/chat.png) | 400×680 | — | — | **ours-only.** Official chat transcript never captured. |
| [`ours/host.png`](parity-shots/ours/host.png) | 1280×800 | [`logged-in-host-error.png`](reference-screens/logged-in-host-error.png) **HE** | 1280×800 | **not a pair.** Same pixel class, different surface: ours = Host diagnostics + YOLO banner, host **online** (`pid` 35281, `:43811`); **HE** = signed-in reconnect / systemd modal. |
| — | — | **W** / **D** welcome | 1280×719 / 1280×800 | **no ours frame** |
| — | — | **S** / **SG** / **SS** / **P** / **AG** / **A** / **K** Settings acts | mix | **no ours frame** (our Settings tab opens Host diagnostics, not those pages) |
| — | — | **OA** / **OH** onboarding | 1280×800 / 1280×90 | **no ours frame** |

`docs/parity-shots/README.md` already refuses **host**→**HE**. This report does not invent extra pairs and does not treat the 0142 README «SP/HY not on `6fb741a`» line as fact.

## 4. Scored pair — chrome ↔ **HT**

Compare structure, tokens, and rhythm — not a 1:1 overlay (`§4.1`). Pipette on the 0142 PNGs (ffmpeg → raw RGB24), not on the 0140 report and not on the 0142 README prose. Full-frame join confirmed on `tasks.png` (same sandwich on `empty` / `canvas` / `host`).

### Composition

Official **HT**: GTK File/Edit/View/Window/Help (`#F6F5F4` y=0…26); back/forward + layers; **Start Page** inactive (`#F6F9F8`); **Settings** active white tab + `+`; utility cluster (gauge, overflow, gear, history, bell); **ZA** avatar disc `#EAEAEA`. Hairline `#DFE9E7` **under** the strip (**HT** y=64, 1067 px; the rest of that row is still `#F6F9F8` inactive-tab / page wash), then page `#F6F9F8` from y=65.

Ours `chrome.png`: back/forward; **Start Page** active white tab (`#FFFFFF` through ~x=180); **Settings** inactive `#F6F9F8` (x≈180–284) + close ×; `+` `#F6F9F8` (x≈292–319); utility cluster on the right; **RT** avatar `#EAEAEA` ~19×20 at x=1245–1263, y=10–29. **No «онлайн» pill.** No File menu. No search field. No RustTraycer wordmark in this crop.

| Delta | In §4? |
|---|---|
| eframe / no XFWM + no GTK File menu | **yes — §4.5** Native window chrome |
| 40 px product nav (spec target tab strip is 37 px; official crop is 88 because it also has the menu) | **yes — §4.5** (`rt-gui` keeps its own 40 px nav) |
| Start Page / Settings / `+` tab strip | **closed vs 0132 / still closed.** Matches §3.2 target IA. Active vs inactive tab is session state (ours Start Page, official Settings) — not a miss |
| Search field in the nav | **closed vs 0132.** Official header has no search; ours chrome has none |
| Avatar ~19×20 `#EAEAEA`, initials `#666666` | **closed vs 0132.** Size/fill match §3.2 / **HA** disc. Initials **RT** vs **ZA** are identity, not a token miss |
| Utility cluster on the right | **closed vs 0132** for *placement*. Lucide-class stand-ins vs official gauge/bell — §3.2 «Lucide-class stand-ins» + §4.6 |
| Extra **онлайн** pill (~85×26 `#EAEAEA` at x≈1151–1235 in 0137) | **closed vs 0137 / still closed.** Chrome frame: `#EAEAEA` long runs are the RT disc only (x=1245–1263). Right-of-utilities field is `#FFFFFF`. 0× `#257174`. Status «онлайн» is a Host-page field (`ours/host.png`), not a chrome widget |
| Header plate `bg.header` `#FFFFFF` (bar / between-tabs / right toolbar); `#F6F9F8` only on the inactive Settings tab and `+` | **closed vs 0137 / still closed.** 44 682 px `#FFFFFF` on the 40×1280 crop. Official strip + avatar toolbar are `#FFFFFF`; inactive tab is the page wash — same token split as §2.1 / §3.2 |
| Hairline `#DFE9E7` **under** the strip | **closed vs 0140 miss (1).** On `chrome.png` y=0 is `#FFFFFF` (1258 px; 0× `#DFE9E7`). y=38 is white plate (1252× `#FFFFFF` + 28× `#F6F9F8`). y=39 is **full-width** `#DFE9E7` (1280 px) — spec §2.3 `chrome.header-seam`. Four stray `#DFE9E7` pixels elsewhere are icon AA, not a second seam. On the *full* frames (`tasks` / `empty` / `canvas` / `host`) the chrome/page join is y=38 white plate → y=39 `#DFE9E7` (1280 px) → y=40 `#F6F9F8` page. **Zero** `#090909` / `#E6EFEC` at the join (the 0139 sandwich). Official sandwich is white strip → `#DFE9E7` → page. Same tokens, same place |

**Composition: plate, pill, and under-strip seam closed vs 0140.**

### Palette

Both sides are signed-in **light**. Ours uses the 0130 / 0138 / 0141 tokens (`#FFFFFF` header plate, `#F6F9F8` inactive tab / page, `#0F0F0F` / `#666666` type, `#EAEAEA` avatar, `#DFE9E7` hairline under the bar). Official **HT** is white header / page-wash inactive tab / hairline grayscale. Accent teal is not the official header language (0× `#257174` on **HT** and on ours chrome).

| Delta | In §4? |
|---|---|
| Light tokens vs official light chrome | match on the shared tokens we actually use (page, active tab, type, avatar disc, hairline hex *and* placement) |
| Accent on a status pill | **closed vs 0132 / still closed.** Chrome frame: 0× `#257174`. No pill |
| Header plate `#FFFFFF` vs official `#FFFFFF` | **closed vs 0137 / still closed.** Wrong-token wash is gone |
| Hairline token under the bar | **closed vs 0140.** Placement now matches §2.3 / official **HT** y=64 sandwich. Not an epaint gamma issue |
| YOLO / offline banner fills on other ours frames | ours, called out in `theme.rs` / spec §2.1 / §3.8 — not this pair |

**Palette: plate wash closed; under-strip seam closed. Accent-as-status stays closed.**

### Typography

Official live default is **Figtree 15** (frame **A**). Ours is **Inter** OFL 15 (`theme.rs` `SIZE_UI`). Tab labels are the official English strings («Start Page», «Settings»). Wordmark is not in the chrome crop. No «онлайн» copy on the strip.

| Delta | In §4? |
|---|---|
| Inter-class 15 instead of Figtree 15 | **yes — §4.6** |
| epaint hinting / hairline vs Chromium | **yes — §4.1** (hinting / coverage — not a missing under-strip row) |

**Typography: accepted** on the scored strip (face/size).

### Density

Official tab strip ~37 px under a separate GTK menu. Ours is 40 px. Official packs icons only on the right; ours packs the same cluster **without** the 85×26 pill.

| Delta | In §4? |
|---|---|
| 40 px nav vs ~37 px official tab row | **yes — §4.5** |
| Extra **онлайн** control in that 40 px | **closed vs 0137 / still closed.** Widget packing of the scored strip matches the official cluster (icons + avatar) |

**Density: height accepted; extra-widget packing closed.**

## 5. Unpaired ours frames (not scored)

Do not grade these against a Settings page, welcome canvas, **SP**, **HY**, or **HE**.

- **tasks** — our open-task list (STAR 0142/0139/0135/0134/0131/0128 rows), filter Открытые / Архив, «Новая задача». White-plate chrome + under-strip seam at the top; that does **not** make this an **SP** pair. Used only to read the chrome/page join (y=38…40).
- **empty** — Архив filter: card «нет архивных». Not History empty.
- **canvas** / **chat** — agents column + empty transcript + composer. Official chat / ladder / panel stack stay **needs live** (`§3.9`–`§3.10`).
- **host** — Host diagnostics from `pid.json` (hostId `01a01b47-…`, pid 35281, `http://127.0.0.1:43811`), YOLO policy banner, status **онлайн** as a *page* field (0138 moved it off chrome). Not **HE**.

Light page + Inter + 0130 tokens + 0133 tab strip + 0138 white plate + 0141 under-strip seam are visible on all 1280×800 ours frames. That is theme + chrome application, not a pair.

## 6. What is not a pair (do not promote later)

- `ours/host.png` ↔ **HE**
- `ours/tasks.png` ↔ **SP**
- `ours/empty.png` ↔ **HY** (or **W**)
- `ours/chrome.png` ↔ **HA** (different crop / no tabs on **HA**)
- `ours/chrome.png` ↔ **S** / **SG** (Settings desktop is not a 40 px rusttraycer crop)
- Any ours frame ↔ **OA** / **OH** / **P** / **AG** / **A** / **K**

## 7. Open misses (outside §4)

On the only scored honest pair (**HT** ↔ `ours/chrome.png`): **none.**

0140 chrome miss that **this** pass treats as closed (pixels on `ours/chrome.png` + the full-frame join on `tasks.png`):

1. **Under-strip hairline / seam.** Official: white plate → `#DFE9E7` under the tabs → page. Ours chrome crop: y=0 `#FFFFFF`, y=39 full-width `#DFE9E7`. Full frames: y=38 white → y=39 `#DFE9E7` → y=40 `#F6F9F8`. Not the 0139 `#090909` / `#E6EFEC` sandwich. Matches §2.3 `chrome.header-seam`.

0137 chrome misses that stay closed (pixels still hold on the 0142 crop):

- **Header plate `#FFFFFF`.** Bar / between-tabs / right toolbar are white. `#F6F9F8` only on the inactive Settings tab and `+` — the official inactive-tab token, not a page-wash bar.
- **Extra «онлайн» pill.** Gone. `#EAEAEA` on the strip is the RT avatar only.

Coverage leftovers (Chief scoped this STAR to **HT**; these do **not** block pair-parity here — unlike 0140):

- **SP** is live in spec §3.13 and in-tree as `logged-in-start-page.png` — **no ours Start Page contents frame**. Official-only / not a pair.
- **HY** is live in spec §3.14 and in-tree as `logged-in-history.png` — **no ours History empty frame**. Official-only / not a pair.

Do not add any of these to §4.

## 8. Conclusion

**Pair-parity: да** — **HT-only scope.** Chief scored «Эталон HT». The honest scored pair **HT** ↔ `ours/chrome.png` has no open miss outside §4. That is **not** whole-product pair-parity.

On that pair, 0141/0142 close the 0140 under-strip seam. Plate and dropped pill stay closed vs 0137. Typography, native/40 px chrome, Inter-class face, and the 0133 tab-strip IA stay match or §4.

**SP** and **HY** are first-class official surfaces on this base (in-tree since 0134; 0142 README L25 was wrong) and have no ours pair. That is a **coverage leftover**, not a pair-parity blocker under this STAR's HT-only definition. **HA** has no honest ours frame and was not scored. `host` ≠ **HE**.

Unpaired ours frames cannot fail or pass a pair they do not have. Official Settings / welcome / onboarding / **HE** remain without an ours counterpart.

No asar. No new screenshots in this change. Host `:45927` not restarted. §4 not edited. Code / official screens / ours PNGs / `design-parity-v1.md` not touched.

## 9. Pointers

- Spec + §4 list: [`docs/design-parity-v1.md`](design-parity-v1.md) (§2.3 `chrome.header-seam`, §3.2, §3.13, §3.14, §4)
- Official frames: [`docs/reference-screens/`](reference-screens/)
- Ours frames + capture notes: [`docs/parity-shots/README.md`](parity-shots/README.md)
- Previous report: STAR 0140 `f15dee5` (merge `bef588e`)
- Seam: STAR 0141 `3698378`
- Ours reshoot: STAR 0142 `2938cd3`
- Plate: STAR 0138 `4b14af4`
- Theme tokens: `crates/rt-gui/src/theme.rs` (STAR 0130 `a605806`)
- Chrome IA: STAR 0133 `87254b4`
