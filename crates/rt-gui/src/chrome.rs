use crate::ladder::YOLO_BANNER;
use crate::metrics::METRICS_LABEL;
use crate::state::{AppState, Screen};
use crate::theme::{self, Icon};

pub const TAB_START_PAGE: &str = "Start Page";
pub const TAB_SETTINGS: &str = "Settings";
pub const TAB_NEW: &str = "+";
pub const AVATAR_FALLBACK: &str = "RT";

pub fn show(ctx: &egui::Context, state: &mut AppState) {
    egui::TopBottomPanel::top("chrome_nav")
        .exact_height(theme::CHROME_NAV_HEIGHT)
        .frame(theme::header_frame())
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                let _ = chrome_icon_button(ui, Icon::ChevronLeft, false, "Back");
                let _ = chrome_icon_button(ui, Icon::ChevronRight, false, "Forward");
                ui.add_space(theme::SPACE_4);

                let start_selected = state.screen == Screen::Tasks;
                if chrome_tab(ui, start_selected, Icon::Layers, TAB_START_PAGE, false).clicked {
                    state.screen = Screen::Tasks;
                }

                let settings_selected = state.screen == Screen::Host;
                let settings =
                    chrome_tab(ui, settings_selected, Icon::Settings, TAB_SETTINGS, true);
                if settings.close {
                    state.screen = Screen::Tasks;
                } else if settings.clicked {
                    state.screen = Screen::Host;
                }

                if plus_tab(ui) {
                    state.show_new_task_dialog = true;
                    state.new_task_title.clear();
                }

                let metrics = state.metrics_chip_value();
                let initials = avatar_initials(state);
                let demo = state.demo;

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(theme::SPACE_8);
                    avatar_disc(ui, &initials);
                    let _ = chrome_icon_button(ui, Icon::Bell, true, "Notifications");
                    if chrome_icon_button(ui, Icon::History, true, "History") {
                        state.screen = Screen::Tasks;
                    }
                    if chrome_icon_button(ui, Icon::Settings, true, TAB_SETTINGS) {
                        state.screen = Screen::Host;
                    }
                    utility_cluster(ui, &metrics);
                    if demo {
                        ui.weak("демо");
                    }
                });
            });
        });

    if state.is_offline() {
        egui::TopBottomPanel::top("offline_banner").show(ctx, |ui| {
            egui::Frame::new()
                .fill(theme::BANNER_OFFLINE_FILL)
                .inner_margin(egui::Margin::symmetric(
                    theme::SPACE_12 as i8,
                    theme::SPACE_8 as i8,
                ))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.colored_label(
                            theme::BANNER_OFFLINE_FG,
                            "Host не запущен или не отвечает. Поднимите его через CLI (`rt-cli start`) — GUI его не стартует.",
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new("Повторить")
                                        .fill(theme::BANNER_OFFLINE_BUTTON),
                                )
                                .clicked()
                            {
                                state.request_retry();
                            }
                        });
                    });
                });
        });
    }

    if state.yolo_on() {
        egui::TopBottomPanel::top("yolo_banner").show(ctx, |ui| {
            egui::Frame::new()
                .fill(theme::BANNER_YOLO_FILL)
                .inner_margin(egui::Margin::symmetric(
                    theme::SPACE_12 as i8,
                    theme::SPACE_8 as i8,
                ))
                .show(ui, |ui| {
                    ui.colored_label(theme::BANNER_YOLO_FG, YOLO_BANNER);
                });
        });
    }

    if state.is_online() {
        if let Some(msg) = state.ws_banner.clone() {
            egui::TopBottomPanel::top("ws_banner").show(ctx, |ui| {
                egui::Frame::new()
                    .fill(theme::BANNER_WS_FILL)
                    .inner_margin(egui::Margin::symmetric(
                        theme::SPACE_12 as i8,
                        theme::SPACE_8 as i8,
                    ))
                    .show(ui, |ui| {
                        ui.colored_label(theme::BANNER_WS_FG, msg);
                    });
            });
        }
    }
}

struct TabClick {
    clicked: bool,
    close: bool,
}

