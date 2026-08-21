//! Design-parity tokens for `rt-gui` (docs/design-parity-v1.md §2).
//!
//! Every product hex lives here. Screens call these tokens — they must not
//! hardcode `#RRGGBB`. Inter (OFL) is the intended UI face; Lucide (ISC)
//! paths are the icon stand-ins. Do not vendor Traycer fonts or marks.

use std::sync::Arc;

use egui::style::WidgetVisuals;
use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke, Style,
    TextStyle, Visuals,
};

const INTER_REGULAR: &[u8] = include_bytes!("../fonts/Inter-Regular.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../fonts/Inter-SemiBold.ttf");

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

// ---------------------------------------------------------------------------
// Canvas (signed-out welcome, frame W)
// ---------------------------------------------------------------------------

pub const BG_CANVAS: Color32 = rgb(0x00, 0x00, 0x00);
pub const BG_CANVAS_HEX: &str = "#000000";
pub const BG_CANVAS_NEAR_GLOW: Color32 = rgb(0x04, 0x10, 0x10);
pub const BG_CANVAS_NEAR_GLOW_HEX: &str = "#041010";
pub const FG_LOGO: Color32 = rgb(0xFF, 0xFF, 0xFF);
pub const FG_LOGO_HEX: &str = "#FFFFFF";
pub const FG_HEADING: Color32 = rgb(0xFF, 0xFF, 0xFF);
pub const FG_HEADING_HEX: &str = "#FFFFFF";
pub const SURFACE_CTA: Color32 = rgb(0xF8, 0xF7, 0xF2);
pub const SURFACE_CTA_HEX: &str = "#F8F7F2";
pub const FG_ON_CTA: Color32 = rgb(0x05, 0x05, 0x05);
pub const FG_ON_CTA_HEX: &str = "#050505";
pub const GLOW_TEAL_1: Color32 = rgb(0x17, 0x38, 0x37);
pub const GLOW_TEAL_1_HEX: &str = "#173837";
pub const GLOW_TEAL_2: Color32 = rgb(0x1F, 0x4A, 0x4C);
pub const GLOW_TEAL_2_HEX: &str = "#1F4A4C";
pub const GLOW_TEAL_3: Color32 = rgb(0x22, 0x55, 0x5B);
pub const GLOW_TEAL_3_HEX: &str = "#22555B";
pub const GLOW_TEAL_4: Color32 = rgb(0x31, 0x64, 0x69);
pub const GLOW_TEAL_4_HEX: &str = "#316469";
pub const GLOW_TEAL_HI: Color32 = rgb(0x7C, 0x8E, 0x8E);
pub const GLOW_TEAL_HI_HEX: &str = "#7C8E8E";
pub const GLOW_PATCH_MEAN: Color32 = rgb(0x26, 0x59, 0x5E);
pub const GLOW_PATCH_MEAN_HEX: &str = "#26595E";
/// Accepted deviation §4: flat wash instead of grain. ~0.35 over black.
pub const GLOW_WASH: Color32 = Color32::from_rgba_premultiplied(13, 31, 33, 89);

// ---------------------------------------------------------------------------
// Signed-in light chrome (SG / S / HT / A / P / AG / K / HE)
// ---------------------------------------------------------------------------

pub const BG_PAGE: Color32 = rgb(0xF6, 0xF9, 0xF8);
pub const BG_PAGE_HEX: &str = "#F6F9F8";
pub const BG_CONTENT: Color32 = rgb(0xF9, 0xFB, 0xFB);
pub const BG_CONTENT_HEX: &str = "#F9FBFB";
pub const BG_HEADER: Color32 = rgb(0xFF, 0xFF, 0xFF);
pub const BG_HEADER_HEX: &str = "#FFFFFF";
pub const BG_TAB_INACTIVE: Color32 = rgb(0xF6, 0xF9, 0xF8);
pub const BG_TAB_INACTIVE_HEX: &str = "#F6F9F8";
pub const BG_NAV_SELECTED: Color32 = rgb(0xEA, 0xEA, 0xEA);
pub const BG_NAV_SELECTED_HEX: &str = "#EAEAEA";
pub const FG_PRIMARY: Color32 = rgb(0x0F, 0x0F, 0x0F);
pub const FG_PRIMARY_HEX: &str = "#0F0F0F";
pub const FG_SECONDARY: Color32 = rgb(0x66, 0x66, 0x66);
pub const FG_SECONDARY_HEX: &str = "#666666";
pub const ACCENT: Color32 = rgb(0x25, 0x71, 0x74);
pub const ACCENT_HEX: &str = "#257174";
pub const ACCENT_SWATCH: Color32 = rgb(0x1A, 0x24, 0x21);
pub const ACCENT_SWATCH_HEX: &str = "#1A2421";
pub const TOGGLE_OFF_TRACK: Color32 = rgb(0xEA, 0xEA, 0xEA);
pub const TOGGLE_OFF_TRACK_HEX: &str = "#EAEAEA";
pub const TOGGLE_KNOB: Color32 = rgb(0xF6, 0xF9, 0xF8);
pub const TOGGLE_KNOB_HEX: &str = "#F6F9F8";
pub const HAIRLINE_HEADER: Color32 = rgb(0xDF, 0xE9, 0xE7);
pub const HAIRLINE_HEADER_HEX: &str = "#DFE9E7";
pub const HAIRLINE_CONTROL: Color32 = rgb(0xDC, 0xE7, 0xE4);
pub const HAIRLINE_CONTROL_HEX: &str = "#DCE7E4";
pub const HAIRLINE_SIDEBAR: Color32 = rgb(0xE6, 0xEF, 0xEC);
pub const HAIRLINE_SIDEBAR_HEX: &str = "#E6EFEC";
pub const SURFACE_INPUT: Color32 = rgb(0xFF, 0xFF, 0xFF);
pub const SURFACE_INPUT_HEX: &str = "#FFFFFF";
pub const SURFACE_SEGMENT_IDLE: Color32 = rgb(0xF5, 0xF6, 0xF6);
pub const SURFACE_SEGMENT_IDLE_HEX: &str = "#F5F6F6";
pub const SURFACE_SEGMENT_ACTIVE: Color32 = rgb(0xFF, 0xFF, 0xFF);
pub const SURFACE_SEGMENT_ACTIVE_HEX: &str = "#FFFFFF";
pub const CHIP_KEYBINDING: Color32 = rgb(0xEA, 0xEA, 0xEA);
pub const CHIP_KEYBINDING_HEX: &str = "#EAEAEA";
pub const EDITOR_CURRENT_LINE: Color32 = rgb(0xF1, 0xFA, 0xFF);
pub const EDITOR_CURRENT_LINE_HEX: &str = "#F1FAFF";
pub const EDITOR_LOADING_LINE: Color32 = rgb(0xF8, 0xFD, 0xFF);
pub const EDITOR_LOADING_LINE_HEX: &str = "#F8FDFF";

