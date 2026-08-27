# Design-parity v1 — visual tokens and component map

For: UI (`rt-gui`, eframe + egui).
From: Architect. Date: 2026-08-21. Not code. No crate bump.
Base: STAR 0126 pin + STAR 0128 logged-in frames + STAR 0134 Start Page / History empty in `docs/reference-env.md` (Traycer Desktop `desktop-v1.1.10`).
Status: visual-parity contract, Start Page contents and History empty now live. Tokens pipetted from a live frame are law for those surfaces. Tokens that exist only as public-docs IA are **not** law for hex. Chat transcript, ladder, and Acts 01–03 (tasks / layout / handoff) were not on screen — those rows stay **needs live**.

This file does not replace `docs/gui-ia-v0.md` (screens, RPC, empty states) or `docs/e1-canvas-v2.md` / `docs/e2-ladder-v2.md` (behavior). It tells `rt-gui` what the official Desktop *looks like*, what to implement in egui, and which mismatches are accepted engine limits rather than bugs.

---

## 0. Legal hygiene

- Do **not** unpack or open `app.asar`. Do **not** copy CSS, fonts, icons, or other Traycer brand assets from the AppImage / squashfs tree.
- Screenshots of the running window are *our* files and may live under `docs/reference-screens/`. The AppImage binary stays out of git (see `docs/reference-env.md`).
- Do **not** vendor Traycer fonts. Recommendation only: an Inter-class geometric sans under the SIL Open Font License (for example [Inter](https://github.com/rsms/inter)). The live Appearance page names **Figtree** as the default UI face — that is a *product setting we observed*, not a license to vendor Figtree from the AppImage. Recommendation only for icons: [Lucide](https://lucide.dev) (ISC). Neither is a brand clone.
- Do not reproduce the official Traycer mark. RustTraycer keeps its own wordmark (`chrome.rs` already says "RustTraycer").
- Public pages on https://docs.traycer.ai were fetched as a **fallback** for surfaces still not captured live (chat transcript, ladder card, History *rows*, Acts 01–03). Start Page *contents* and History *empty* are live (**SP** / **HY**). Fallback pages are documentation, not live captures. This spec does **not** claim they are screenshots of Desktop 1.1.10.

---

## 1. Sources

### 1.1 Live in-tree frames (law for hex / ruler)

Two sessions, same pinned build `desktop-v1.1.10`. Method and sha256: `docs/reference-env.md`.

**Welcome (STAR 0126).** Observe-only X11 dumps on `DISPLAY=:5` (1280×800×24). Taken 2026-08-21 09:37 YEKT / 04:37 UTC. Signed out.

| ID | Path | Pixels | What it is |
|---|---|---|---|
| **W** | [`docs/reference-screens/welcome-sign-in.png`](reference-screens/welcome-sign-in.png) | 1280×719 RGB | Official client window: native File/Edit/View/Window/Help strip, black canvas, white mark, heading "Welcome to Traycer", single **Sign in** button, grainy teal floor glow. No email field. |
| **D** | [`docs/reference-screens/display5-1280x800-desktop.png`](reference-screens/display5-1280x800-desktop.png) | 1280×800 RGB | Same moment, full `:5` framebuffer. XFWM title bar "Traycer" (24 px) above **W**, Plank dock below. Client pixels match **W** at `y_D = y_W + 24`. |

**Logged-in (STAR 0128).** `DISPLAY=:3`, Electron HOME `/workspace/ref-traycer/home-d3`, avatar initials **ZA**. Full-desktop PNGs are 1280×800 (XFWM 24 px + client + Plank). Client-window dumps are 1280×719, same origin convention as **W**. Crops keep their own origin — cite the frame, do not mix coordinate systems.

| ID | Path | Pixels | What it is |
|---|---|---|---|
| **HE** | [`docs/reference-screens/logged-in-host-error.png`](reference-screens/logged-in-host-error.png) | 1280×800 RGB | Signed-in light canvas, ZA avatar, official-host systemd modal (before process start). |
| **HA** | [`docs/reference-screens/logged-in-header-avatar.png`](reference-screens/logged-in-header-avatar.png) | 1280×100 RGB | Crop: File/Edit/View/Window/Help + ZA avatar on the white signed-in header. |
| **HT** | [`docs/reference-screens/logged-in-header-tabs-avatar.png`](reference-screens/logged-in-header-tabs-avatar.png) | 1280×88 RGB | Crop: File menu, Start Page / Settings tabs, `+` tab, ZA avatar. |
| **OA** | [`docs/reference-screens/logged-in-onboarding-act05.png`](reference-screens/logged-in-onboarding-act05.png) | 1280×800 RGB | After official host process: ACT 05 Delegation, "Tell Traycer how to choose", Agent selection guide modal, Skip intro. v1.1.10. |
| **OH** | [`docs/reference-screens/logged-in-onboarding-header.png`](reference-screens/logged-in-onboarding-header.png) | 1280×90 RGB | Crop: XFWM + menu + dark intro header (traycer wordmark + Skip intro / Esc). |
| **S** | [`docs/reference-screens/logged-in-settings.png`](reference-screens/logged-in-settings.png) | 1280×800 RGB | Full `:3` desktop: Settings / General, Start Page + Settings tabs, left nav, ZA avatar. systemd modal gone. |
| **SG** | [`docs/reference-screens/logged-in-settings-general.png`](reference-screens/logged-in-settings-general.png) | 1280×719 RGB | Client window: Settings / General. Same chrome as **S** without XFWM/Plank. Bottom 80 px of this PNG is `#000000` capture padding — not a product footer. |
| **SS** | [`docs/reference-screens/logged-in-settings-sidebar.png`](reference-screens/logged-in-settings-sidebar.png) | 280×631 RGB | Crop of the Settings nav. Origin = top of the selected **General** row. Same 80 px black pad at the bottom. |
| **P** | [`docs/reference-screens/logged-in-act04-providers.png`](reference-screens/logged-in-act04-providers.png) | 1280×800 RGB | Settings → Providers. Codex selected, bundled v0.146.0, Enabled toggle on. |
| **AG** | [`docs/reference-screens/logged-in-act05-agent-selection.png`](reference-screens/logged-in-act05-agent-selection.png) | 1280×800 RGB | Settings → Agent selection. Guide markdown saved. |
| **A** | [`docs/reference-screens/logged-in-act06-appearance.png`](reference-screens/logged-in-act06-appearance.png) | 1280×800 RGB | Settings → Appearance. Theme System + preset **Traycer Green**. UI font **Figtree (Default)**, size **15 px**. |
| **K** | [`docs/reference-screens/logged-in-act06-keybindings.png`](reference-screens/logged-in-act06-keybindings.png) | 1280×800 RGB | Settings → Keybindings. Ctrl+1–9 / Ctrl+N / tab shortcuts. Cmd+K is not in the first screenful. |

**Logged-in Start Page + History (STAR 0134).** Same `:3` session as 0128 (HOME `/workspace/ref-traycer/home-d3`, avatar **ZA**). Method and sha256: `docs/reference-env.md` — `ffmpeg -f x11grab -video_size 1280x800 -i :3 -frames:v 1` plus a precise Start Page tab click at framebuffer `(240, 70)`, then the header clock for History. Taken 2026-08-21 ~11:00 YEKT / 06:00 UTC. Full-desktop 1280×800; XFWM 24 px above the client (same convention as **S**).

| ID | Path | Pixels | What it is |
|---|---|---|---|
| **SP** | [`docs/reference-screens/logged-in-start-page.png`](reference-screens/logged-in-start-page.png) | 1280×800 RGB | Official Start Page *page* (Start Page tab active, Settings tab inactive). Greeting "Good morning" / "What's on your mind?"; composer "Ask Traycer anything. @ mention for context"; update banner; Most recent / Filter / Select over an empty list. ZA. |
| **HY** | [`docs/reference-screens/logged-in-history.png`](reference-screens/logged-in-history.png) | 1280×800 RGB | History modal after the header clock. Title History; focused search "Search by title, repo, branch, or PR"; Most recent / Filter / Select / Refresh; empty state **"No tasks yet"**. Start Page dimmed behind a scrim. Pop-out + close. |

Do not add new screenshots in this change. Do not treat XFWM / Plank / GTK menu chrome as Traycer product tokens — they are host-desktop chrome that happens to surround the official window. Plank icon colors (Chrome green `#229342`, dock shelf `#263742`) are **not** product.

Acts 01–03 of the intro (tasks/sidebar/agents/artifacts, layout/split/terminal, handoff/bubbles) are **not** in this set. File → Settings does not replay them. A Task chat transcript and a ladder / Epic card were not on screen (**HY** is empty; File → Open Epic showed "No Epics yet" and that picker was not committed). Those rows stay **needs live**.

### 1.2 Fallback — docs.traycer.ai (IA only, not live)

Fetched 2026-08-21. HTML of the docs site was scraped for `<img>` / `og:image` / mintcdn product shots. Result: **no product screenshots**. The only raster images are Font Awesome icons and Mintlify-generated OG cards (`backgroundDark=#0e0e10`, `primaryColor=#454545`). Those are *docs-site* tokens. They are **not** Desktop tokens and were not pipetted into §2.

Fallback pages still used for *structure* of surfaces we have not captured live (chat transcript, ladder, History *rows*, panel stack). Start Page *page* and History empty are live (**SP** / **HY**):

- https://docs.traycer.ai — mental model (Task, agents, panels, artifacts)
- https://docs.traycer.ai/quickstart — first-run: folder → Task → agent → inspect
- https://docs.traycer.ai/concepts/tasks-and-workspace-folders — Task tab + canvas tabs
- https://docs.traycer.ai/concepts/history — History rows, search/filter/sort
- https://docs.traycer.ai/panels — sidebar panel list, headers, `+`, rearrange/stack
- https://docs.traycer.ai/panels/agents — Agents tree, Chat vs Terminal, composer controls
- https://docs.traycer.ai/panels/artifacts — Spec/Ticket/Story/Review, status Todo/In Progress/Done
- https://docs.traycer.ai/panels/git-diff — worktree picker, empty/loading/error states
- https://docs.traycer.ai/panels/file-tree — workspace picker, open-in-editor
- https://docs.traycer.ai/panels/terminals — plain shell sessions
- https://docs.traycer.ai/panels/comments — anchored threads, contextual panel
- https://docs.traycer.ai/panels/sharing — Task access (out of scope by ADR for us; listed so the map is complete)
- https://docs.traycer.ai/settings/appearance — Theme system/light/dark, preset, UI/code font size, artifact icon colors (now also **live A**)
- https://docs.traycer.ai/changelog — 1.1.x chrome notes (split, context chip, notification center, redesigned message cards)

Every component that exists only in this column is tagged **fallback docs.traycer.ai** and **needs live**.

### 1.3 How pixels were read

Python 3 + Pillow, no eyedropper GUI. A sample is an exact `Image.getpixel((x, y))` on the PNG. Regions used `Image.crop` + a color counter. Distances are inclusive pixel spans (`max − min + 1`). Corner radius is the left/right inset of the fill on the first/last rows of the control (classic rounded-rect footprint), not a CSS `border-radius` read from asar.

When a color is a single-pixel sample, the coordinate is in the table. When a fill is a plateau (thousands of identical pixels), the table still cites one representative pixel plus the count.

Anti-aliased type is a cloud, not one hex. The spec records the dominant plateau and the AA neighbors, and says so. Grainy dark washes (onboarding header / ACT 05 floor) are clouds — do not flatten them to a hex we did not measure as a plateau.

---

## 2. Design tokens from the screens

Every token cites its source frame. Tokens without a live sample are **not invented**.

### 2.1 Palette

#### Canvas (product, frame **W** — signed-out welcome)

| Token | Hex | Sample | Notes |
|---|---|---|---|
| `bg.canvas` | `#000000` | **W** `(640, 40)`, also `(20, 360)`, `(1260, 360)`, `(640, 80)` | Upper two-thirds of the client. True black, not charcoal. The field is flat `#000000` until the floor glow begins. |
| `bg.canvas.near-glow` | `#041010` | **W** `(640, 420)` | First measurable lift above black as the glow approaches the CTA. Not a fill token — a gradient stop. |
| `fg.logo` | `#FFFFFF` | **W** `(640, 304)` | Logo stroke plateau. Same hex as the heading plateau. |
| `fg.heading` | `#FFFFFF` | **W** heading band `y=382…385` (see §2.2) | Dominant glyph fill. AA neighbors `#FEFEFE`, `#FCFCFC`, `#F7F7F7`, `#F0F0F0` at the same band. Do not average them into a gray heading. |
| `surface.cta` | `#F8F7F2` | **W** `(560, 464)` | Sign-in button fill. 6 484 of 6 492 fill pixels in the button box are this exact hex. Neighbors `#F6F5F0` (3), `#F7F6F1` (2) are AA at the rounded corners. Warm off-white, not `#FFFFFF`. |
| `fg.on-cta` | `#050505` | **W** `(632, 464)` | "Sign in" glyph interior. 35 pixels at this exact hex; AA `#111111`, `#1A1A19`, `#252525`. Not pure `#000000`. |
| `glow.teal-1` | `#173837` | **W** `(640, 500)` | 18 px below the button. |
| `glow.teal-2` | `#1F4A4C` | **W** `(640, 540)` | |
| `glow.teal-3` | `#22555B` | **W** `(640, 580)` | |
| `glow.teal-4` | `#316469` | **W** `(640, 620)` | Last solid-looking stop before the field falls back to black. |
| `glow.teal-hi` | `#7C8E8E` | **W** `(725, 611)` | Brightest glow pixel found (`lum≈138`). Desaturated, not neon cyan. |
| `glow.patch-mean` | `#26595E` | **W** crop `(620,560)–(660,600)` | Mean of a 40×40 patch (802 unique colors). Cite as atmosphere, not a flat fill. |

The floor glow is a **grainy, noisy teal wash**, not a CSS linear-gradient. A 5×5 at `(630,580)` is already five different hexes (`#376B6F`, `#2E6166`, `#23575C`, `#205358`, …). egui cannot reproduce film grain cheaply — see §4.

No hairline border was measured on the welcome canvas. Left/right mid samples **W** `(20, 360)` and `(1260, 360)` are `#000000`. No card, no sidebar, no splitter on this frame.

#### Signed-in light chrome (frames **SG** / **S** / **HT** / **A** / **P** / **AG** / **K** / **HE** / **SP** / **HY**)

The logged-in product is a **light** theme (Appearance → Theme = System on this machine). Surfaces are cool off-whites, not the welcome black and not mint `#0e0e10`.

| Token | Hex | Sample | Notes |
|---|---|---|---|
| `bg.page` | `#F6F9F8` | **SG** `(20, 150)`; **S** `(20, 200)`; **HE** `(20, 200)` (737 943 px on **HE**); **SP** `(640, 200)` (743 613 px on **SP**); **HY** modal `(640, 400)` | Signed-in wash: settings sidebar, page margins, host-error canvas behind the modal, Start Page field, History modal body. Cool gray-green, not `#FFFFFF` and not `#F6F5F4` (that is the GTK menu). Dominant plateau on every logged-in light frame. |
| `bg.content` | `#F9FBFB` | **SG** `(400, 250)`; **A** `(400, 320)`; **K** `(400, 350)` | Settings main column / cards. 1-step lighter than `bg.page`. 344 724 px in the **A** content box. Not the Start Page composer (that is `surface.composer` `#F3F5F4` on **SP**). |
| `bg.header` | `#FFFFFF` | **SG** `(80, 40)`; **HT** `(80, 40)`; **HA** `(80, 30)`; **SP** `(100, 70)` | Tab strip and the right-hand toolbar that holds the avatar. Active tab (Settings on **SG**, Start Page on **SP**) is this same white. |
| `bg.tab.inactive` | `#F6F9F8` | **HT** `(400, 40)`; **SG** `(400, 40)`; **SP** Settings tab `(220, 70)` | Inactive tab fill. Same hex as `bg.page` — the inactive tab is the page wash showing through, not a third surface. On **SP** the *Settings* tab is inactive; on **SG** / **HT** the *Start Page* tab is inactive. |
| `bg.nav.selected` | `#EAEAEA` | **SG** `(80, 90)` (4 850 px on **SS**); **SS** `(80, 10)` | Selected settings-nav pill (General / Appearance / Providers / …). Also the keybinding chip fill (**K**) and the avatar disc (**HA**). One hex, three roles. |
| `fg.primary` | `#0F0F0F` | **SG** `(360, 115)` heading "General"; **SG** `(350, 240)` row title; **A** `(360, 140)` "Appearance" | Near-black. Heading plateau and settings row titles. Not `#000000` and not `#050505` (that is welcome CTA type). |
| `fg.secondary` | `#666666` | **SG** `(400, 180)` page subtitle / descriptions; **HE** modal body (695 px in the card); **K** chip glyphs | Muted gray. AA neighbors `#696969`, `#7E7E7E`. |
| `accent.traycer-green` | `#257174` | **SG** `(1160, 237)` toggle ON (657 px across three toggles); **A** `(1160, 528)` "Use pointer cursors" ON (219 px); **P** `(1160, 260)` Enabled ON (234 px); **P** `(557, 548)` Bundled radio (37 px) | **Traycer Green.** Teal, not lime. RGB `(37, 113, 116)`. This is the ON-toggle track and the selected radio. AA rim `#31797C`, `#277275`, `#347B7E`. |
| `accent.traycer-green.swatch` | `#1A2421` | **A** `(1040, 346)` (311 px) | Dark square behind the "Aa" on the Appearance preset dropdown labeled **Traycer Green**. The letters themselves punch through as `#257174` (sparse, AA). Do not use the swatch fill as the accent — use `accent.traycer-green`. |
| `toggle.off.track` | `#EAEAEA` | **SG** `(1165, 484)` | OFF toggle track. Same hex as `bg.nav.selected`. |
| `toggle.knob` | `#F6F9F8` | **SG** `(1174, 237)` ON knob; **SG** `(1160, 484)` OFF knob | Knob matches `bg.page`, not pure white. |
| `hairline.header` | `#DFE9E7` | **SG** / **HT** full row `y=64` (1 067 px on **SG**) | 1 px seam under the tab strip, before `bg.page`. |
| `hairline.control` | `#DCE7E4` | **A** theme-segment / dropdown edges; **SG** `(358, 27)` cluster | Cool green-gray stroke around segmented controls and inputs. Not a 1 px CSS we can read — a plateau on the control rim. |
| `hairline.sidebar` | `#E6EFEC` | **SG** `(239, 150)` | Faint 1 px between nav column (`x=0…238`) and the page. Sidebar and page are the *same* `#F6F9F8`; the split is this hairline, not a fill change. |
| `surface.input` | `#FFFFFF` | **A** `(1040, 350)` preset dropdown; **A** `(1100, 650)` Figtree dropdown; **AG** editor | Raised white on `bg.content`. |
| `surface.segment.idle` | `#F5F6F6` | **A** `(980, 268)` (4 003 px in the theme control) | Unselected Light / Dark segments. |
| `surface.segment.active` | `#FFFFFF` | **A** `(1140, 268)` System segment, ~87×26 | Selected segment. Soft `#DCE7E4` halo, not a drop-shadow plate. |
| `chip.keybinding` | `#EAEAEA` | **K** `(1160, 230)` | Shortcut chip fill. Same hex as selected nav. Halo `#EDF2F1` / `#E2E9E7`. |
| `editor.current-line` | `#F1FAFF` | **AG** `(800, 320)` (13 232 px) | Agent-selection guide: current-line wash. Cool blue, not Traycer Green. |
| `editor.loading-line` | `#F8FDFF` | **OA** `(800, 277)` (10 346 px) | Onboarding guide while "Loading…". Same family as `editor.current-line`, one step lighter. |

#### Host-error modal (frame **HE**)

| Token | Hex | Sample | Notes |
|---|---|---|---|
| `modal.fill` | `#FFFFFF` | **HE** `(640, 450)` | Card ~420×226 at `(430, 301)–(849, 526)`. |
| `modal.edge` | `#DCDFDE` | **HE** `(640, 300)` | 1 px gray bar on the top edge before the white fill. Side hairline `#D6D8D7` at `(429, 400)`. |
| `modal.radius` | 8–10 px | Top row `y=301`: fill `x=439…840` vs full `x=430…849` (inset 9 L / 9 R). Then 6, 4, 3, 0. | Rounded rect, not a pill. |
| `fg.modal-body` | `#666666` | **HE** modal interior (695 px) | systemd error copy. Centered. |
| `fg.modal-action` | `#0F0F0F` | **HE** `(576, 479)` | "Retry" glyph plateau (25 px). |
| `surface.retry` | `#F6F9F8` | **HE** `(580, 479)` | Retry is a ghost/secondary on the *page* wash, with `#DCE7E4` halo. Not `surface.cta` cream. |
| `fg.report` | `#666666` | **HE** `(654, 479)` | "Report issue" next to Retry. |

No Traycer Green on **HE**. No danger-red banner — the host error is a centered white card on `bg.page`, not a `#5C1C1C` strip. Current `rt-gui` offline banner `#5C1C1C` remains *our* color, not a pipetted Traycer token.

#### Onboarding ACT 05 (frames **OA** / **OH**)

These fills are **clouds**, not plateaus. Cite as atmosphere.

| Token | Hex (dominant) | Sample | Notes |
|---|---|---|---|
| `onboarding.header` | `#0E1A17` / `#0E1B18` | **OH** `(640, 70)`; **OA** `(640, 70)` | Dark intro bar from **OH** `y=51`. Neighbors `#0D1A17`, `#0D1916`, `#0E1916` (thousands of px each). Not `#000000`. Grainy teal-black, same class as the welcome floor but much darker. |
| `onboarding.fg` | `#FFFFFF` | **OH** `(640, 72)`, `(640, 76)`, `(640, 81)` | Wordmark "traycer" + "Skip intro". |
| `onboarding.canvas` | `#0B1513` / `#262E2B` | **OA** `(20, 200)` `#0B1613`; left copy crop dominated by `#262E2B` (8 919), `#252D2A` (7 387), `#272F2C` (6 105) | Grainy dark field behind the guide modal. A 5×5 is already several hexes. Not a flat fill. |
| `onboarding.scrim` | `#7F7F7F` / `#7B7C7C` | **OA** mid-frame (24 368 / 18 307 px) | Blurred settings-like backdrop around the white modal. AA gray cloud. |
| `onboarding.modal` | `#FFFFFF` | **OA** `(800, 400)` | Guide card ~677 px wide at `x=464…1140`, `y≈202…554`. Title `#0F0F0F` at `(480, 217)`; subtitle `#666666`. |
| `onboarding.continue` | `#DEDFDF` | **OA** `(1180, 680)` | Continue on the dark footer. Cool gray, **not** welcome cream `#F8F7F2`. Arrow glyph `#000000` at `(1187, 670)–(1201, 684)` (109 px). This frame is a hover (hand cursor). |
| `onboarding.progress.on` | `#FFFFFF` | **OA** crop `(20, 90)–(300, 130)` (568 px) | Lit segments of the 7-step bar (ACT 05 ⇒ first five lit). Rest of that crop is the canvas cloud. |

Footer version "v1.1.10" sits on the same dark cloud (`#0B1513` at **OA** `y=720…748`). Do not treat Plank / XFWM pixels on **OA** as product.

#### Start Page (frame **SP**)

Same light chrome as Settings. New tokens are the update banner, the composer plate, and a few type colors that sit on those plates. Greeting / subtitle reuse `fg.primary` / `fg.secondary`.

| Token | Hex | Sample | Notes |
|---|---|---|---|
| `banner.update` | `#DDF0F6` | **SP** `(640, 120)` (24 126 px on the full frame; 15 197 / 16 320 = 93.1% of crop `(320, 104)–(830, 136)`) | Update-installed strip under the tab seam. Cool ice-blue, not Traycer Green. This machine's session showed "Update installed — restart host to finish." — a live *state*, not permanent chrome. |
| `banner.update.edge` | `#9ADAF5` | **SP** `(640, 100)`; also `(640, 142)` | 1 px top and bottom of the banner (`y=100` and `y=142`). Mid-row fill is `banner.update`; the edge is one step brighter. |
| `fg.banner` | `#052F4A` | **SP** `(433, 120)` (91 px at this exact hex in `(317, 118)–(563, 126)`) | Banner body type. Dark teal, **not** `fg.primary` `#0F0F0F`. AA neighbors `#08324C`, `#0B354F`, `#06304B`. |
| `fg.on-accent` | `#FFFFFF` | **SP** `(853, 120)` (62 px in `(853, 115)–(922, 124)`) | "Restart host" glyph on the `#257174` button. Sparse; AA into the teal. |
| `surface.composer` | `#F3F5F4` | **SP** `(640, 500)` (59 643 px; box `(303, 474)–(975, 567)`) | Composer plate. Cooler and one step darker than `bg.page`, not `bg.content` `#F9FBFB` and not `#FFFFFF`. |
| `hairline.composer` | `#B4CDCD` | **SP** `(302, 500)`; `(976, 500)`; `(640, 473)`; `(640, 568)` (1 484 px) | 1 px outline around the composer. Teal-gray, not `hairline.control` `#DCE7E4`. |
| `surface.send` | `#EEEFEE` | **SP** `(950, 540)` (513 px; disc `(938, 530)–(964, 557)`) | Send circle on the composer. Ghost — almost the plate, not `accent.traycer-green`. Circular footprint. |
| `fg.placeholder` | `#9E9F9F` (dominant) | **SP** `(317, 494)` (148 px); bbox `(317, 494)–(613, 507)` | "Ask Traycer anything. @ mention for context". **AA cloud**, not a plateau. Neighbors `#A1A2A1` (40), `#B9BAB9` (40), `#B5B6B6` (38), `#A5A6A5` (36). Do not flatten to one hex. |

Restart host is `accent.traycer-green` `#257174` — same hex as the Settings ON toggle: **SP** `(860, 120)`, box `(842, 109)–(932, 133)` (1 848 px). AA rim `#3E8184`, `#84AFB1`, `#699D9F`. No new accent.

"Switch to Terminal" above the composer and "cursor" / "Add folder" below it are **AA clouds** on `bg.page`. A few `#666666` pixels sit in the switch (**SP** `(374, 448)`, 12 px) and the folder cluster is a gray mist (`#909191` 38 px in `(309, 587)–(548, 598)`). Composition only — do not law a new muted hex.

Most recent / Filter / Select on the empty list: `#666666` 101 px at **SP** `(682, 655)` in `(682, 655)–(840, 666)` (broader dark cloud to `(961, 667)`). Same `fg.secondary`. No row cards — the list under the composer is empty `bg.page`.

#### History empty (frame **HY**)

The modal is **not** the host-error white card. Body is `bg.page` `#F6F9F8`. Header bar is `bg.nav.selected` `#EAEAEA`. Title and empty copy sit on `fg.primary`. Search is focused on this frame (teal ring).

| Token | Hex | Sample | Notes |
|---|---|---|---|
| `history.header` | `#EAEAEA` | **HY** `(640, 140)` (full rows `y=129…160` at `x=128…1151`, 1 024 px each) | Modal title bar. Same hex as selected-nav / keybinding chip / avatar disc. |
| `history.header-seam` | `#E4E9E7` then `#F0F4F2` | **HY** full row `y=161` `#E4E9E7` (1 024 px); `y=162` `#F0F4F2` (1 024 px) | Two 1 px blends under the header before `bg.page`. Not `hairline.header` `#DFE9E7`. Do not collapse them into one token. |
| `history.search.ring` | `#8EB5B6` (dominant) | **HY** `(640, 184)` (2 790 px); bbox `(307, 183)–(972, 217)` | Focused search outline. **Cloud**, not a 1 px CSS stroke: `#A7C5C6` at `y=182`, `#4A898B` at `y=185`, `#CBDDDD` at `y=186`, `#4F8D8F` at `y=214`, `#8CB3B5` at `y=215`, `#DBE7E7` at `y=218`. Inner sides also punch 36 px of `accent.traycer-green` `#257174` at **HY** `(310, 191)` and `(969, 191)` — AA of the same ring, not a caret fill. |
| `history.edge` | `#919291` | **HY** `(127, 400)`; `(1152, 400)` | 1 px immediately outside the `#F6F9F8` body. Neighbors fade into the scrim (`#9FA1A0`, `#A0A2A1`). |
| `history.scrim` | `#ACAEAD` (dominant) | **HY** `(20, 400)` (33 200 / 48 000 = 69.2% of a 120×400 side strip) | Dim overlay over Start Page. **Cloud**: `#ABADAC`, `#A9ABAA`, `#AAACAB`, `#A7A9A8`. Over the white tab strip **HY** `(640, 70)` is `#B2B2B2` (SP was `#FFFFFF`); over the banner **HY** `(640, 110)` is `#98A5AA` (SP was `#DDF0F6`). Not a flat hex. Same class as `onboarding.scrim`. |

Search field *interior* is `bg.page` `#F6F9F8` (**HY** `(640, 200)`). Placeholder / magnifier glyphs: `#666666` 79 px in `(320, 194)–(539, 206)` plus AA (`#696969` 34, `#9FA1A0` 24). Toolbar "Most recent / Filter / Select": `#666666` 111 px at **HY** `(682, 233)` in `(682, 233)–(962, 245)`. Empty **"No tasks yet"**: **AA cloud** at **HY** `(605, 323)–(675, 335)` — only 40 px are exact `#0F0F0F`; neighbors `#131313` (13), `#3C3C3C` (8), `#3D3E3E` (7). Cite `fg.primary` as the intended face; do not invent a third gray.

Close / pop-out on the header right are an AA dark cluster at **HY** `(1087, 135)–(1127, 146)` (`#464646` 12, `#0F0F0F` 10, `#171717` 10). Placement live; no isolated icon-fill token.

#### Native host chrome (frames **W** / **D** / logged-in full-desktop — do not copy into `rt-gui`)

These are the Linux window manager and Electron menu, not Traycer widgets.

| Token | Hex | Sample | What it is |
|---|---|---|---|
| `host.menu-strip` | `#F6F5F4` | **W** `(640, 10)`; **SG** `(640, 10)`; full rows `y=0…26` on client dumps | Electron/GTK menu. 27 px tall. On **W** the canvas seam is `y=26` `#F6F5F4` → `y=27` `#000000`. On **SG** the seam is `y=26` → `y=27` `#FFFFFF` (tab strip). |
| `host.menu-label` | `#2E3436` | **W** / **SG** / **HT** menu-dark cluster (106 px on **HT**) | "File Edit View Window Help". System UI, ~11 px cap. |
| `host.xfwm-title` | `#D8D5D2` | **D** `(640, 8)`; **S** / **A** / **HE** `(640, 8)` | XFWM title bar fill. Top row `(640, 0)` is `#D9D6D3`. Height 24 px (`y=0…23`); client begins at framebuffer `y=24`. |
| `host.xfwm-title-text` | `#2E3436` | **D** title-dark bbox `(581, 6)–(630, 17)`, 83 px | The word "Traycer" on the title bar. |
| `host.plank` | `#263742` | **D** `(640, 760)`; **S** / **A** `(640, 760)` | Plank dock shelf. Not product. |

`rt-gui` already draws its own 40 px `TopBottomPanel` (`chrome.rs`). Do not restyle that panel to match XFWM Adwaita. Match the *client canvas*, not the window manager.

#### Not measured (no live frame)

| Token | Why missing | Until |
|---|---|---|
| Chat bubble user vs assistant fills | Acts 01–03 / a Task transcript were not captured | **needs live** (Acts 01–03) |
| Task-thread composer, mention hover, context chip, permission chip | Start Page composer (empty) is live **SP**; a Task thread was not | **needs live** (Acts 01–03) for the Task thread |
| History *rows* / loading / no-match | **HY** is the empty modal only | **needs live** (populated list) |
| Canvas sidebar / panel stack / agent row in a Task | Behind onboarding + Settings; not on **SP** / **HY** | **needs live** (Acts 01–03) |
| Status: ticket/story Todo / In Progress / Done | **fallback docs.traycer.ai** `/panels/artifacts` names the three values only | **needs live** |
| Light-theme *welcome* (signed-out is dark) | Landing is dark | Dark welcome remains law for the gate |
| Dedicated light *vs* dark Appearance preview of the whole app | **A** shows System + the picker; we did not flip to Light or Dark | **needs live** for a forced-light / forced-dark chrome pass |
| Scrollbar thumb / track | Nothing we isolated as a thumb | **needs live** |
| Tooltip fill | Not shown | **needs live** |
| Ladder / approval-card scrim | Not shown | **needs live** |
| Cmd+K / full keybinding list below the first screenful | **K** is one screenful | **needs live** (scroll) |

Do not borrow Mintlify `backgroundDark=#0e0e10` or `primaryColor=#454545` as stand-ins. Do not borrow Plank Chrome-icon green as Traycer Green.

### 2.2 Typography

**Family (recommendation only).** The heading and button on **W** are a geometric neo-grotesque. We did not identify the official face from asar (forbidden). Implement with an Inter-class OFL font. Do not vendor Traycer fonts. Do not use egui's default Proggy-like debug face for product chrome.

**Live setting (frame **A**).** Appearance → Typography shows **UI font = Figtree (Default)** in a white dropdown at **A** `(1021, 638)–(1183, 667)` (~163×30), and a size control under it whose digit plateau is `#0F0F0F` at **A** `(1127, 686)–(1137, 695)`. The control reads **15 px**. That is a *product default we can see*, not a CSS `font-size` from asar. Cite **Figtree 15** as the official UI size to match; ship an Inter-class OFL stand-in at 15 px. Do not vendor Figtree from the AppImage.

Settings row titles on **SG** (e.g. "Voice input") have an `#0F0F0F` plateau about 9–11 px tall (`y=236…244`, `y=313…323`) — consistent with a 15 px face plus AA, not with the welcome 30 px display.

Appearance docs (**fallback docs.traycer.ai** `/settings/appearance`, now also **live A**) expose three user sizes: **UI font size**, **code font size**, and (changelog) a separate **terminal** size. Code / terminal defaults were **not** on the first screenful of **A**. `rt-gui` should keep one UI face + one mono face, with a single scale factor.

| Role | Measured box | Inferred size / weight | Sample | Confidence |
|---|---|---|---|---|
| Display heading "Welcome to Traycer" | **W** lum>200 bbox `(462, 374)–(818, 410)` = **357×37** px including AA. Dense white rows `y=382…385` and `y=397…401`. | **28–32 px, weight 600–700**, line-height ≈ 1.15. Advance ≈ 357 / 18 glyphs ≈ 20 px, which fits Inter Bold ~32 more than ~24. | **W** `(640, 384)` `#F0F0F0` (counter/AA through the center column); plateau `#FFFFFF` in the same band. | Size is inferred from a raster, not a CSS `font-size`. Say **~30 px Bold** in implementation notes; ±2 px is not a bug. |
| Primary CTA "Sign in" | Dark-glyph rows **W** `y=461…467` on a 35 px button. | **13–14 px, weight 500–600**. Center column is `#050505` at those rows. | **W** `(632, 464)` `#050505` | Medium, not Bold. Vertical pad ≈ 10–11 px each side of a ~14 px em. |
| Settings page title ("General", "Appearance", …) | **SG** `#0F0F0F` "General" `y=107…127`, `x=307…401` ≈ **95×21**. **A** "Appearance" `y=131…151`, `x=306…458`. | **22–26 px, weight 600–700**. | **SG** `(360, 115)` `#0F0F0F` | Raster inference. Say **~24 px Semibold**. |
| Settings row title / nav label / keybinding command | **SG** row-title plateau ~9–11 px; **SS** selected "General" `#0F0F0F` at `(32, 2)–(93, 84)` (23 px, AA cloud). | **15 px Regular / Medium** — matches the Appearance control. | **SG** `(350, 240)` `#0F0F0F`; **A** size digits `(1130, 690)` `#0F0F0F` | **Figtree 15** is the live default. |
| Settings description / subtitle | **SG** `#666666` under the page title and under each row. | **13–15 px Regular**. | **SG** `(400, 180)` `#666666` | Same face, muted. |
| Keybinding chip | **K** `#666666` / `#848484` inside `#EAEAEA` chips. | **12–13 px Regular**. | **K** chip cluster | Slightly smaller than the command label. |
| Native menu labels | Cap ~10–11 px, **W** / **SG** `y=7…17` | System UI. Ignore for product type. | `#2E3436` | Not a product token. |
| Avatar initials "ZA" | **HA** disc `(1246, 36)–(1264, 55)`; glyph plateau `#666666` (14 px). AA `#696969`, `#747474`, `#A0A0A0`. | **10–12 px Semibold** on a 19×20 disc. | **HA** `(1255, 46)` `#666666` | Small. |
| Start Page greeting "Good morning" | **SP** `#0F0F0F` bbox `(519, 342)–(757, 374)` = **239×33** (1 318 px at the exact hex). | **28–32 px, weight 600–700**. Advance ≈ 239 / 12 glyphs ≈ 20 px — same class as the welcome display, on `fg.primary` not white. | **SP** `(640, 352)` `#0F0F0F` | Raster inference. Say **~30 px Semibold**. AA neighbors `#171717`, `#141414`, `#121212`. |
| Start Page subtitle "What's on your mind?" | **SP** mid-gray bbox `(566, 390)–(711, 403)` = **146×14**. 201 px `#666666`. | **13–15 px Regular**. | **SP** `(640, 400)` `#666666` | Same as settings description. AA `#696969`, `#6C6C6C`, `#6D6D6D`. |
| Banner body / Restart label | **SP** `#052F4A` `y=118…126`; Restart `#FFFFFF` `y=115…124` on `#257174`. | **13–15 px Regular / Medium**. | **SP** `(433, 120)` `#052F4A`; `(853, 120)` `#FFFFFF` | Small plateaus (91 / 62 px). |
| History title | **HY** `#0F0F0F` `(167, 134)–(213, 147)` (72 px at the exact hex). | **15 px Medium / Semibold** — settings-row class, not the Start Page greeting. | **HY** `(167, 140)` `#0F0F0F` | On `history.header` `#EAEAEA`. |
| History empty "No tasks yet" | **HY** AA cloud `(605, 323)–(675, 335)`. 40 px `#0F0F0F`. | **15 px Regular / Medium**. | **HY** `(605, 323)` `#0F0F0F` | Honest AA. Do not treat 40 px as a heading plateau. |
| Body / chat / chips in a Task | — | Unknown. | — | **needs live** (Acts 01–03). Until then: 15 px Regular UI (now measured), 12 px mono for code/PTY, 12 px Regular for secondary. Mark chat uses as assumed. |

Line-height was not a CSS property we could read. Measured welcome heading em-box including AA is 37 px on a ~30 px face → line-height ≈ 1.2. Settings titles sit tighter. Use 1.2–1.35 for body once a live transcript exists.

### 2.3 Spacing, radii, borders, shadows

Measured on **W**, client origin `(0, 0)` = top-left of the 1280×719 window (menu included). Logged-in client dumps (**SG**, **HT**, **HA**) use the same origin. Full-desktop frames add 24 px of XFWM above the client.

| Token | Value | How it was measured |
|---|---|---|
| `space.menu` | 27 px | Menu fill `y=0…26`; product starts `y=27`. Native, not product. |
| `logo.box` | 79×81 px at `(601, 264)–(679, 344)` | Bright (lum>180) span of the mark on **W**. Centered: cx = 640. |
| `space.logo → heading` | **30 px** | Logo last bright row `y=344` → heading first lum>200 row `y=374`. `374 − 344 − 1 = 29`; visual gap reads as 30. |
| `space.heading → cta` | **44 px** | Heading last lum>200 row `y=402` (ignore the 4 px "r"/"r" AA speckles at `y=403…410`) → button top `y=447`. `447 − 402 − 1 = 44`. |
| `cta.box` | **193×35 px** at `(544, 447)–(736, 481)` | Long run of `#F8F7F2` on **W**. Mid-row `y=464`: `x=544…736`. |
| `cta.radius` | **4 px** | Top row `y=447`: inset 4 L / 4 R. Then 2, 1, 1, 0. Bottom row `y=481`: inset 4. Symmetric rounded rect, not a pill (`35/2 = 17.5` would be a pill). |
| `cta.shadow` | **none** | Halo pixels immediately outside the fill are glow texture (`#0C1F1F`, `#091C1D` at the top corners), not a drop-shadow plate. No darker duplicated rect offset down-right. |
| `cta.border` | **none** | Fill only. No 1 px stroke of a different hex. |
| `layout.center` | Content mid `y ≈ 372.5` vs client mid (after menu) `373.5` | The logo+heading+button cluster is vertically centered in the *canvas*, not in the full window including the menu. Horizontally centered on 640. |
| `chrome.tabs` | 37 px | **SG** / **HT** tab strip `y=27…63`. |
| `chrome.header-seam` | 1 px `#DFE9E7` | **SG** / **HT** `y=64`. |
| `nav.width` | **240 px** | **SG** selected pill `x=15…223` on a column that ends at the `#E6EFEC` hairline `x=239`. Content title starts `x=307` (≈ 68 px pad). |
| `nav.row` | **33×209 px** pill | **SG** selected General `y=80…112`, `x=15…223`. **SS** same pill `y=0…24` (crop starts on the row), span 209, bottom inset 5 ⇒ radius **~6 px**. |
| `nav.row.pitch` | ~38–40 px | Ten items (General … Diagnostics) stacked under the header; **SS** shows icons every ~36–40 px. |
| `toggle.on` | track **21×17** `#257174` + knob `~13` px `#F6F9F8` | **SG** three ON toggles at `y=229…245`, `305…321`, `381…397` (pitch 76), `x=1156…1176` green / knob to `x≈1182`. Full control ≈ **32×17**. |
| `toggle.off` | track `#EAEAEA` + knob `#F6F9F8` (knob on the left) | **SG** `(1165, 484)` band `y=476…492`. Same size class as ON. |
| `avatar.disc` | **19×20 px** | **HA** `#EAEAEA` `(1246, 36)–(1264, 55)`. Circular footprint (1 px at the poles, 19 at the equator). |
| `theme.segment.active` | **87×26 px** | **A** System `#FFFFFF` `(1095, 256)–(1181, 281)`. |
| `theme.swatch` | **20×21 px** | **A** `#1A2421` `(1032, 336)–(1051, 356)`. |
| `modal.host-error` | **420×226**, radius 8–10 | **HE** `(430, 301)–(849, 526)`. |
| `onboarding.modal` | ~677 px wide | **OA** `x=464…1140`. |
| `banner.update` | **673×43**, radius ~5–6 | **SP** fill `#DDF0F6` `x=303…975`, `y=101…141`; edge `#9ADAF5` at `y=100` / `y=142`. Top row `y=100`: `x=308…970` (inset 5). Same width as the composer. |
| `banner.restart` | **91×25**, radius ~3 | **SP** `#257174` `(842, 109)–(932, 133)`. Top row `y=109`: `x=845…929` (inset 3 L / 3 R). Then 1, 0. |
| `composer.box` | **673×94**, radius ~6 | **SP** `#F3F5F4` `(303, 474)–(975, 567)`. Top row `y=474`: `x=309…969` (inset 6). Bottom `y=567`: inset 6. |
| `composer.send` | **~27×28** disc | **SP** `#EEEFEE` `(938, 530)–(964, 557)`. Circular (1–2 px at the poles, 26 at the equator). |
| `history.modal` | **1024×553**, radius ~8 | **HY** `(128, 121)–(1151, 673)`. Header `#EAEAEA` `y=121…160` (40 px). Body `#F6F9F8` `y=163…664`. Top row `y=121`: fill `x=134…1145` (inset 6). Bottom `y=673`: `x=135…1144` (inset 7). Same 8–10 class as `modal.host-error`. |
| `history.search` | **~666×35**, radius ~4–6 | **HY** ring bbox `(307, 183)–(972, 217)`. Interior `#F6F9F8`. |

Inferred spacing scale for the rest of the app (now with a second surface — still a ladder, not law for every gap):

`4 / 8 / 12 / 16 / 24 / 32 / 48`

- 4 = welcome CTA radius.
- 6–8 = nav-pill / modal radius we actually saw after login.
- 8 / 12 = typical egui inner margin (current chrome already uses 8–12).
- 16 / 24 / 32 / 48 = page title pad (`x=307`) and the measured inter-block gaps (30 and 44 sit between 24–32 and 32–48). Toggle pitch 76 is 48+32.

**Do not** invent a Task sidebar width or a Task-thread composer height. Those **need live** (Acts 01–03). Settings nav width 240 is law for *Settings*, not automatically for the Task Agents list. The Start Page composer height 94 and History modal 1024×553 are law for *those* surfaces only.

### 2.4 Motion and material (from what the frame can prove)

| Effect | On the frames | egui target |
|---|---|---|
| Floor glow (**W**) | Grainy teal, no CSS we can read | Translucent flat wash `rgba(38, 89, 94, ≈0.35)` over `#000000`, or omit. **Accepted deviation** (§4). |
| Onboarding dark wash (**OA** / **OH**) | Grainy teal-black cloud + a diagonal prism mentioned in the capture notes | Flat `#0E1A17` header + `#0B1513` canvas, or omit the prism. **Accepted deviation**. |
| CTA welcome | Static default. Hover/focus/active/disabled not captured | Default = `surface.cta` + `fg.on-cta`. Hover/focus **needs live**; until then, 8–12% lighter fill on hover and a 1 px `#F8F7F2` focus ring on a dark field. Mark as assumed. |
| Continue on **OA** | Hover captured (hand cursor); fill `#DEDFDF` | Default vs hover were not both captured. Do not treat `#DEDFDF` as the welcome cream. |
| Toggle | ON / OFF both live | ON = `accent.traycer-green` + `toggle.knob`. OFF = `toggle.off.track` + `toggle.knob`. |
| Animation | None on a still frame | No entrance animation required for welcome, settings, Start Page, or History parity. |
| History scrim (**HY**) | Dim cloud `#ACAEAD` over Start Page | Flat translucent gray, or omit. Same accepted limit as item 2 in §4 — not a new deviation. |

---

## 3. Component inventory

Column "egui" names the existing `rt-gui` surface where one exists as of 0126, or the egui primitive to use. This is a mapping, not an implementation.

Legend: **live \<ID\>** = pipetted. **fallback docs.traycer.ai** = IA only. **needs live** = do not ship a "we matched Traycer" claim for hex on that row. **needs live (Acts 01–03)** = the intro acts were not on screen; File → Settings does not replay them.

### 3.1 Welcome / sign-in

| Traycer element | State on frame | egui | Notes |
|---|---|---|---|
| Full-window dark canvas | default | `CentralPanel` + `bg.canvas` `#000000` | **live W** |
| Brand mark | default, white, ~79×81 | Do **not** copy the Traycer mark. Own wordmark or a Lucide-class icon, `#FFFFFF` | **live W** for color/size only |
| Display heading | default | `RichText` 28–32 px Strong, `#FFFFFF`, centered | **live W** |
| Primary CTA "Sign in" | default only | `egui::Button` 193×35, fill `#F8F7F2`, fg `#050505`, rounding 4 | **live W**. Hover / focus / active / disabled **needs live** |
| Email / password field | absent | — | Official 1.1.10 welcome is one button (browser PKCE). Do not add fields to "match". |
| Floor glow | default | Optional wash; see §4 | **live W** |
| Native menu File/Edit/… | host chrome | Not an egui widget. eframe/winit decorations stay ours | **live W**, not product |

Empty / error / loading of sign-in were not captured. Changelog says a failed sign-in stays visible and that sign-in completes when the browser approves — **fallback docs.traycer.ai** `/changelog`. After login the next painted surfaces are **HE** then **OA**, not a second welcome.

### 3.2 App chrome (header, tabs, avatar)

| Traycer element | Source | egui (current → target) | States |
|---|---|---|---|
| Title / window controls | **D** / **S** is XFWM, not product | eframe native decorations. Do not fake traffic lights | n/a |
| Native File/Edit/View/Window/Help | **HT** / **HA** / **SG** `#F6F5F4` | Not an egui widget | **live**, not product |
| Back / forward + home/layers | **HT** left of the tabs | Small icon buttons. Inactive arrows read light gray | **live HT** for placement; hover **needs live** |
| Start Page tab | **SP** active `#FFFFFF` `(100, 70)`; **HT** / **SG** inactive `#F6F9F8` | `egui::ScrollArea` horizontal + selectable label | active **live SP**; inactive **live HT/SG**. Contents: §3.13 **live SP**. |
| Settings tab | **HT** / **SG** active `#FFFFFF`; **SP** inactive `#F6F9F8` `(220, 70)`, gear + "Settings" + close × | Same tab strip. Active tab is white and joins `bg.page` under `hairline.header` | selected **live SG**; inactive **live SP** |
| `+` new tab | **HT** / **SP** to the right of the tabs | Small `Button` | default **live** |
| Utility cluster (gauge, overflow, gear, history, bell) | **HT** / **SG** / **SP** right side. Clock opens **HY**. | Icon row. Lucide-class stand-ins | placement **live**; tooltips **needs live** |
| Avatar | **HA** `(1246, 36)–(1264, 55)` `#EAEAEA` disc, initials `#666666`; **SP** `(1255, 70)` `#EAEAEA` / `(1260, 70)` `#666666` | Circle `32` logical? No — **19×20 px** on this 1280 frame. Initials, not a photo. | default **live**. Menu **needs live** |
| Task tabs (outer) beyond Start Page / Settings | **fallback** `/concepts/tasks-and-workspace-folders` | Today: `chrome.rs` `TopBottomPanel` 40 px with Tasks / Canvas / Host. Target: the live tab strip (white active, page-wash inactive, 37 px), not three nav buttons forever | overflow / drag **needs live** |
| Canvas tabs / tiles (inner) | **fallback** + changelog split | `screens/canvas.rs` panes. Divider = `egui::Resize` / split. Drag-to-edge tiling is C63 (later) | **needs live (Acts 01–03)** |
| History | **live HY** empty modal; **fallback** `/concepts/history` for *rows* | `screens/tasks.rs` list + `egui::Window` / card | empty **live HY**. loading / no-match / populated rows **needs live**. See §3.14. |
| Start / home (folder first) | **live SP** | Tasks empty states already in `gui-ia-v0.md`. Start Page *page* is §3.13 | empty list **live SP**. no-host **live HE**. no-workspace **needs live** |

### 3.3 Settings shell (nav + pages)

Live. File → Settings opens the Settings tab; it does **not** replay onboarding Acts 01–06.

| Traycer element | Source | egui | Notes |
|---|---|---|---|
| Settings nav column | **SS** / **SG** / **S** | `egui::SidePanel::left`, width 240, fill `bg.page` | Items, in order, each with a Lucide-class leading icon: **General**, **Appearance**, **Providers**, **Notifications**, **Agent selection**, **Keybindings**, **Shell**, **Worktrees**, **Host**, **Diagnostics**. |
| Selected nav row | **SG** `y=80…112` / **SS** top pill | Rounded rect `#EAEAEA`, ~209×33, radius ~6, icon + `#0F0F0F` label | **live**. Hover wash not isolated (some frames show a hand cursor on the already-selected row). |
| Page title + subtitle | **SG** "General" / "App behavior…"; **A** "Appearance" / "Theme, typography…"; **P** "Providers"; **AG** "Agent selection"; **K** "Keybindings" | `#0F0F0F` ~24 px Semibold + `#666666` 13–15 px | **live** |
| Settings row (title, description, control) | **SG** Chat & composer / Running agents | `ui.horizontal` + two-line label + toggle on the right | Voice input / Quote reply / Steer with Ctrl+Enter = ON; Pin context usage / Prevent sleep = OFF. **live** |
| Toggle | **SG** / **A** / **P** | Custom or `egui::Switch` restyled | ON `accent.traycer-green` `#257174`, OFF `#EAEAEA`, knob `#F6F9F8`, ~32×17. **live** |
| Notifications / Shell / Worktrees / Host / Diagnostics pages | Nav labels only | — | **needs live** (nav is live; the pages were not opened) |

### 3.4 Appearance (act 06 theme)

| Traycer element | Source | egui | Notes |
|---|---|---|---|
| Theme segmented Light / Dark / System | **A** | Three-segment control. Idle `#F5F6F6`, active `#FFFFFF` ~87×26, rim `#DCE7E4` | System selected. Light and Dark *pages* **needs live**. |
| Preset dropdown | **A** | Combo labeled **Traycer Green** with 20×21 `#1A2421` swatch and "Aa" in `#257174` | **live** for the named preset. Other presets **needs live**. |
| Zoom 100% + Reset | **A** | Dropdown + ghost button | Values visible; hex of Reset **needs a tighter crop** — do not invent. |
| Use pointer cursors | **A** toggle ON `#257174` at `(1160, 528)` | Same toggle token | **live** |
| UI font | **A** **Figtree (Default)** + **15 px** | Dropdown + numeric stepper. Digit plateau `#0F0F0F` at `(1127, 686)–(1137, 695)` | **live** as a *setting*. Ship Inter-class OFL at 15 px. Do not vendor Figtree. |
| Code font / terminal size / artifact icon colors | docs + below-the-fold | — | **needs live** (not on the first screenful) |

### 3.5 Providers (act 04)

| Traycer element | Source | egui | Notes |
|---|---|---|---|
| Provider list | **P** | Narrow column of rows; selected **Codex** `#EAEAEA` at `y≈260` | Also visible: Claude Code, OpenCode, Traycer Inference, OpenRouter, Droid, Cursor, Copilot, Grok, Kiro, Kilo Code, Kimi. Icons are brand marks — **do not copy**. Lucide-class stand-ins. |
| Provider detail | **P** | Title + one-line description + Enabled toggle | Codex / "OpenAI's Codex CLI." Toggle ON `#257174`. |
| Profiles card | **P** | "+ Add profile", refresh, profile row, Sign in, Manage profile | A warm-red AA cluster `#D97757` at **P** `(332, 290)` sits in this column — likely an icon glyph, **not** a confirmed status-dot plateau. Do not invent a danger token from it. |
| Path / Bundled radio | **P** | Radio `#257174` at `(554, 545)–(560, 551)` (7×7), label bundled **v0.146.0** | **live** |
| CLI arguments | **P** | `TextEdit` with `--full-auto` | Field fill not isolated from `bg.content`. |
| "Checked just now" + refresh + workspace filter | **P** header | Status + icon + dropdown ("cursor") | Placement **live**. |

### 3.6 Agent selection + guide (act 05 settings + onboarding)

| Traycer element | Source | egui | Notes |
|---|---|---|---|
| Settings → Agent selection page | **AG** | Page title + subtitle + card "Agent selection guide" | Subtitle: "How Traycer picks a coding agent… This does not manage the agents inside a Task." **live** |
| Guide editor | **AG** / **OA** | Mono `TextEdit` with line numbers. Current line `#F1FAFF` (**AG**); loading line `#F8FDFF` (**OA** "Loading…") | **live** for those two states. |
| Revert to default | **AG** / **OA** | Ghost text button `#666666` | **live** for color |
| Saved | **AG** | Label + tiny check. Check is a **brighter mid-green AA cloud** around `#479957` at `(1145, 669)–(1150, 674)` (a handful of pixels) — **not** `accent.traycer-green` `#257174` | Honest: too small to law a second green. Prefer a Lucide-class check in `accent.traycer-green` or `#479957`; ± hue is not a bug. |
| Onboarding ACT 05 modal | **OA** | Same guide, over a dark intro. Title "Agent selection guide" `#0F0F0F` | **live OA**. Acts 01–04 and 06 of the *intro* **needs live**. |
| Skip intro / Esc | **OH** / **OA** | Text + keycap on `onboarding.header` | **live** for placement/color (`#FFFFFF` on the dark bar). Keycap fill is a slightly lighter cloud, not a single hex. |
| Back / Continue | **OA** | Continue `#DEDFDF` + `#000000` arrow (hover). Back is a bordered ghost on the dark field | Continue hover **live**. Back / default Continue **needs** a second frame. |

### 3.7 Keybindings (act 06 shortcuts)

| Traycer element | Source | egui | Notes |
|---|---|---|---|
| Keybindings page | **K** | Title + rows: command `#0F0F0F` / `#313232` left, chip `#EAEAEA` right | **live** first screenful |
| Shortcut chips | **K** | `Frame` fill `#EAEAEA`, type `#666666`, radius ~6 (same family as nav pill) | Visible: Ctrl+1–Ctrl+9, Ctrl+N, Ctrl+Shift+K, Ctrl+Shift+] / [, Ctrl+Shift+W, Ctrl+T, Ctrl+W, Ctrl+Alt+W, Ctrl+Shift+Alt+] / W, Ctrl+] / [ |
| Cmd+K / the rest of the list | not in the first screenful | — | **needs live** (scroll) |

### 3.8 Host-error

| Traycer element | Source | egui | Notes |
|---|---|---|---|
| Signed-in empty canvas | **HE** | `CentralPanel` + `bg.page` `#F6F9F8` + header/avatar | **live HE**. This is the first signed-in paint *before* the official host process. |
| systemd modal | **HE** | `egui::Window` / card 420×226, `#FFFFFF`, radius 8–10, body `#666666` | Copy is the official host talking about `ai.traycer.host.service` — do not clone the systemd text into RustTraycer. Match *structure* (centered card, Retry + Report). |
| Retry | **HE** | Ghost button, fill `#F6F9F8`, fg `#0F0F0F`, `#DCE7E4` halo | **live**. Do not restyle to welcome cream. |
| Report issue | **HE** | Text + bug icon, `#666666` | **live** |

### 3.9 Sidebar and panels (Task chrome — still mostly fallback)

| Traycer element | Source | egui | States |
|---|---|---|---|
| Sidebar column (Task, not Settings) | **fallback** `/panels` | `egui::SidePanel::left` on the canvas | collapsed / expanded / stacked. Width **needs live (Acts 01–03)**. Do not reuse 240 from Settings without a Task frame. |
| Panel header | **fallback** `/panels` | `ui.horizontal` + `egui::CollapsingHeader` or a custom header row; `+` = small `Button` | hover / collapsed / overflow |
| Panel stack / rearrange | **fallback** `/panels` | Persist order in GUI state. Drag = `egui::dnd` if we take it; otherwise move-up/down is an accepted stand-in until live | **needs live (Acts 01–03)** |
| Agents panel (tree) | **fallback** `/panels/agents` | Existing agent list in `screens/canvas.rs`. Target: tree (`ui.indent`) with Chat/Terminal filter chips, sort menu, row `+` for child | empty / filter-empty / selected / running. **needs live (Acts 01–03)** |
| Agent row / card | **fallback** changelog "second line of detail and a leading icon" | Custom `Frame` + two-line `label` + Lucide-class leading icon. Not the official mark | hover / selected / running / archived. **needs live (Acts 01–03)** |
| Artifacts panel | **fallback** `/panels/artifacts` | `artifacts.rs`. Types Spec/Ticket/Story/Review; ticket/story status Todo / In Progress / Done | empty / filtered. Colors **needs live** |
| Git Diff panel | **fallback** `/panels/git-diff` | Existing git pane. Named states: no worktrees, no changes, loading, error, conflict/detached, binary | **needs live** for chrome |
| File Tree panel | **fallback** `/panels/file-tree` | Existing files pane. Workspace picker; open-in-editor already copy | unavailable host. **needs live** |
| Terminals panel | **fallback** `/panels/terminals` | `terminal.rs` / PTY pane. Not the same as a Terminal-interface *agent* | empty / exited. **needs live** |
| Comments panel | **fallback** `/panels/comments` | Contextual; hide when no active artifact | empty. **needs live** |
| Sharing panel | **fallback** `/panels/sharing` | Out of scope by existing ADR (managed cloud / teams). Do not fake a sharing chrome | — |

### 3.10 Chat, composer, chips

| Traycer element | Source | egui | States |
|---|---|---|---|
| Transcript | **fallback** changelog "redesigned message cards" | `ScrollArea::vertical` + per-message `Frame` | user / assistant / system / pending. **needs live (Acts 01–03)** |
| Chat bubble | same | `egui::Frame` with 4–8 px rounding (4 is the welcome radius; 6–8 showed up on settings pills). Fill unknown | **needs live (Acts 01–03)** |
| Composer (Start Page, empty) | **live SP** | `TextEdit::multiline` in a 673×94 `#F3F5F4` plate, radius ~6, `#B4CDCD` 1 px outline. Placeholder AA `#9E9F9F` | empty **live SP**. This is the Start Page *input*, not a Task thread. |
| Composer (Task thread) | **fallback** `/panels/agents` | Same primitive, pinned at the bottom of a Task canvas. Disabled when `agent.status == running` (IA law) | focus / disabled / sending / transcript. **needs live (Acts 01–03)** |
| Composer controls (model, permissions, thinking, fast, attach, voice, send/stop) | **live SP** for placement on Start Page (Full access / GPT-5.6-Sol / High / mic / send `#EEEFEE`); Voice is also a *setting* on **SG** | Horizontal chip/button row on the composer plate. Labels `#666666` / `#0F0F0F` AA. Send is a ghost disc, not Traycer Green. | Start Page default **live SP**. Hover / unavailable / Task-thread **needs live (Acts 01–03)** |
| Chips (interface filter All/Chat/Terminal; context usage; permission; Deprecated badge) | **fallback** agents + changelog | Small `Button`/`Frame` with 4–6 px radius, 12–15 px type | selected / muted. **needs live (Acts 01–03)** |
| Mentions / hover card | **fallback** changelog | `Popup` / `Area` | **needs live** |

### 3.11 Buttons, inputs, scrolls, tooltips

| Traycer element | Source | egui | States |
|---|---|---|---|
| Primary button (welcome) | **live W** Sign in | `Button` fill `#F8F7F2`, fg `#050505`, rounding 4, height 35 | default **live**. hover / focus / active / disabled **needs live** |
| Continue (onboarding, hover) | **live OA** | `Button` fill `#DEDFDF`, fg `#000000`, on dark canvas | hover **live**. default **needs live** |
| Retry (host-error) | **live HE** | Ghost: fill `#F6F9F8`, fg `#0F0F0F`, halo `#DCE7E4` | default **live** |
| Secondary / danger | not isolated | Ghost assumed 1 px `#DCE7E4` on `#F6F9F8` / `#F9FBFB` after login; welcome ghost remains assumed 1 px `#F8F7F2` at 20% on `#000000` | **needs live** for a dedicated ghost/danger frame |
| Text input | **P** CLI args / **AG** editor; **HY** History search (focused) | `TextEdit`. Editor current-line `#F1FAFF`. History search focus ring is the `#8EB5B6` cloud in §2.1 | empty **live** as History placeholder. Focus ring **live HY** (cloud). Error **needs live** |
| Scroll area | not isolated | `egui::ScrollArea`. Thumb color unknown | **needs live**. Physics: §4 |
| Tooltip | not on any frame | `on_hover_text` / `egui::Tooltip` | **needs live** |
| Context / overflow menu | **fallback** History "right-click menu" | `ui.menu_button` / `popup` | **needs live** (no rows on **HY**) |
| Restart host (banner) | **live SP** | `Button` fill `#257174`, fg `#FFFFFF`, 91×25, radius ~3 | default **live**. Hover **needs live** |

### 3.12 Ladder, dialogs, toasts

| Traycer element | Source | egui | States |
|---|---|---|---|
| Approval card | **fallback** changelog "approval cards summarize the command"; behavior in `docs/e2-ladder-v2.md` | Already `egui::Window` from `screens/canvas.rs` `show_ladder_dialogs` (`ladder.rs` copy). Target: a card, not a full-screen modal. Rounding 4–8 (we now have 4 and ~8). Scrim **needs live** | ask / allow-once / allow-always / deny |
| Host-error / systemd card | **live HE** | Centered card — see §3.8. RustTraycer should not paste official systemd copy | — |
| Yolo confirm | our spec, not Traycer "full access default" (C26 out of scope) | Existing `YOLO_CONFIRM_*` window | confirm / cancel |
| Push / commit confirms | our git panel | Existing windows | — |
| Notification / toast | **fallback** changelog notification center; bell icon is **live HT** | Today: bottom-right `egui::Window` "Сообщение". Target: clickable toast; a center list is later | **needs live** |
| Resource monitor | **fallback** changelog; gauge icon is **live HT** | `metrics.rs` chip in chrome | **needs live** for the open panel |

Empty states that *are* specified as behavior (not pixels) remain in `docs/gui-ia-v0.md`: no host, no workspace, no tasks, no agent. Visual treatment: host-offline **HE**; Start Page empty list **SP**; History empty **HY**. no-workspace and a populated History **need live**.

### 3.13 Start Page

Inventory for **SP**. Tokens from §2.1 (`bg.page`, `banner.update`, `surface.composer`, `accent.traycer-green`). This is the Start Page *page*, not a Task canvas.

| Traycer element | Source | egui | Notes |
|---|---|---|---|
| Page wash | **live SP** | `CentralPanel` + `bg.page` `#F6F9F8` | 743 613 px at **SP** `(640, 200)`. Same wash as Settings / **HE**. |
| Greeting | **live SP** | `RichText` ~30 px Semibold, `fg.primary` `#0F0F0F` | "Good morning" bbox `(519, 342)–(757, 374)`. |
| Subtitle | **live SP** | 13–15 px Regular, `fg.secondary` `#666666` | "What's on your mind?" |
| Update banner | **live SP** | `Frame` 673×43, fill `#DDF0F6`, edge `#9ADAF5`, type `#052F4A`, radius ~5–6 | Session state ("Update installed — restart host to finish."), not permanent chrome. |
| Restart host | **live SP** | `Button` 91×25, fill `#257174`, fg `#FFFFFF`, radius ~3 | Same accent as Settings ON toggle. Hover **needs live**. |
| Composer plate | **live SP** | `TextEdit::multiline` in 673×94 `#F3F5F4`, outline `#B4CDCD`, radius ~6 | Placeholder AA `#9E9F9F`. Not a Task-thread composer. |
| Composer controls | **live SP** | Chip/button row on the plate (Full access / model / High / mic / send `#EEEFEE`) | Labels `#666666` / `#0F0F0F`. Send is a ghost disc, not Traycer Green. |
| Recent / Filter / Select | **live SP** | Toolbar over the empty list, `#666666` | Empty list **live**. Populated recents **needs live**. |
| Empty list | **live SP** | No rows under the toolbar | Density: greeting + composer + empty field. no-workspace **needs live**. |

### 3.14 History empty

Inventory for **HY**. Tokens from §2.1 (`history.header`, `history.search.ring`, `history.scrim`). Empty modal only — no rows.

| Traycer element | Source | egui | Notes |
|---|---|---|---|
| Scrim | **live HY** | Translucent fill over Start Page, dominant `#ACAEAD` | Cloud, not a flat hex. Same accepted limit as §4.2. |
| Modal | **live HY** | `egui::Window` / card **1024×553**, radius ~8 | `(128, 121)–(1151, 673)`. Edge `#919291`. |
| Title bar | **live HY** | 40 px `#EAEAEA`, title `#0F0F0F` ~15 px | "History" at `(167, 134)–(213, 147)`. Pop-out + close on the right. |
| Header seam | **live HY** | Two 1 px blends `#E4E9E7` then `#F0F4F2` | Do not collapse into `hairline.header`. |
| Search | **live HY** | `TextEdit` ~666×35, interior `#F6F9F8`, focused ring cloud `#8EB5B6` | Placeholder "Search by title, repo, branch, or PR" `#666666`. |
| Toolbar | **live HY** | Most recent / Filter / Select / Refresh, `#666666` | **HY** `(682, 233)–(962, 245)`. |
| Empty copy | **live HY** | Centered "No tasks yet", `fg.primary` AA cloud | `(605, 323)–(675, 335)`. 15 px Regular. |
| History *rows* / loading / no-match | not on **HY** | `screens/tasks.rs` list | **needs live**. |

---

## 4. Accepted deviations (not bugs)

These are engine and legal limits. Reviewer does not open a DF against them.

1. **epaint raster vs Chromium.** Official Desktop is Electron/Chromium. `rt-gui` is egui + epaint (software or glow). Subpixel font hinting, gamma, and hairline coverage will not match a Chrome screenshot at 1:1. Compare structure, tokens, and rhythm, not a flip-book overlay.
2. **Backdrop-blur / film grain → translucent fill.** The welcome floor and the onboarding dark field are noisy teal washes. The ACT 05 backdrop is a blurred gray cloud (`#7F7F7F`). egui has no backdrop-filter and no cheap grain shader in our stack. A flat or lightly dithered wash (or omitting the glow / prism) is accepted. Do not unpack a Traycer texture to fake it.
3. **CSS animations → egui transitions.** No looping glow or page transition is required. `request_repaint_after` for status pills and streaming tokens is enough. Do not chase Chromium easing curves.
4. **Scroll physics.** Chromium + OS smooth scroll vs egui's discrete `ScrollArea` (no rubber-band, no inertial coast unless we add it). Matching pixel-per-notch is accepted as "good enough"; kinetic scroll is not a v1 parity bug.
5. **Native window chrome.** XFWM title bar, GTK/Electron menu, and Plank on **D** / **S** / **A** are the reference *machine*, not the product. `rt-gui` keeps eframe decorations and its own 40 px nav.
6. **Brand mark and fonts.** We do not ship the Traycer logo or their font files. Inter-class OFL at **15 px** (the live Figtree default) + Lucide-class ISC + the RustTraycer wordmark are the stand-in. A different geometric sans of the same class is not a bug. Naming "Figtree" in our UI is optional and must not imply we vendored their file.
7. **Forced Light / Dark.** Appearance documents and shows a Light / Dark / System picker (**live A**, System selected). We did not capture a forced-Light or forced-Dark chrome pass. Matching System-on-this-machine (the light tokens in §2.1) is accepted until those frames exist.
8. **Sign-in itself.** RustTraycer does not implement Traycer's PKCE wall. The welcome tokens apply to *any* full-canvas empty/gate we choose to draw (host-offline, first-run), not to a clone of their auth. After login, host-offline looks like **HE** (light page + card), not the black welcome.
9. **Provider / agent brand icons.** Codex / Claude / Cursor / … marks on **P** are third-party brand assets. Lucide-class stand-ins (or a letter in a circle) are accepted.

---

## 5. What this spec does not do

- No production `rt-gui` edits, no crate bump, no tag, no origin push.
- No new screenshots.
- No asar, no font/icon vendoring.
- No claim that docs.traycer.ai pages are live Desktop captures.
- No invented hex for chat bubbles, Task sidebar, ladder scrim, or forced Light/Dark.
- No claim that Acts 01–03 were captured. They were not.

Next Architect pass: a Task with the Agents sidebar and a Chat transcript + composer (Acts 01–03), a ladder/approval card, History *rows*, Settings → Notifications / Shell / Worktrees / Host / Diagnostics, and Appearance forced Light + Dark. Replace every remaining **needs live** row with a pipetted token and a frame id. Start Page contents and History empty are already live (**SP** / **HY**).

---

## 6. Pointers

- Live frames: [`docs/reference-screens/`](reference-screens/)
- How those frames were taken: [`docs/reference-env.md`](reference-env.md)
- IA / empty states: [`docs/gui-ia-v0.md`](gui-ia-v0.md)
- Canvas behavior: [`docs/e1-canvas-v2.md`](e1-canvas-v2.md)
- Ladder behavior: [`docs/e2-ladder-v2.md`](e2-ladder-v2.md)
- Capability matrix: [`docs/parity-matrix.md`](parity-matrix.md)