fn chrome_tab(
    ui: &mut egui::Ui,
    selected: bool,
    icon: Icon,
    label: &str,
    closable: bool,
) -> TabClick {
    let fill = if selected {
        theme::BG_HEADER
    } else {
        theme::BG_TAB_INACTIVE
    };
    let fg = if selected {
        theme::FG_PRIMARY
    } else {
        theme::FG_SECONDARY
    };
    let rounding = egui::CornerRadius {
        nw: theme::RADIUS_NAV as u8,
        ne: theme::RADIUS_NAV as u8,
        sw: 0,
        se: 0,
    };
    let mut clicked = false;
    let mut close = false;
    let inner = egui::Frame::new()
        .fill(fill)
        .corner_radius(rounding)
        .inner_margin(egui::Margin::symmetric(
            theme::SPACE_8 as i8,
            theme::SPACE_4 as i8,
        ))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = theme::SPACE_4;
            ui.horizontal(|ui| {
                theme::show_icon(ui, icon, theme::SIZE_CHIP, fg);
                let label_resp = ui.add(
                    egui::Label::new(egui::RichText::new(label).color(fg))
                        .sense(egui::Sense::click()),
                );
                if label_resp.clicked() {
                    clicked = true;
                }
                if closable {
                    let close_resp = ui.add(
                        egui::Label::new(egui::RichText::new("×").color(theme::FG_SECONDARY))
                            .sense(egui::Sense::click()),
                    );
                    if close_resp.clicked() {
                        close = true;
                    }
                }
            });
        });
    if inner.response.interact(egui::Sense::click()).clicked() && !close {
        clicked = true;
    }
    TabClick { clicked, close }
}

fn plus_tab(ui: &mut egui::Ui) -> bool {
    let inner = egui::Frame::new()
        .fill(theme::BG_TAB_INACTIVE)
        .corner_radius(egui::CornerRadius {
            nw: theme::RADIUS_NAV as u8,
            ne: theme::RADIUS_NAV as u8,
            sw: 0,
            se: 0,
        })
        .inner_margin(egui::Margin::symmetric(
            theme::SPACE_8 as i8,
            theme::SPACE_4 as i8,
        ))
        .show(ui, |ui| {
            theme::show_icon(ui, Icon::Plus, theme::SIZE_CHIP, theme::FG_SECONDARY);
        });
    inner
        .response
        .interact(egui::Sense::click())
        .on_hover_text(TAB_NEW)
        .clicked()
}

fn chrome_icon_button(ui: &mut egui::Ui, icon: Icon, enabled: bool, tip: &str) -> bool {
    let size = egui::vec2(
        theme::SIZE_UI + theme::SPACE_4,
        theme::SIZE_UI + theme::SPACE_4,
    );
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    let color = if !enabled {
        theme::HAIRLINE_CONTROL
    } else if resp.hovered() {
        theme::FG_PRIMARY
    } else {
        theme::FG_SECONDARY
    };
    theme::paint_icon(ui.painter(), rect.shrink(2.0), icon, color);
    let resp = resp.on_hover_text(tip);
    enabled && resp.clicked()
}

fn utility_cluster(ui: &mut egui::Ui, metrics: &str) {
    egui::Frame::new()
        .fill(theme::BG_HEADER)
        .stroke(egui::Stroke::new(1.0, theme::HAIRLINE_CONTROL))
        .corner_radius(theme::RADIUS_NAV)
        .inner_margin(egui::Margin::symmetric(
            theme::SPACE_4 as i8,
            theme::SPACE_4 as i8,
        ))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme::SPACE_4;
                let _ = chrome_icon_button(
                    ui,
                    Icon::Gauge,
                    true,
                    &format!("{METRICS_LABEL} {metrics}"),
                );
                let _ = chrome_icon_button(ui, Icon::Overflow, true, "More");
            });
        });
}

fn avatar_disc(ui: &mut egui::Ui, initials: &str) {
    let size = egui::vec2(theme::AVATAR_DISC_W, theme::AVATAR_DISC);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let radius = rect.height() * 0.5;
    ui.painter()
        .circle_filled(rect.center(), radius, theme::BG_NAV_SELECTED);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        initials,
        egui::FontId::new(
            theme::SIZE_AVATAR,
            egui::FontFamily::Name(theme::FAMILY_SEMIBOLD.into()),
        ),
        theme::FG_SECONDARY,
    );
}