// ---------------------------------------------------------------------------
// Host-error modal (HE)
// ---------------------------------------------------------------------------

pub const MODAL_FILL: Color32 = rgb(0xFF, 0xFF, 0xFF);
pub const MODAL_FILL_HEX: &str = "#FFFFFF";
pub const MODAL_EDGE: Color32 = rgb(0xDC, 0xDF, 0xDE);
pub const MODAL_EDGE_HEX: &str = "#DCDFDE";
pub const FG_MODAL_BODY: Color32 = rgb(0x66, 0x66, 0x66);
pub const FG_MODAL_BODY_HEX: &str = "#666666";
pub const FG_MODAL_ACTION: Color32 = rgb(0x0F, 0x0F, 0x0F);
pub const FG_MODAL_ACTION_HEX: &str = "#0F0F0F";
pub const SURFACE_RETRY: Color32 = rgb(0xF6, 0xF9, 0xF8);
pub const SURFACE_RETRY_HEX: &str = "#F6F9F8";
pub const FG_REPORT: Color32 = rgb(0x66, 0x66, 0x66);
pub const FG_REPORT_HEX: &str = "#666666";

// ---------------------------------------------------------------------------
// Onboarding atmosphere (OA / OH) — clouds, cited as atmosphere
// ---------------------------------------------------------------------------

pub const ONBOARDING_HEADER: Color32 = rgb(0x0E, 0x1A, 0x17);
pub const ONBOARDING_HEADER_HEX: &str = "#0E1A17";
pub const ONBOARDING_FG: Color32 = rgb(0xFF, 0xFF, 0xFF);
pub const ONBOARDING_FG_HEX: &str = "#FFFFFF";
pub const ONBOARDING_CANVAS: Color32 = rgb(0x0B, 0x15, 0x13);
pub const ONBOARDING_CANVAS_HEX: &str = "#0B1513";
pub const ONBOARDING_SCRIM: Color32 = rgb(0x7F, 0x7F, 0x7F);
pub const ONBOARDING_SCRIM_HEX: &str = "#7F7F7F";
pub const ONBOARDING_MODAL: Color32 = rgb(0xFF, 0xFF, 0xFF);
pub const ONBOARDING_MODAL_HEX: &str = "#FFFFFF";
pub const ONBOARDING_CONTINUE: Color32 = rgb(0xDE, 0xDF, 0xDF);
pub const ONBOARDING_CONTINUE_HEX: &str = "#DEDFDF";
pub const ONBOARDING_PROGRESS_ON: Color32 = rgb(0xFF, 0xFF, 0xFF);
pub const ONBOARDING_PROGRESS_ON_HEX: &str = "#FFFFFF";

// ---------------------------------------------------------------------------
// Ours — not pipetted Traycer tokens (spec §2.1 / §3.8)
// Offline banner #5C1C1C stays our color. YOLO / WS banners keep the fills
// already in chrome.rs so screens do not invent hex.
// ---------------------------------------------------------------------------

pub const BANNER_OFFLINE_FILL: Color32 = rgb(0x5C, 0x1C, 0x1C);
pub const BANNER_OFFLINE_FILL_HEX: &str = "#5C1C1C";
pub const BANNER_OFFLINE_FG: Color32 = rgb(0xFF, 0xDC, 0xDC);
pub const BANNER_OFFLINE_BUTTON: Color32 = rgb(0x8C, 0x28, 0x28);
pub const BANNER_YOLO_FILL: Color32 = rgb(0x78, 0x30, 0x10);
pub const BANNER_YOLO_FG: Color32 = rgb(0xFF, 0xDC, 0xAA);
pub const BANNER_WS_FILL: Color32 = rgb(0x48, 0x38, 0x10);
pub const BANNER_WS_FG: Color32 = rgb(0xFF, 0xE6, 0xAA);

