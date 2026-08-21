# Design-parity v1 — visual tokens and component map

For: UI (`rt-gui`, eframe + egui).
From: Architect. Date: 2026-08-21. Not code. No crate bump.
Base: STAR 0126 pin `docs/reference-env.md` (Traycer Desktop `desktop-v1.1.10`).
Status: first visual-parity contract. Tokens that were pipetted from a live frame are law for those surfaces. Tokens that exist only as public-docs IA are **not** law for hex.

This file does not replace `docs/gui-ia-v0.md` (screens, RPC, empty states) or `docs/e1-canvas-v2.md` / `docs/e2-ladder-v2.md` (behavior). It tells `rt-gui` what the official Desktop *looks like*, what to implement in egui, and which mismatches are accepted engine limits rather than bugs.

---

## 0. Legal hygiene

- Do **not** unpack or open `app.asar`. Do **not** copy CSS, fonts, icons, or other Traycer brand assets from the AppImage / squashfs tree.
- Screenshots of the running window are *our* files and may live under `docs/reference-screens/`. The AppImage binary stays out of git (see `docs/reference-env.md`).
- Do **not** vendor Traycer fonts. Recommendation only: an Inter-class geometric sans under the SIL Open Font License (for example [Inter](https://github.com/rsms/inter)). Recommendation only for icons: [Lucide](https://lucide.dev) (ISC). Neither is a brand clone.
- Do not reproduce the official Traycer mark. RustTraycer keeps its own wordmark (`chrome.rs` already says "RustTraycer").
- Public pages on https://docs.traycer.ai were fetched as a **fallback** for surfaces behind the sign-in wall. They are documentation, not live captures. This spec does **not** claim they are screenshots of Desktop 1.1.10.

---

## 1. Sources

### 1.1 Live in-tree frames (law for hex / ruler)

Pinned by STAR 0126. Observe-only X11 dumps of official Traycer Desktop 1.1.10 on `DISPLAY=:5` (1280×800×24). Taken 2026-08-21 09:37 YEKT / 04:37 UTC. Method and sha256: `docs/reference-env.md`.

| ID | Path | Pixels | What it is |
|---|---|---|---|
| **W** | [`docs/reference-screens/welcome-sign-in.png`](reference-screens/welcome-sign-in.png) | 1280×719 RGB | Official client window: native File/Edit/View/Window/Help strip, black canvas, white mark, heading "Welcome to Traycer", single **Sign in** button, grainy teal floor glow. No email field. |
| **D** | [`docs/reference-screens/display5-1280x800-desktop.png`](reference-screens/display5-1280x800-desktop.png) | 1280×800 RGB | Same moment, full `:5` framebuffer. XFWM title bar "Traycer" (24 px) above **W**, Plank dock below. Client pixels match **W** at `y_D = y_W + 24`. |

Do not add new screenshots in this change. Do not treat XFWM / Plank / GTK menu chrome as Traycer product tokens — they are host-desktop chrome that happens to surround the official window.

### 1.2 Fallback — docs.traycer.ai (IA only, not live)

Fetched 2026-08-21. HTML of the docs site was scraped for `<img>` / `og:image` / mintcdn product shots. Result: **no product screenshots**. The only raster images are Font Awesome icons and Mintlify-generated OG cards (`backgroundDark=#0e0e10`, `primaryColor=#454545`). Those are *docs-site* tokens. They are **not** Desktop tokens and were not pipetted into §2.

Fallback pages used for *structure* of login-walled surfaces:

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
- https://docs.traycer.ai/settings/appearance — Theme system/light/dark, preset, UI/code font size, artifact icon colors
- https://docs.traycer.ai/changelog — 1.1.x chrome notes (split, context chip, notification center, redesigned message cards)

Every component that exists only in this column is tagged **fallback docs.traycer.ai** and **needs live after login**.

### 1.3 How pixels were read

Python 3 + Pillow, no eyedropper GUI. A sample is an exact `Image.getpixel((x, y))` on the PNG. Regions used `Image.crop` + a color counter. Distances are inclusive pixel spans (`max − min + 1`). Corner radius is the left/right inset of the cream fill on the first/last rows of the button (classic rounded-rect footprint), not a CSS `border-radius` read from asar.

When a color is a single-pixel sample, the coordinate is in the table. When a fill is a plateau (thousands of identical pixels), the table still cites one representative pixel plus the count.

Anti-aliased type is a cloud, not one hex. The spec records the dominant plateau and the AA neighbors, and says so.

---

## 2. Design tokens from the screens

Every token cites its source frame. Tokens without a live sample are **not invented**.

### 2.1 Palette

#### Canvas (product, frame **W**)

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

No hairline border was measured on the canvas. Left/right mid samples **W** `(20, 360)` and `(1260, 360)` are `#000000`. No card, no sidebar, no splitter on this frame.

#### Native host chrome (frames **W** / **D** — do not copy into `rt-gui`)

These are the Linux window manager and Electron menu, not Traycer widgets.

| Token | Hex | Sample | What it is |
|---|---|---|---|
| `host.menu-strip` | `#F6F5F4` | **W** `(640, 10)`; full rows `y=0…26` | Electron/GTK menu. 27 px tall. Canvas seam: `y=26` `#F6F5F4` → `y=27` `#000000`. |
| `host.menu-label` | `#2E3436` | **W** menu-dark cluster, 106 px at this hex (also `#000000` / `#000001` glyph cores) | "File Edit View Window Help". System UI, ~11 px cap. |
| `host.xfwm-title` | `#D8D5D2` | **D** `(640, 8)` | XFWM title bar fill. Top row **D** `(640, 0)` is `#D9D6D3`. Height 24 px (`y=0…23`); **W** begins at **D** `y=24`. |
| `host.xfwm-title-text` | `#2E3436` | **D** title-dark bbox `(581, 6)–(630, 17)`, 83 px | The word "Traycer" on the title bar. |
| `host.plank` | `#263742` | **D** `(640, 760)` | Plank dock shelf. Not product. |

`rt-gui` already draws its own 40 px `TopBottomPanel` (`chrome.rs`). Do not restyle that panel to match XFWM Adwaita. Match the *client canvas*, not the window manager.

#### Not measured (no live frame)

| Token | Why missing | Until |
|---|---|---|
| Sidebar / panel surface, sidebar border, selected-row fill | Behind sign-in | **needs live after login** |
| Chat bubble user vs assistant fills | Behind sign-in | **needs live after login** |
| Chip / badge fills (context usage, permission, interface filter) | Behind sign-in; changelog names a "context chip" | **needs live after login** |
| Status: agent idle / running / error | Behind sign-in | **needs live after login** |
| Status: ticket/story Todo / In Progress / Done | **fallback docs.traycer.ai** `/panels/artifacts` names the three values only | **needs live after login** |
| Danger / warning / success (toasts, notifications, Yolo banner) | Behind sign-in | **needs live after login**. Current `rt-gui` offline banner `#5C1C1C` is *our* color, not a pipetted Traycer token. |
| Input / focus ring / hover wash | Sign-in is a single button; no field, no hover frame | **needs live after login** |
| Light theme surfaces | Landing is dark. Appearance docs say Theme = system / light / dark | **fallback docs.traycer.ai** `/settings/appearance`; **needs live after login** |
| Scrollbar thumb / track | Nothing scrollable on **W** | **needs live after login** |
| Tooltip fill | Not shown | **needs live after login** |
| Modal / ladder-card scrim | Not shown | **needs live after login** |

Do not borrow Mintlify `backgroundDark=#0e0e10` or `primaryColor=#454545` as stand-ins.

### 2.2 Typography

**Family (recommendation only).** The heading and button on **W** are a geometric neo-grotesque. We did not identify the official face (that would require opening asar or shipping their font files). Implement with an Inter-class OFL font. Do not vendor Traycer fonts. Do not use egui's default Proggy-like debug face for product chrome.

Appearance docs (**fallback docs.traycer.ai** `/settings/appearance`) expose three user sizes: **UI font size**, **code font size**, and (changelog) a separate **terminal** size. Defaults were not visible. `rt-gui` should keep one UI face + one mono face, with a single scale factor.

| Role | Measured box | Inferred size / weight | Sample | Confidence |
|---|---|---|---|---|
| Display heading "Welcome to Traycer" | **W** lum>200 bbox `(462, 374)–(818, 410)` = **357×37** px including AA. Dense white rows `y=382…385` and `y=397…401`. | **28–32 px, weight 600–700**, line-height ≈ 1.15. Advance ≈ 357 / 18 glyphs ≈ 20 px, which fits Inter Bold ~32 more than ~24. | **W** `(640, 384)` `#F0F0F0` (counter/AA through the center column); plateau `#FFFFFF` in the same band. | Size is inferred from a raster, not a CSS `font-size`. Say **~30 px Bold** in implementation notes; ±2 px is not a bug. |
| Primary CTA "Sign in" | Dark-glyph rows **W** `y=461…467` on a 35 px button. | **13–14 px, weight 500–600**. Center column is `#050505` at those rows. | **W** `(632, 464)` `#050505` | Medium, not Bold. Vertical pad ≈ 10–11 px each side of a ~14 px em. |
| Native menu labels | Cap ~10–11 px, **W** `y=7…17` | System UI. Ignore for product type. | **W** `#2E3436` | Not a product token. |
| Body / sidebar / chips / chat | — | Unknown. | — | **needs live after login**. Working assumption until then: 13 px Regular UI, 12 px mono for code/PTY, 12 px Regular for secondary. Mark every use as assumed. |

Line-height was not a CSS property we could read. Measured heading em-box including AA is 37 px on a ~30 px face → line-height ≈ 1.2. Use 1.2–1.35 for body once a live transcript exists.

### 2.3 Spacing, radii, borders, shadows

Measured on **W**, client origin `(0, 0)` = top-left of the 1280×719 window (menu included).

| Token | Value | How it was measured |
|---|---|---|
| `space.menu` | 27 px | Menu fill `y=0…26`; canvas starts `y=27`. Native, not product. |
| `logo.box` | 79×81 px at `(601, 264)–(679, 344)` | Bright (lum>180) span of the mark. Centered: cx = 640. |
| `space.logo → heading` | **30 px** | Logo last bright row `y=344` → heading first lum>200 row `y=374`. `374 − 344 − 1 = 29`; visual gap reads as 30. |
| `space.heading → cta` | **44 px** | Heading last lum>200 row `y=402` (ignore the 4 px "r"/"r" AA speckles at `y=403…410`) → button top `y=447`. `447 − 402 − 1 = 44`. |
| `cta.box` | **193×35 px** at `(544, 447)–(736, 481)` | Long run of `#F8F7F2`. Mid-row `y=464`: `x=544…736`. |
| `cta.radius` | **4 px** | Top row `y=447`: inset 4 L / 4 R. Then 2, 1, 1, 0. Bottom row `y=481`: inset 4. Symmetric rounded rect, not a pill (`35/2 = 17.5` would be a pill). |
| `cta.shadow` | **none** | Halo pixels immediately outside the fill are glow texture (`#0C1F1F`, `#091C1D` at the top corners), not a drop-shadow plate. No darker duplicated rect offset down-right. |
| `cta.border` | **none** | Fill only. No 1 px stroke of a different hex. |
| `layout.center` | Content mid `y ≈ 372.5` vs client mid (after menu) `373.5` | The logo+heading+button cluster is vertically centered in the *canvas*, not in the full window including the menu. Horizontally centered on 640. |

Inferred spacing scale for the rest of the app (not measured on a second surface — treat as a starting ladder, not law):

`4 / 8 / 12 / 16 / 24 / 32 / 48`

- 4 = CTA radius and the smallest inset we actually saw.
- 8 / 12 = typical egui inner margin (current chrome already uses 8–12).
- 24 / 32 / 48 = the measured inter-block gaps (30 and 44 sit between 24–32 and 32–48).

**Do not** invent a sidebar width or a composer height from this frame. Those **need live after login**.

### 2.4 Motion and material (from what the frame can prove)

| Effect | On **W** | egui target |
|---|---|---|
| Floor glow | Grainy teal, no CSS we can read | Translucent flat wash `rgba(38, 89, 94, ≈0.35)` over `#000000`, or omit. **Accepted deviation** (§4). |
| CTA | Static default. Hover/focus/active/disabled not captured | Default = `surface.cta` + `fg.on-cta`. Hover/focus **needs live after login**; until then, 8–12% lighter fill on hover and a 1 px `#F8F7F2` focus ring on a dark field. Mark as assumed. |
| Animation | None on a still frame | No entrance animation required for welcome parity. |

---

## 3. Component inventory

Column "egui" names the existing `rt-gui` surface where one exists as of 0126, or the egui primitive to use. This is a mapping, not an implementation.

Legend: **live W/D** = pipetted. **fallback docs.traycer.ai** = IA only. **needs live after login** = do not ship a "we matched Traycer" claim for hex on that row.

### 3.1 Welcome / sign-in

| Traycer element | State on frame | egui | Notes |
|---|---|---|---|
| Full-window dark canvas | default | `CentralPanel` + `bg.canvas` `#000000` | **live W** |
| Brand mark | default, white, ~79×81 | Do **not** copy the Traycer mark. Own wordmark or a Lucide-class icon, `#FFFFFF` | **live W** for color/size only |
| Display heading | default | `RichText` 28–32 px Strong, `#FFFFFF`, centered | **live W** |
| Primary CTA "Sign in" | default only | `egui::Button` 193×35, fill `#F8F7F2`, fg `#050505`, rounding 4 | **live W**. Hover / focus / active / disabled **needs live after login** |
| Email / password field | absent | — | Official 1.1.10 welcome is one button (browser PKCE). Do not add fields to "match". |
| Floor glow | default | Optional wash; see §4 | **live W** |
| Native menu File/Edit/… | host chrome | Not an egui widget. eframe/winit decorations stay ours | **live W**, not product |

Empty / error / loading of sign-in were not captured. Changelog says a failed sign-in stays visible and that sign-in completes when the browser approves — **fallback docs.traycer.ai** `/changelog`, **needs live after login**.

### 3.2 App chrome (behind login)

| Traycer element | Source | egui (current → target) | States |
|---|---|---|---|
| Title / window controls | **D** is XFWM, not product | eframe native decorations. Do not fake traffic lights | n/a |
| Task tabs (outer) | **fallback** `/concepts/tasks-and-workspace-folders`, changelog "any two tabs side by side" | Today: `chrome.rs` `TopBottomPanel` 40 px with Tasks / Canvas / Host. Target: a tab strip (`egui::ScrollArea` horizontal + selectable labels), not three nav buttons forever | hover / selected / overflow. **needs live after login** |
| Canvas tabs / tiles (inner) | **fallback** same + changelog split | `screens/canvas.rs` panes. Divider = `egui::Resize` / split. Drag-to-edge tiling is C63 (later) | **needs live after login** |
| History | **fallback** `/concepts/history` | `screens/tasks.rs` list. Rows: title, updated, repos/folders, yours vs shared. Search + ownership/repo/workspace filters | empty / loading / no-match. **needs live after login** |
| Start / home (folder first) | **fallback** `/quickstart` | Tasks empty states already in `gui-ia-v0.md`. Visuals **needs live after login** | empty no-host / no-workspace / no-tasks |
| Appearance settings | **fallback** `/settings/appearance` | Host screen. Theme system/light/dark; UI vs code size | **needs live after login** for the settings chrome itself |

### 3.3 Sidebar and panels

| Traycer element | Source | egui | States |
|---|---|---|---|
| Sidebar column | **fallback** `/panels` | `egui::SidePanel::left` on the canvas | collapsed / expanded / stacked. Width **needs live after login** |
| Panel header | **fallback** `/panels` ("Headers can collapse or expand… `+`") | `ui.horizontal` + `egui::CollapsingHeader` or a custom header row; `+` = small `Button` | hover / collapsed / overflow |
| Panel stack / rearrange | **fallback** `/panels` | Persist order in GUI state. Drag = `egui::dnd` if we take it; otherwise move-up/down is an accepted stand-in until live | **needs live after login** |
| Agents panel (tree) | **fallback** `/panels/agents` | Existing agent list in `screens/canvas.rs`. Target: tree (`ui.indent`) with Chat/Terminal filter chips, sort menu, row `+` for child | empty / filter-empty / selected / running. **needs live after login** |
| Agent row / card | **fallback** changelog "second line of detail and a leading icon" | Custom `Frame` + two-line `label` + Lucide-class leading icon. Not the official mark | hover / selected / running / archived. **needs live after login** |
| Artifacts panel | **fallback** `/panels/artifacts` | `artifacts.rs`. Types Spec/Ticket/Story/Review; ticket/story status Todo / In Progress / Done | empty / filtered. Colors **needs live after login** |
| Git Diff panel | **fallback** `/panels/git-diff` | Existing git pane. Named states: no worktrees, no changes, loading, error, conflict/detached, binary | **needs live after login** for chrome |
| File Tree panel | **fallback** `/panels/file-tree` | Existing files pane. Workspace picker; open-in-editor already copy | unavailable host. **needs live after login** |
| Terminals panel | **fallback** `/panels/terminals` | `terminal.rs` / PTY pane. Not the same as a Terminal-interface *agent* | empty / exited. **needs live after login** |
| Comments panel | **fallback** `/panels/comments` | Contextual; hide when no active artifact | empty. **needs live after login** |
| Sharing panel | **fallback** `/panels/sharing` | Out of scope by existing ADR (managed cloud / teams). Do not fake a sharing chrome | — |

### 3.4 Chat, composer, chips

| Traycer element | Source | egui | States |
|---|---|---|---|
| Transcript | **fallback** changelog "redesigned message cards" | `ScrollArea::vertical` + per-message `Frame` | user / assistant / system / pending. **needs live after login** |
| Chat bubble | same | `egui::Frame` with 4–8 px rounding (4 is the only live radius we have). Fill unknown | **needs live after login** |
| Composer | **fallback** `/panels/agents` | `TextEdit::multiline` pinned at the bottom of the canvas. Disabled when `agent.status == running` (IA law) | empty / focus / disabled / sending. **needs live after login** |
| Composer controls (model, permissions, thinking, fast, attach, voice, send/stop) | **fallback** `/panels/agents` | Horizontal chip/button row above or below the field. Voice may stay later | hover / unavailable ("see why Send is unavailable" — changelog). **needs live after login** |
| Chips (interface filter All/Chat/Terminal; context usage; permission; Deprecated badge) | **fallback** agents + changelog | Small `Button`/`Frame` with 4 px radius, 12 px type | selected / muted. **needs live after login** |
| Mentions / hover card | **fallback** changelog | `Popup` / `Area` | **needs live after login** |

### 3.5 Buttons, inputs, scrolls, tooltips

| Traycer element | Source | egui | States |
|---|---|---|---|
| Primary button | **live W** Sign in | `Button` fill `#F8F7F2`, fg `#050505`, rounding 4, height 35 | default **live**. hover / focus / active / disabled **needs live after login** |
| Secondary / ghost / danger | not on **W** | Ghost = 1 px `#F8F7F2` at 20% on `#000000` is *assumed*, not law | **needs live after login** |
| Text input | not on **W** | `TextEdit`. Focus ring assumed 1 px `#F8F7F2` | empty / focus / error. **needs live after login** |
| Scroll area | not on **W** | `egui::ScrollArea`. Thumb color unknown | **needs live after login**. Physics: §4 |
| Tooltip | not on **W** | `on_hover_text` / `egui::Tooltip` | **needs live after login** |
| Context / overflow menu | **fallback** History "right-click menu" | `ui.menu_button` / `popup` | **needs live after login** |

### 3.6 Ladder, dialogs, toasts

| Traycer element | Source | egui | States |
|---|---|---|---|
| Approval card | **fallback** changelog "approval cards summarize the command"; behavior in `docs/e2-ladder-v2.md` | Already `egui::Window` from `screens/canvas.rs` `show_ladder_dialogs` (`ladder.rs` copy). Target: a card, not a full-screen modal. Rounding 4. Scrim **needs live after login** | ask / allow-once / allow-always / deny |
| Yolo confirm | our spec, not Traycer "full access default" (C26 out of scope) | Existing `YOLO_CONFIRM_*` window | confirm / cancel |
| Push / commit confirms | our git panel | Existing windows | — |
| Notification / toast | **fallback** changelog notification center | Today: bottom-right `egui::Window` "Сообщение". Target: clickable toast; a center list is later | **needs live after login** |
| Resource monitor | **fallback** changelog | `metrics.rs` chip in chrome | **needs live after login** |

Empty states that *are* specified as behavior (not pixels) remain in `docs/gui-ia-v0.md`: no host, no workspace, no tasks, no agent. Visual treatment of those empties **needs live after login**.

---

## 4. Accepted deviations (not bugs)

These are engine and legal limits. Reviewer does not open a DF against them.

1. **epaint raster vs Chromium.** Official Desktop is Electron/Chromium. `rt-gui` is egui + epaint (software or glow). Subpixel font hinting, gamma, and hairline coverage will not match a Chrome screenshot at 1:1. Compare structure, tokens, and rhythm, not a flip-book overlay.
2. **Backdrop-blur / film grain → translucent fill.** The welcome floor is a noisy teal wash. egui has no backdrop-filter and no cheap grain shader in our stack. A flat or lightly dithered wash (or omitting the glow) is accepted. Do not unpack a Traycer texture to fake it.
3. **CSS animations → egui transitions.** No looping glow or page transition is required. `request_repaint_after` for status pills and streaming tokens is enough. Do not chase Chromium easing curves.
4. **Scroll physics.** Chromium + OS smooth scroll vs egui's discrete `ScrollArea` (no rubber-band, no inertial coast unless we add it). Matching pixel-per-notch is accepted as "good enough"; kinetic scroll is not a v1 parity bug.
5. **Native window chrome.** XFWM title bar, GTK/Electron menu, and Plank on **D** are the reference *machine*, not the product. `rt-gui` keeps eframe decorations and its own 40 px nav.
6. **Brand mark and fonts.** We do not ship the Traycer logo or their font files. Inter-class OFL + Lucide-class ISC + the RustTraycer wordmark are the stand-in. A different geometric sans of the same class is not a bug.
7. **Light theme.** Documented (**fallback** `/settings/appearance`) but not captured. Dark-only `rt-gui` is accepted until a live light frame exists.
8. **Sign-in itself.** RustTraycer does not implement Traycer's PKCE wall. The welcome tokens apply to *any* full-canvas empty/gate we choose to draw (host-offline, first-run), not to a clone of their auth.

---

## 5. What this spec does not do

- No production `rt-gui` edits, no crate bump, no tag, no origin push.
- No new screenshots.
- No asar, no font/icon vendoring.
- No claim that docs.traycer.ai pages are live Desktop captures.
- No invented hex for sidebar, bubbles, chips, status, or light theme.

Next Architect pass after PO login: capture Home / History, a Task with the Agents sidebar, a Chat transcript + composer, a ladder/approval card, and Settings › Appearance (dark and light). Replace every **needs live after login** row with a pipetted token and a frame id.

---

## 6. Pointers

- Live frames: [`docs/reference-screens/`](reference-screens/)
- How those frames were taken: [`docs/reference-env.md`](reference-env.md)
- IA / empty states: [`docs/gui-ia-v0.md`](gui-ia-v0.md)
- Canvas behavior: [`docs/e1-canvas-v2.md`](e1-canvas-v2.md)
- Ladder behavior: [`docs/e2-ladder-v2.md`](e2-ladder-v2.md)
- Capability matrix: [`docs/parity-matrix.md`](parity-matrix.md)