fn avatar_initials(state: &AppState) -> String {
    let picked = state.picked_account_id();
    let label = picked
        .as_deref()
        .and_then(|id| {
            state
                .accounts
                .iter()
                .find(|a| a.id == id)
                .map(|a| a.display_label())
        })
        .or_else(|| state.accounts.first().map(|a| a.display_label()));
    match label {
        Some(label) => initials_from_label(label),
        None => AVATAR_FALLBACK.to_string(),
    }
}

fn initials_from_label(label: &str) -> String {
    let words: Vec<&str> = label.split_whitespace().filter(|w| !w.is_empty()).collect();
    if words.len() >= 2 {
        if let (Some(a), Some(b)) = (first_letter(words[0]), first_letter(words[1])) {
            return format!("{a}{b}");
        }
    }
    let mut out = String::new();
    for ch in label.chars().filter(|c| c.is_alphabetic()).take(2) {
        if let Some(up) = ch.to_uppercase().next() {
            out.push(up);
        }
    }
    if out.is_empty() {
        AVATAR_FALLBACK.to_string()
    } else {
        out
    }
}

fn first_letter(word: &str) -> Option<char> {
    let ch = word.chars().find(|c| c.is_alphabetic())?;
    ch.to_uppercase().next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account_ux::AccountItem;
    use crate::state::AppState;

    #[test]
    fn tab_strip_labels_match_design_parity() {
        assert_eq!(TAB_START_PAGE, "Start Page");
        assert_eq!(TAB_SETTINGS, "Settings");
        assert_eq!(TAB_NEW, "+");
        assert_eq!(AVATAR_FALLBACK, "RT");
        let src = include_str!("chrome.rs");
        let prod = match src.split("#[cfg(test)]").next() {
            Some(part) => part,
            None => src,
        };
        assert!(prod.contains("Start Page"));
        assert!(prod.contains("Settings"));
        assert!(!prod.contains("SEARCH_LABEL"));
        assert!(!prod.contains("SEARCH_HINT"));
        assert!(!prod.contains("search_q"));
        assert!(!prod.contains("Поиск"));
        assert!(!prod.contains("Задачи"));
    }

    #[test]
    fn chrome_has_no_online_status_pill() {
        let src = include_str!("chrome.rs");
        let prod = match src.split("#[cfg(test)]").next() {
            Some(part) => part,
            None => src,
        };
        assert!(!prod.contains("status_pill"));
        assert!(!prod.contains("status_pill_colors"));
        assert!(!prod.contains("HostStatus"));
        assert!(!prod.contains("онлайн"));
        assert!(!prod.contains("подключение"));
        assert!(!prod.contains("офлайн"));
        assert!(prod.contains("avatar_disc"));
        let host_src = include_str!("screens/host.rs");
        assert!(host_src.contains("host_status.label_ru()"));
        assert_eq!(crate::state::HostStatus::Online.label_ru(), "онлайн");
        assert_eq!(
            crate::state::HostStatus::Connecting.label_ru(),
            "подключение"
        );
        assert_eq!(crate::state::HostStatus::Offline.label_ru(), "офлайн");
        assert_eq!(theme::header_frame().fill, theme::BG_HEADER);
        assert_eq!(theme::color_hex(theme::header_frame().fill), "#FFFFFF");
        assert_eq!(
            theme::color_hex(theme::header_frame().stroke.color),
            "#DFE9E7"
        );
    }

    #[test]
    fn avatar_initials_use_label_or_rt() {
        assert_eq!(initials_from_label(""), AVATAR_FALLBACK);
        assert_eq!(initials_from_label("   "), AVATAR_FALLBACK);
        assert_eq!(initials_from_label("123"), AVATAR_FALLBACK);
        assert_eq!(initials_from_label("work"), "WO");
        assert_eq!(initials_from_label("Valeriy Khalikov"), "VK");
        let mut state = AppState::new();
        assert_eq!(avatar_initials(&state), AVATAR_FALLBACK);
        state.accounts.push(AccountItem {
            id: "acc-1".into(),
            label: "Ada Lovelace".into(),
            provider: None,
        });
        assert_eq!(avatar_initials(&state), "AL");
        state.picker_account_id = Some("acc-1".into());
        state.accounts.push(AccountItem {
            id: "acc-2".into(),
            label: "work".into(),
            provider: None,
        });
        assert_eq!(avatar_initials(&state), "AL");
    }
}