// ---------------------------------------------------------------------------
// Spacing scale (spec §2.3): 4 / 8 / 12 / 16 / 24 / 32 / 48
// ---------------------------------------------------------------------------

pub const SPACE_4: f32 = 4.0;
pub const SPACE_8: f32 = 8.0;
pub const SPACE_12: f32 = 12.0;
pub const SPACE_16: f32 = 16.0;
pub const SPACE_24: f32 = 24.0;
pub const SPACE_32: f32 = 32.0;
pub const SPACE_48: f32 = 48.0;

pub const RADIUS_CTA: f32 = 4.0;
pub const RADIUS_NAV: f32 = 6.0;
pub const RADIUS_MODAL: f32 = 8.0;

/// Accepted: keep the existing 40 px nav (spec §4.5). Live tab strip is 37.
pub const CHROME_NAV_HEIGHT: f32 = 40.0;
pub const CHROME_TABS: f32 = 37.0;
pub const NAV_WIDTH: f32 = 240.0;
pub const CTA_WIDTH: f32 = 193.0;
pub const CTA_HEIGHT: f32 = 35.0;
pub const AVATAR_DISC: f32 = 20.0;
/// Live HA disc width (spec §2.3 avatar.disc 19×20).
pub const AVATAR_DISC_W: f32 = 19.0;

// ---------------------------------------------------------------------------
// Type (Inter-class OFL stand-in for live Figtree 15)
// ---------------------------------------------------------------------------

pub const SIZE_UI: f32 = 15.0;
pub const SIZE_TITLE: f32 = 24.0;
pub const SIZE_DISPLAY: f32 = 30.0;
pub const SIZE_SECONDARY: f32 = 13.0;
pub const SIZE_CHIP: f32 = 12.0;
pub const SIZE_MONO: f32 = 12.0;
pub const SIZE_AVATAR: f32 = 11.0;

pub const FAMILY_SEMIBOLD: &str = "inter-semibold";

// ---------------------------------------------------------------------------
// Lucide (ISC) SVG subset we actually draw. Paths match the public lucide
// 24×24 stroke set. No Traycer icons.
// ---------------------------------------------------------------------------

pub const LUCIDE_SEARCH_SVG: &str = concat!(
    r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"##,
    r##"<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/></svg>"##
);
pub const LUCIDE_LIST_SVG: &str = concat!(
    r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"##,
    r##"<path d="M3 12h.01"/><path d="M3 18h.01"/><path d="M3 6h.01"/>"##,
    r##"<path d="M8 12h13"/><path d="M8 18h13"/><path d="M8 6h13"/></svg>"##
);
pub const LUCIDE_LAYERS_SVG: &str = concat!(
    r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"##,
    r##"<path d="M12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83z"/>"##,
    r##"<path d="M2 12a1 1 0 0 0 .58.91l8.6 3.91a2 2 0 0 0 1.65 0l8.58-3.9A1 1 0 0 0 22 12"/>"##,
    r##"<path d="M2 17a1 1 0 0 0 .58.91l8.6 3.91a2 2 0 0 0 1.65 0l8.58-3.9A1 1 0 0 0 22 17"/></svg>"##
);
pub const LUCIDE_SERVER_SVG: &str = concat!(
    r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"##,
    r##"<rect width="20" height="8" x="2" y="2" rx="2" ry="2"/>"##,
    r##"<rect width="20" height="8" x="2" y="14" rx="2" ry="2"/>"##,
    r##"<line x1="6" x2="6.01" y1="6" y2="6"/><line x1="6" x2="6.01" y1="18" y2="18"/></svg>"##
);
pub const LUCIDE_GAUGE_SVG: &str = concat!(
    r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"##,
    r##"<path d="m12 14 4-4"/><path d="M3.34 19a10 10 0 1 1 17.32 0"/></svg>"##
);
pub const LUCIDE_CHECK_SVG: &str = concat!(
    r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"##,
    r##"<path d="M20 6 9 17l-5-5"/></svg>"##
);
pub const LUCIDE_PLUS_SVG: &str = concat!(
    r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"##,
    r##"<path d="M5 12h14"/><path d="M12 5v14"/></svg>"##
);
pub const LUCIDE_MESSAGE_SVG: &str = concat!(
    r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"##,
    r##"<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>"##
);
pub const LUCIDE_SETTINGS_SVG: &str = concat!(
    r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"##,
    r##"<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/>"##,
    r##"<circle cx="12" cy="12" r="3"/></svg>"##
);
pub const LUCIDE_BELL_SVG: &str = concat!(
    r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"##,
    r##"<path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9"/>"##,
    r##"<path d="M10.3 21a1.94 1.94 0 0 0 3.4 0"/></svg>"##
);
pub const LUCIDE_HISTORY_SVG: &str = concat!(
    r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"##,
    r##"<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/>"##,
    r##"<path d="M3 3v5h5"/><path d="M12 7v5l4 2"/></svg>"##
);
pub const LUCIDE_MENU_SVG: &str = concat!(
    r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"##,
    r##"<path d="M4 8h16"/><path d="M4 16h16"/></svg>"##
);
pub const LUCIDE_CHEVRON_LEFT_SVG: &str = concat!(
    r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"##,
    r##"<path d="m15 18-6-6 6-6"/></svg>"##
);
pub const LUCIDE_CHEVRON_RIGHT_SVG: &str = concat!(
    r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"##,
    r##"<path d="m9 18 6-6-6-6"/></svg>"##
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Search,
    List,
    Layers,
    Server,
    Gauge,
    Check,
    Plus,
    Message,
    Settings,
    Bell,
    History,
    Overflow,
    ChevronLeft,
    ChevronRight,
}

