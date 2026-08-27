# Design-parity report — official ↔ ours (STAR 0137)

Observe-only. Docs only. No asar. Host `:45927` not touched. Base `2b8c08d` (merge STAR 0135; parent of this report). Official **SP** / **HY** from 0134 are on this base. Spec §3.13 / §3.14 from 0136 are on this base.

This report pairs **live official frames** in `docs/reference-screens/` with **our** `rt-gui` frames in `docs/parity-shots/ours/` (theme 0130 / chrome IA 0133 / ours shots 0135). Criterion: composition, palette, typography, density — and only on **honest pairs**. A delta is **accepted** only if it is already on the list in [`design-parity-v1.md` §4](design-parity-v1.md#4-accepted-deviations-not-bugs). Anything else is an **open miss**, not a new accepted deviation. §4 is not expanded here.

**Pair-parity** (Chief): every considered official surface has an honest ours pair **and** the scored pairs have no open miss. Default scope includes **HT**, **HA**, **SP**, **HY**. Missing ours for **SP** / **HY** is enough for **нет**.

## 0. What changed since 0132

0132 (`fee5172`, base `c6a6a22`) scored chrome ↔ **HA**/**HT** and said **not at pair-parity**. Open misses then: no Start Page / Settings / `+` tab strip; search in the product nav; no avatar; accent `#257174` as the **онлайн** pill; utility cluster not on the right like **HT**.

0133 (`87254b4`) rewired chrome IA toward §3.2. 0135 (`90872e1`, merge `2b8c08d`) reshot ours. 0134 (`c19ad12`, merge `88633cd`) added official **SP** / **HY**. 0136 (`56a0b0a` / `92635e1`, merge `48b873a`) wrote §3.13 / §3.14 and marked those surfaces **live**. This pass re-reads the 0135 PNGs against **HT** and the current spec. 0132 misses are **not** assumed closed.

## 1. Resolution class

Both sets were taken on an Xvfb-class 1280-wide framebuffer (official `:5` / `:3`; ours 0135 `:10`, 1280×800×24). Crops keep their own height — do not mix coordinate systems (`design-parity-v1.md` §1.1).

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
| Report base | `2b8c08d` `2b8c08d013a94d46a64a61029e51f75b95d589f4` | `origin/main` after 0135 merge |
| Ours shots | `90872e1` | `docs/parity-shots/ours/{chrome,tasks,empty,canvas,chat,host}.png` + README. **No** `start-page.png`, **no** `history.png` |
| Chrome IA | `87254b4` | tab strip Start Page / Settings / `+` / avatar |
| Official **SP** / **HY** | `c19ad12` / merge `88633cd` | `logged-in-start-page.png`, `logged-in-history.png` — now in-tree on `2b8c08d` |
| Official **HA** / **HT** / **HE** | 0128 set (still in-tree) | `logged-in-header-avatar.png`, `logged-in-header-tabs-avatar.png`, `logged-in-host-error.png` |
| Spec | `48b873a` (0136 merge) on this base | §3.2 chrome, §3.13 Start Page, §3.14 History empty, §4 accepted deviations |
| Theme tokens | `a605806` | `crates/rt-gui/src/theme.rs` |

0135 README said official **SP** / **HY** lived on `88633cd` and not on the 0135 worktree base `8ce54bb`. After the 0134+0135 merges they **are** on `2b8c08d`. Pairing still requires an ours frame of the *same surface*. We do not have one.

## 3. Honest pairs

Pair only where the **surface** matches. Do not invent a pair because chrome is visible at the top of a full window. Do not pair **SP** to `tasks.png`. Do not pair **HA** to `tasks` / `empty`.

| Ours | Pixels | Official | Pixels | Pair? |
|---|---|---|---|---|
| [`ours/chrome.png`](parity-shots/ours/chrome.png) | 1280×40 | [`logged-in-header-tabs-avatar.png`](reference-screens/logged-in-header-tabs-avatar.png) **HT** | 1280×88 | **yes — strip only.** Same signed-in tab strip (Start Page / Settings / `+` / utilities / avatar). Crop heights differ (40 vs 88): official includes GTK File menu (~27 px) + ~37 px tabs + seam; ours is the 40 px product nav alone. |
| [`ours/chrome.png`](parity-shots/ours/chrome.png) | 1280×40 | [`logged-in-header-avatar.png`](reference-screens/logged-in-header-avatar.png) **HA** | 1280×100 | **no.** **HA** is menu + empty white header + **ZA** disc. No tab labels (left-of-center below the menu is `#FFFFFF` / `#F6F9F8`, no type). Ours chrome *is* the tab strip. Not the same crop, not the same IA. |
| — | — | [`logged-in-start-page.png`](reference-screens/logged-in-start-page.png) **SP** | 1280×800 | **official-only / ours missing.** No `ours/start-page.png`. Do **not** pair to `tasks.png` (that is the task list «Задачи», not greeting + composer). |
| — | — | [`logged-in-history.png`](reference-screens/logged-in-history.png) **HY** | 1280×800 | **official-only / ours missing.** No `ours/history.png`. History empty is live in spec §3.14; we did not shoot it. |
| [`ours/tasks.png`](parity-shots/ours/tasks.png) | 1280×800 | — | — | **ours-only.** Task / chat / ladder still **needs live** on the official side. |
| [`ours/empty.png`](parity-shots/ours/empty.png) | 1280×800 | — | — | **ours-only.** Archive-empty card «нет архивных» ≠ **HY** «No tasks yet» and ≠ **SP**. |
| [`ours/canvas.png`](parity-shots/ours/canvas.png) | 1280×800 | — | — | **ours-only.** |
| [`ours/chat.png`](parity-shots/ours/chat.png) | 400×680 | — | — | **ours-only.** Official chat transcript never captured. |
| [`ours/host.png`](parity-shots/ours/host.png) | 1280×800 | [`logged-in-host-error.png`](reference-screens/logged-in-host-error.png) **HE** | 1280×800 | **not a pair.** Same pixel class, different surface: ours = Host diagnostics + YOLO banner, host **online** (`pid` 28385, `:43549`); **HE** = signed-in reconnect / systemd modal. |
| — | — | **W** / **D** welcome | 1280×719 / 1280×800 | **no ours frame** |
| — | — | **S** / **SG** / **SS** / **P** / **AG** / **A** / **K** Settings acts | mix | **no ours frame** (our Settings tab opens Host diagnostics, not those pages) |
| — | — | **OA** / **OH** onboarding | 1280×800 / 1280×90 | **no ours frame** |

`docs/parity-shots/README.md` already refuses **SP**→tasks and **host**→**HE**. This report does not invent extra pairs.

## 4. Scored pair — chrome ↔ **HT**

Compare structure, tokens, and rhythm — not a 1:1 overlay (`§4.1`). Pipette on the two PNGs (Pillow `getpixel`), not on the 0135 README prose. README 0135 says «green **онлайн**»; the chrome frame has **zero** pixels of `accent.traycer-green` `#257174` and zero green-ish pixels. Pixels win.

### Composition

Official **HT**: GTK File/Edit/View/Window/Help; back/forward + layers; **Start Page** inactive (`#F6F9F8`); **Settings** active white tab + `+`; utility cluster (gauge, overflow, gear, history, bell); **ZA** avatar disc 19×20 `#EAEAEA`.

Ours `chrome.png`: back/forward; **Start Page** active white tab (`#FFFFFF` x≈67–172); **Settings** inactive + close ×; `+`; utility cluster on the right; **онлайн** pill; **RT** avatar 19×20 `#EAEAEA` at x=1244–1262, y=11–30. No File menu. No search field. No RustTraycer wordmark in this crop (0132 still had it).

| Delta | In §4? |
|---|---|
| eframe / no XFWM + no GTK File menu | **yes — §4.5** Native window chrome |
| 40 px product nav (spec target tab strip is 37 px; official crop is 88 because it also has the menu) | **yes — §4.5** (`rt-gui` keeps its own 40 px nav) |
| Start Page / Settings / `+` tab strip instead of Задачи / task / Host | **closed vs 0132.** Matches §3.2 target IA. Active vs inactive tab is session state (ours Start Page, official Settings) — not a miss |
| Search field in the nav | **closed vs 0132.** Official header has no search; ours chrome has none (search, if any, lives on the Tasks page) |
| Avatar 19×20 `#EAEAEA`, initials `#666666` | **closed vs 0132.** Size/fill match §3.2 / **HA** disc. Initials **RT** vs **ZA** are identity, not a token miss |
| Utility cluster on the right | **closed vs 0132** for *placement*. Lucide-class stand-ins vs official gauge/bell — §3.2 «Lucide-class stand-ins» + §4.6 |
| Extra **онлайн** pill (≈85×26 `#EAEAEA` at x=1151–1235) in that cluster | **no — open miss.** §3.2 names gauge / overflow / gear / history / bell / avatar. A status pill is not on that list and is not in §4 |
| Header plate is `bg.page` `#F6F9F8` with only the active tab white; official strip + right toolbar are `bg.header` `#FFFFFF` (inactive tab is the page wash) | **no — open miss.** §2.1 / §3.2: tab strip and the toolbar that holds the avatar are `#FFFFFF`. §4.5 covers height and native chrome, not the fill token |
| Hairline `#DFE9E7`: official under the strip (**HT** y=64); ours is the top row of the 40 px crop (y=0), bottom of the nav blends into page | **no — open miss** (same sandwich: official is white strip → seam → page; ours is seam → page-wash bar). Not §4.1 hairline *coverage* |

**Composition: closer than 0132 (IA of the bar is now the official tab strip), still a miss** on the extra pill and on the white header plate / seam.

### Palette

Both sides are signed-in **light**. Ours uses the 0130 tokens (`#F6F9F8` page, `#FFFFFF` active tab, `#0F0F0F` / `#666666` type, `#EAEAEA` avatar + pill, `#DFE9E7` hairline). Official **HT** is white header / page-wash inactive tab / hairline grayscale. Accent teal is not the official header language (0× `#257174` on **HT**; it lives on Settings toggles / **SP** Restart host).

| Delta | In §4? |
|---|---|
| Light tokens vs official light chrome | match on the shared tokens we actually use (page, active tab, type, avatar disc, hairline hex) |
| Accent on the status pill | **closed vs 0132.** Chrome frame: 0× `#257174`. Pill is `#EAEAEA` + gray type, not Traycer Green |
| Header plate `#F6F9F8` vs official `#FFFFFF` | **no — open miss** (see composition). Wrong token, not an epaint gamma issue |
| YOLO / offline banner fills on other ours frames | ours, called out in `theme.rs` / spec §2.1 / §3.8 — not this pair |

**Palette: hold on the light tokens that match; header-plate wash is an open miss. Accent-as-status is closed.**

### Typography

Official live default is **Figtree 15** (frame **A**). Ours is **Inter** OFL 15 (`theme.rs` `SIZE_UI`). Tab labels are the official English strings («Start Page», «Settings»). Wordmark is no longer in the chrome crop.

| Delta | In §4? |
|---|---|
| Inter-class 15 instead of Figtree 15 | **yes — §4.6** |
| epaint hinting / hairline vs Chromium | **yes — §4.1** |
| «онлайн» copy on the extra pill | the *widget* is the miss (composition); localization of our own chip is not a Figtree issue |

**Typography: accepted** on the scored strip (face/size). The pill is not a type miss.

### Density

Official tab strip ~37 px under a separate GTK menu. Ours is 40 px. Official packs icons only on the right; ours packs the same cluster **plus** an 85×26 pill into that 40 px.

| Delta | In §4? |
|---|---|
| 40 px nav vs ~37 px official tab row | **yes — §4.5** |
| Extra **онлайн** control in that 40 px | **no — open miss** (density of *widgets*, not the bar height) |

**Density: height accepted; widget packing is still an open miss.**

## 5. Unpaired ours frames (not scored)

Do not grade these against a Settings page, welcome canvas, **SP**, **HY**, or **HE**.

- **tasks** — our open-task list (STAR 0128/0131/0134 rows), filter Открытые / Архив, «Новая задача». Chrome IA visible at the top; that does **not** make this an **SP** pair.
- **empty** — Архив filter: card «нет архивных». Not History empty.
- **canvas** / **chat** — agents column + empty transcript + composer. Official chat / ladder / panel stack stay **needs live** (`§3.9`–`§3.10`).
- **host** — Host diagnostics from `pid.json` (hostId `01a01b47-…`, pid 28385, `http://127.0.0.1:43549`), YOLO policy banner, **онлайн**. Not **HE**.

Light page + Inter + 0130 tokens + 0133 tab strip are visible on all 1280×800 ours frames. That is theme + chrome application, not a pair.

## 6. What is not a pair (do not promote later)

- `ours/host.png` ↔ **HE**
- `ours/tasks.png` ↔ **SP**
- `ours/empty.png` ↔ **HY** (or **W**)
- `ours/chrome.png` ↔ **HA** (different crop / no tabs on **HA**)
- `ours/chrome.png` ↔ **S** / **SG** (Settings desktop is not a 40 px rusttraycer crop)
- Any ours frame ↔ **OA** / **OH** / **P** / **AG** / **A** / **K**

## 7. Open misses (outside §4)

On the only honest pair (**HT** ↔ `ours/chrome.png`):

1. **Header plate / seam.** Official strip + avatar toolbar = `bg.header` `#FFFFFF`, `hairline.header` `#DFE9E7` under the tabs. Ours 40 px nav is `bg.page` `#F6F9F8` with a top-row seam and a white active tab only.
2. **Extra «онлайн» pill** in the utility cluster (~85×26 `#EAEAEA`). Not in §3.2. Not an accepted deviation.

0132 chrome-IA misses that **this** pass treats as closed (pixels on `ours/chrome.png`): tab strip Start Page / Settings / `+`; search gone from the nav; avatar 19×20 `#EAEAEA`; accent not used as the online pill; utilities on the right.

Coverage (blocks pair-parity even if chrome were clean):

3. **SP** is live in spec §3.13 and in-tree as `logged-in-start-page.png` — **no ours Start Page contents frame**.
4. **HY** is live in spec §3.14 and in-tree as `logged-in-history.png` — **no ours History empty frame**.

Do not add any of these to §4.

## 8. Conclusion

**Pair-parity: нет.**

The only honest pair is the header strip **HT** ↔ `ours/chrome.png`. On that pair, typography, native/40 px chrome, Inter-class face, and the 0133 tab-strip IA are either match or §4. Remaining chrome deltas (white header plate + seam; extra **онлайн** pill) are **open misses**.

**SP** and **HY** are now first-class official surfaces on this base and have no ours pair. That alone is **нет**. **HA** has no honest ours frame. `host` ≠ **HE**.

Unpaired ours frames cannot fail or pass a pair they do not have. Official Settings / welcome / onboarding / **HE** remain without an ours counterpart.

No asar. No new screenshots in this change. Host `:45927` not restarted. §4 not edited.

## 9. Pointers

- Spec + §4 list: [`docs/design-parity-v1.md`](design-parity-v1.md) (§3.2, §3.13, §3.14, §4)
- Official frames: [`docs/reference-screens/`](reference-screens/)
- Ours frames + capture notes: [`docs/parity-shots/README.md`](parity-shots/README.md)
- Previous report: STAR 0132 `fee5172`
- Theme tokens: `crates/rt-gui/src/theme.rs` (STAR 0130 `a605806`)
- Chrome IA: STAR 0133 `87254b4`