pub fn show_icon(ui: &mut egui::Ui, kind: Icon, size: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    paint_icon(ui.painter(), rect, kind, color);
}

pub fn paint_icon(painter: &egui::Painter, rect: egui::Rect, kind: Icon, color: Color32) {
    let stroke = Stroke::new((rect.width() / 12.0).clamp(1.0, 2.0), color);
    let r = rect.shrink(rect.width() * 0.08);
    match kind {
        Icon::Search => {
            let rad = r.width() * 0.28;
            let c = r.center() - egui::vec2(rad * 0.2, rad * 0.2);
            painter.circle_stroke(c, rad, stroke);
            painter.line_segment(
                [
                    c + egui::vec2(rad * 0.72, rad * 0.72),
                    r.right_bottom() - egui::vec2(1.5, 1.5),
                ],
                stroke,
            );
        }
        Icon::List => {
            let y0 = r.top() + r.height() * 0.22;
            let step = r.height() * 0.28;
            for i in 0..3 {
                let y = y0 + step * i as f32;
                let dot = egui::pos2(r.left() + 2.0, y);
                painter.circle_filled(dot, 1.1, color);
                painter.line_segment(
                    [
                        egui::pos2(r.left() + 6.0, y),
                        egui::pos2(r.right() - 1.0, y),
                    ],
                    stroke,
                );
            }
        }
        Icon::Layers => {
            let mid = r.center();
            let w = r.width() * 0.38;
            let h = r.height() * 0.12;
            for (dy, shrink) in [(0.22, 0.0), (0.0, 0.04), (-0.22, 0.08)] {
                let hw = w * (1.0 - shrink);
                painter.line_segment(
                    [
                        mid + egui::vec2(-hw, r.height() * dy + h),
                        mid + egui::vec2(0.0, r.height() * dy - h),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        mid + egui::vec2(0.0, r.height() * dy - h),
                        mid + egui::vec2(hw, r.height() * dy + h),
                    ],
                    stroke,
                );
            }
        }
        Icon::Server => {
            let h = r.height() * 0.32;
            let gap = r.height() * 0.12;
            let top = egui::Rect::from_min_size(
                egui::pos2(r.left() + 1.0, r.top() + 1.0),
                egui::vec2(r.width() - 2.0, h),
            );
            let bot = egui::Rect::from_min_size(
                egui::pos2(r.left() + 1.0, top.bottom() + gap),
                egui::vec2(r.width() - 2.0, h),
            );
            painter.rect_stroke(top, CornerRadius::same(2), stroke, egui::StrokeKind::Inside);
            painter.rect_stroke(bot, CornerRadius::same(2), stroke, egui::StrokeKind::Inside);
            painter.circle_filled(egui::pos2(top.left() + 4.0, top.center().y), 1.2, color);
            painter.circle_filled(egui::pos2(bot.left() + 4.0, bot.center().y), 1.2, color);
        }
        Icon::Gauge => {
            let c = r.center() + egui::vec2(0.0, r.height() * 0.12);
            painter.circle_stroke(c, r.width() * 0.38, stroke);
            painter.line_segment(
                [c, c + egui::vec2(r.width() * 0.18, -r.height() * 0.18)],
                stroke,
            );
        }
        Icon::Check => {
            let a = egui::pos2(r.left() + r.width() * 0.18, r.center().y);
            let b = egui::pos2(r.left() + r.width() * 0.40, r.bottom() - r.height() * 0.22);
            let c = egui::pos2(r.right() - r.width() * 0.16, r.top() + r.height() * 0.22);
            painter.line_segment([a, b], stroke);
            painter.line_segment([b, c], stroke);
        }
        Icon::Plus => {
            let c = r.center();
            let arm = r.width() * 0.32;
            painter.line_segment([c - egui::vec2(arm, 0.0), c + egui::vec2(arm, 0.0)], stroke);
            painter.line_segment([c - egui::vec2(0.0, arm), c + egui::vec2(0.0, arm)], stroke);
        }
        Icon::Message => {
            let body = r.shrink(r.width() * 0.08);
            painter.rect_stroke(
                body.with_max_y(body.bottom() - body.height() * 0.18),
                CornerRadius::same(2),
                stroke,
                egui::StrokeKind::Inside,
            );
            painter.line_segment(
                [
                    egui::pos2(body.left() + 3.0, body.bottom() - body.height() * 0.18),
                    egui::pos2(body.left() + 1.0, body.bottom()),
                ],
                stroke,
            );
        }
        Icon::Settings => {
            painter.circle_stroke(r.center(), r.width() * 0.18, stroke);
            for i in 0..6 {
                let a = (i as f32) * std::f32::consts::TAU / 6.0;
                let inner = r.width() * 0.26;
                let outer = r.width() * 0.40;
                painter.line_segment(
                    [
                        r.center() + egui::vec2(a.cos() * inner, a.sin() * inner),
                        r.center() + egui::vec2(a.cos() * outer, a.sin() * outer),
                    ],
                    stroke,
                );
            }
        }
        Icon::Bell => {
            let c = r.center();
            let top = egui::pos2(c.x, r.top() + r.height() * 0.18);
            painter.line_segment(
                [
                    egui::pos2(r.left() + r.width() * 0.22, c.y + r.height() * 0.08),
                    top,
                ],
                stroke,
            );
            painter.line_segment(
                [
                    top,
                    egui::pos2(r.right() - r.width() * 0.22, c.y + r.height() * 0.08),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(r.left() + r.width() * 0.18, c.y + r.height() * 0.10),
                    egui::pos2(r.right() - r.width() * 0.18, c.y + r.height() * 0.10),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(c.x - r.width() * 0.10, r.bottom() - r.height() * 0.18),
                    egui::pos2(c.x + r.width() * 0.10, r.bottom() - r.height() * 0.18),
                ],
                stroke,
            );
        }
        Icon::History => {
            painter.circle_stroke(r.center(), r.width() * 0.36, stroke);
            painter.line_segment(
                [r.center(), r.center() + egui::vec2(0.0, -r.height() * 0.18)],
                stroke,
            );
            painter.line_segment(
                [
                    r.center(),
                    r.center() + egui::vec2(r.width() * 0.18, r.height() * 0.08),
                ],
                stroke,
            );
        }
        Icon::Overflow => {
            let y0 = r.center().y - r.height() * 0.12;
            let y1 = r.center().y + r.height() * 0.12;
            let x0 = r.left() + r.width() * 0.16;
            let x1 = r.right() - r.width() * 0.16;
            painter.line_segment([egui::pos2(x0, y0), egui::pos2(x1, y0)], stroke);
            painter.line_segment([egui::pos2(x0, y1), egui::pos2(x1, y1)], stroke);
        }
        Icon::ChevronLeft => {
            let c = r.center();
            let dx = r.width() * 0.16;
            let dy = r.height() * 0.22;
            painter.line_segment(
                [egui::pos2(c.x + dx, c.y - dy), egui::pos2(c.x - dx, c.y)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(c.x - dx, c.y), egui::pos2(c.x + dx, c.y + dy)],
                stroke,
            );
        }
        Icon::ChevronRight => {
            let c = r.center();
            let dx = r.width() * 0.16;
            let dy = r.height() * 0.22;
            painter.line_segment(
                [egui::pos2(c.x - dx, c.y - dy), egui::pos2(c.x + dx, c.y)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(c.x + dx, c.y), egui::pos2(c.x - dx, c.y + dy)],
                stroke,
            );
        }
    }
}

pub fn color_hex(c: Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
}

pub fn status_live(live: bool) -> Color32 {
    if live {
        ACCENT
    } else {
        FG_SECONDARY
    }
}

pub fn bubble_fill(role: &str) -> Color32 {
    match role {
        "user" => EDITOR_CURRENT_LINE,
        "assistant" => BG_CONTENT,
        "tool" => SURFACE_SEGMENT_IDLE,
        _ => BG_PAGE,
    }
}

pub fn cap_color(on: bool, muted: bool) -> Color32 {
    if muted {
        FG_SECONDARY
    } else if on {
        ACCENT
    } else {
        FG_SECONDARY
    }
}

pub fn primary_button(text: impl Into<egui::WidgetText>) -> egui::Button<'static> {
    egui::Button::new(text).fill(SURFACE_CTA)
}

pub fn ghost_button(text: impl Into<egui::WidgetText>) -> egui::Button<'static> {
    egui::Button::new(text)
        .fill(SURFACE_RETRY)
        .stroke(Stroke::new(1.0, HAIRLINE_CONTROL))
}

pub fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(MODAL_FILL)
        .stroke(Stroke::new(1.0, MODAL_EDGE))
        .corner_radius(RADIUS_MODAL)
        .inner_margin(Margin::same(SPACE_16 as i8))
}

pub fn content_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(BG_CONTENT)
        .stroke(Stroke::new(1.0, HAIRLINE_CONTROL))
        .corner_radius(RADIUS_NAV)
        .inner_margin(Margin::same(SPACE_8 as i8))
}

pub fn chip_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(CHIP_KEYBINDING)
        .corner_radius(RADIUS_NAV)
        .inner_margin(Margin::symmetric(SPACE_8 as i8, SPACE_4 as i8))
}

pub fn header_frame() -> egui::Frame {
    // Tab-strip field is page-wash; the active tab paints BG_HEADER on top.
    egui::Frame::new()
        .fill(BG_TAB_INACTIVE)
        .inner_margin(Margin::symmetric(SPACE_8 as i8, 0))
        .stroke(Stroke::new(1.0, HAIRLINE_HEADER))
}

/// Install Inter and apply the signed-in light Style + Visuals.
pub fn apply(ctx: &egui::Context) {
    install_fonts(ctx);
    ctx.set_style(signed_in_style());
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "inter".to_owned(),
        Arc::new(FontData::from_static(INTER_REGULAR)),
    );
    fonts.font_data.insert(
        "inter-semibold".to_owned(),
        Arc::new(FontData::from_static(INTER_SEMIBOLD)),
    );
    if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
        family.insert(0, "inter".to_owned());
    }
    fonts.families.insert(
        FontFamily::Name(FAMILY_SEMIBOLD.into()),
        vec!["inter-semibold".to_owned(), "inter".to_owned()],
    );
    ctx.set_fonts(fonts);
}

pub fn signed_in_style() -> Style {
    let mut style = Style {
        visuals: signed_in_visuals(),
        ..Style::default()
    };
    style.spacing.item_spacing = egui::vec2(SPACE_8, SPACE_8);
    style.spacing.button_padding = egui::vec2(SPACE_12, SPACE_8);
    style.spacing.indent = SPACE_16;
    style.spacing.window_margin = Margin::same(SPACE_16 as i8);
    style.spacing.menu_margin = Margin::same(SPACE_8 as i8);

    let semibold = FontFamily::Name(FAMILY_SEMIBOLD.into());
    style.text_styles.insert(
        TextStyle::Body,
        FontId::new(SIZE_UI, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(SIZE_UI, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(SIZE_CHIP, FontFamily::Proportional),
    );
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::new(SIZE_TITLE, semibold));
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(SIZE_MONO, FontFamily::Monospace),
    );
    style
}

fn signed_in_visuals() -> Visuals {
    let mut visuals = Visuals::light();
    visuals.override_text_color = Some(FG_PRIMARY);
    visuals.window_fill = MODAL_FILL;
    visuals.panel_fill = BG_PAGE;
    visuals.faint_bg_color = BG_CONTENT;
    visuals.extreme_bg_color = SURFACE_INPUT;
    visuals.code_bg_color = BG_CONTENT;
    visuals.hyperlink_color = ACCENT;
    visuals.warn_fg_color = FG_SECONDARY;
    visuals.error_fg_color = BANNER_OFFLINE_FILL;
    visuals.window_stroke = Stroke::new(1.0, MODAL_EDGE);
    visuals.window_corner_radius = CornerRadius::same(RADIUS_MODAL as u8);
    visuals.menu_corner_radius = CornerRadius::same(RADIUS_NAV as u8);
    visuals.popup_shadow = egui::Shadow::NONE;
    visuals.window_shadow = egui::Shadow::NONE;
    visuals.selection.bg_fill = BG_NAV_SELECTED;
    visuals.selection.stroke = Stroke::new(1.0, BG_NAV_SELECTED);

    let idle = WidgetVisuals {
        bg_fill: SURFACE_SEGMENT_IDLE,
        weak_bg_fill: BG_PAGE,
        bg_stroke: Stroke::new(1.0, HAIRLINE_CONTROL),
        fg_stroke: Stroke::new(1.0, FG_PRIMARY),
        corner_radius: CornerRadius::same(RADIUS_CTA as u8),
        expansion: 0.0,
    };
    visuals.widgets.noninteractive = WidgetVisuals {
        bg_fill: BG_PAGE,
        weak_bg_fill: BG_PAGE,
        bg_stroke: Stroke::new(1.0, HAIRLINE_SIDEBAR),
        fg_stroke: Stroke::new(1.0, FG_PRIMARY),
        corner_radius: CornerRadius::same(RADIUS_CTA as u8),
        expansion: 0.0,
    };
    visuals.widgets.inactive = idle;
    visuals.widgets.hovered = WidgetVisuals {
        bg_fill: BG_CONTENT,
        weak_bg_fill: BG_CONTENT,
        bg_stroke: Stroke::new(1.0, HAIRLINE_CONTROL),
        fg_stroke: Stroke::new(1.0, FG_PRIMARY),
        corner_radius: CornerRadius::same(RADIUS_CTA as u8),
        expansion: 0.0,
    };
    visuals.widgets.active = WidgetVisuals {
        bg_fill: BG_NAV_SELECTED,
        weak_bg_fill: BG_NAV_SELECTED,
        bg_stroke: Stroke::new(1.0, ACCENT),
        fg_stroke: Stroke::new(1.0, FG_PRIMARY),
        corner_radius: CornerRadius::same(RADIUS_CTA as u8),
        expansion: 0.0,
    };
    visuals.widgets.open = WidgetVisuals {
        bg_fill: SURFACE_INPUT,
        weak_bg_fill: SURFACE_INPUT,
        bg_stroke: Stroke::new(1.0, HAIRLINE_CONTROL),
        fg_stroke: Stroke::new(1.0, FG_PRIMARY),
        corner_radius: CornerRadius::same(RADIUS_NAV as u8),
        expansion: 0.0,
    };
    visuals
}

/// Dark canvas Style for a welcome / signed-out gate. Not the default chrome.
pub fn welcome_style() -> Style {
    let mut style = signed_in_style();
    let mut visuals = Visuals::dark();
    visuals.override_text_color = Some(FG_HEADING);
    visuals.window_fill = BG_CANVAS;
    visuals.panel_fill = BG_CANVAS;
    visuals.extreme_bg_color = BG_CANVAS;
    visuals.faint_bg_color = BG_CANVAS_NEAR_GLOW;
    visuals.hyperlink_color = GLOW_TEAL_HI;
    visuals.selection.bg_fill = GLOW_PATCH_MEAN;
    visuals.widgets.inactive.bg_fill = SURFACE_CTA;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, FG_ON_CTA);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(RADIUS_CTA as u8);
    visuals.widgets.hovered.bg_fill = SURFACE_CTA;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, FG_ON_CTA);
    style.visuals = visuals;
    style
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(color: Color32, hex: &str) -> (String, String) {
        (color_hex(color), hex.to_string())
    }

    #[test]
    fn spec_canvas_tokens_match_hex() {
        assert_eq!(
            pair(BG_CANVAS, BG_CANVAS_HEX),
            ("#000000".into(), "#000000".into())
        );
        assert_eq!(BG_CANVAS_HEX, "#000000");
        assert_eq!(color_hex(BG_CANVAS), "#000000");
        assert_eq!(color_hex(BG_CANVAS_NEAR_GLOW), BG_CANVAS_NEAR_GLOW_HEX);
        assert_eq!(color_hex(FG_LOGO), FG_LOGO_HEX);
        assert_eq!(color_hex(FG_HEADING), FG_HEADING_HEX);
        assert_eq!(SURFACE_CTA_HEX, "#F8F7F2");
        assert_eq!(color_hex(SURFACE_CTA), "#F8F7F2");
        assert_eq!(color_hex(FG_ON_CTA), FG_ON_CTA_HEX);
        assert_eq!(color_hex(GLOW_TEAL_1), GLOW_TEAL_1_HEX);
        assert_eq!(color_hex(GLOW_TEAL_2), GLOW_TEAL_2_HEX);
        assert_eq!(color_hex(GLOW_TEAL_3), GLOW_TEAL_3_HEX);
        assert_eq!(color_hex(GLOW_TEAL_4), GLOW_TEAL_4_HEX);
        assert_eq!(color_hex(GLOW_TEAL_HI), GLOW_TEAL_HI_HEX);
        assert_eq!(color_hex(GLOW_PATCH_MEAN), GLOW_PATCH_MEAN_HEX);
    }

    #[test]
    fn spec_signed_in_bg_fg_match_hex() {
        assert_eq!(BG_PAGE_HEX, "#F6F9F8");
        assert_eq!(color_hex(BG_PAGE), "#F6F9F8");
        assert_eq!(color_hex(BG_CONTENT), BG_CONTENT_HEX);
        assert_eq!(color_hex(BG_HEADER), BG_HEADER_HEX);
        assert_eq!(color_hex(BG_TAB_INACTIVE), BG_TAB_INACTIVE_HEX);
        assert_eq!(color_hex(BG_NAV_SELECTED), BG_NAV_SELECTED_HEX);
        assert_eq!(FG_PRIMARY_HEX, "#0F0F0F");
        assert_eq!(color_hex(FG_PRIMARY), "#0F0F0F");
        assert_eq!(FG_SECONDARY_HEX, "#666666");
        assert_eq!(color_hex(FG_SECONDARY), "#666666");
        assert_eq!(color_hex(ACCENT), ACCENT_HEX);
        assert_eq!(ACCENT_HEX, "#257174");
        assert_eq!(color_hex(ACCENT_SWATCH), ACCENT_SWATCH_HEX);
        assert_eq!(color_hex(TOGGLE_OFF_TRACK), TOGGLE_OFF_TRACK_HEX);
        assert_eq!(color_hex(TOGGLE_KNOB), TOGGLE_KNOB_HEX);
        assert_eq!(color_hex(HAIRLINE_HEADER), HAIRLINE_HEADER_HEX);
        assert_eq!(color_hex(HAIRLINE_CONTROL), HAIRLINE_CONTROL_HEX);
        assert_eq!(color_hex(HAIRLINE_SIDEBAR), HAIRLINE_SIDEBAR_HEX);
        assert_eq!(color_hex(SURFACE_INPUT), SURFACE_INPUT_HEX);
        assert_eq!(color_hex(SURFACE_SEGMENT_IDLE), SURFACE_SEGMENT_IDLE_HEX);
        assert_eq!(
            color_hex(SURFACE_SEGMENT_ACTIVE),
            SURFACE_SEGMENT_ACTIVE_HEX
        );
        assert_eq!(color_hex(CHIP_KEYBINDING), CHIP_KEYBINDING_HEX);
        assert_eq!(color_hex(EDITOR_CURRENT_LINE), EDITOR_CURRENT_LINE_HEX);
        assert_eq!(color_hex(EDITOR_LOADING_LINE), EDITOR_LOADING_LINE_HEX);
    }

    #[test]
    fn spec_modal_and_onboarding_tokens_match_hex() {
        assert_eq!(color_hex(MODAL_FILL), MODAL_FILL_HEX);
        assert_eq!(color_hex(MODAL_EDGE), MODAL_EDGE_HEX);
        assert_eq!(color_hex(FG_MODAL_BODY), FG_MODAL_BODY_HEX);
        assert_eq!(color_hex(FG_MODAL_ACTION), FG_MODAL_ACTION_HEX);
        assert_eq!(color_hex(SURFACE_RETRY), SURFACE_RETRY_HEX);
        assert_eq!(color_hex(FG_REPORT), FG_REPORT_HEX);
        assert_eq!(color_hex(ONBOARDING_HEADER), ONBOARDING_HEADER_HEX);
        assert_eq!(color_hex(ONBOARDING_FG), ONBOARDING_FG_HEX);
        assert_eq!(color_hex(ONBOARDING_CANVAS), ONBOARDING_CANVAS_HEX);
        assert_eq!(color_hex(ONBOARDING_SCRIM), ONBOARDING_SCRIM_HEX);
        assert_eq!(color_hex(ONBOARDING_MODAL), ONBOARDING_MODAL_HEX);
        assert_eq!(color_hex(ONBOARDING_CONTINUE), ONBOARDING_CONTINUE_HEX);
        assert_eq!(
            color_hex(ONBOARDING_PROGRESS_ON),
            ONBOARDING_PROGRESS_ON_HEX
        );
        assert_eq!(BANNER_OFFLINE_FILL_HEX, "#5C1C1C");
        assert_eq!(color_hex(BANNER_OFFLINE_FILL), "#5C1C1C");
    }

    #[test]
    fn spacing_scale_matches_spec() {
        assert_eq!(
            [SPACE_4, SPACE_8, SPACE_12, SPACE_16, SPACE_24, SPACE_32, SPACE_48],
            [4.0, 8.0, 12.0, 16.0, 24.0, 32.0, 48.0]
        );
        assert_eq!(RADIUS_CTA, 4.0);
        assert_eq!(RADIUS_NAV, 6.0);
        assert_eq!(RADIUS_MODAL, 8.0);
        assert_eq!(SIZE_UI, 15.0);
        assert_eq!(NAV_WIDTH, 240.0);
        assert_eq!(CTA_WIDTH, 193.0);
        assert_eq!(CTA_HEIGHT, 35.0);
        assert!((CHROME_TABS - 37.0).abs() < f32::EPSILON);
        assert!((AVATAR_DISC - 20.0).abs() < f32::EPSILON);
        assert!((AVATAR_DISC_W - 19.0).abs() < f32::EPSILON);
    }

    #[test]
    fn signed_in_style_uses_page_tokens() {
        let style = signed_in_style();
        assert_eq!(style.visuals.panel_fill, BG_PAGE);
        assert_eq!(style.visuals.window_fill, MODAL_FILL);
        assert_eq!(style.visuals.override_text_color, Some(FG_PRIMARY));
        assert_eq!(style.visuals.hyperlink_color, ACCENT);
        assert_eq!(style.spacing.item_spacing, egui::vec2(SPACE_8, SPACE_8));
        let welcome = welcome_style();
        assert_eq!(welcome.visuals.panel_fill, BG_CANVAS);
        assert_eq!(welcome.visuals.window_fill, BG_CANVAS);
        let _ = GLOW_WASH;
        let _ = AVATAR_DISC;
        let _ = SIZE_DISPLAY;
        let _ = SIZE_SECONDARY;
        let _ = SIZE_AVATAR;
        let _ = CHROME_NAV_HEIGHT;
    }

    #[test]
    fn inter_ttf_embedded_and_licenses_present() {
        assert!(INTER_REGULAR.len() > 10_000);
        assert!(INTER_SEMIBOLD.len() > 10_000);
        assert_eq!(&INTER_REGULAR[0..4], b"\x00\x01\x00\x00");
        assert_eq!(&INTER_SEMIBOLD[0..4], b"\x00\x01\x00\x00");
        let ofl = include_str!("../licenses/OFL-Inter.txt");
        assert!(ofl.contains("SIL OPEN FONT LICENSE"));
        assert!(ofl.contains("Inter Project Authors"));
        let isc = include_str!("../licenses/ISC-Lucide.txt");
        assert!(isc.contains("ISC License"));
        assert!(isc.contains("Lucide"));
    }

    #[test]
    fn lucide_svg_subset_is_present() {
        for svg in [
            LUCIDE_SEARCH_SVG,
            LUCIDE_LIST_SVG,
            LUCIDE_LAYERS_SVG,
            LUCIDE_SERVER_SVG,
            LUCIDE_GAUGE_SVG,
            LUCIDE_CHECK_SVG,
            LUCIDE_PLUS_SVG,
            LUCIDE_MESSAGE_SVG,
            LUCIDE_SETTINGS_SVG,
            LUCIDE_BELL_SVG,
            LUCIDE_HISTORY_SVG,
            LUCIDE_MENU_SVG,
            LUCIDE_CHEVRON_LEFT_SVG,
            LUCIDE_CHEVRON_RIGHT_SVG,
        ] {
            assert!(svg.contains("viewBox=\"0 0 24 24\""));
            assert!(svg.contains("stroke=\"currentColor\""));
        }
        assert_eq!(bubble_fill("user"), EDITOR_CURRENT_LINE);
        assert_eq!(bubble_fill("assistant"), BG_CONTENT);
        assert_eq!(status_live(true), ACCENT);
        assert_eq!(cap_color(true, false), ACCENT);
    }
}
