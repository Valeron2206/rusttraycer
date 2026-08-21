use crate::ladder::YOLO_BANNER;
use crate::metrics::METRICS_LABEL;
use crate::search_ux::{search_enter_submits, SEARCH_HINT, SEARCH_LABEL};
use crate::state::{AppState, HostStatus, Screen};
use crate::theme::{self, Icon};

pub fn show(ctx: &egui::Context, state: &mut AppState) {
    egui::TopBottomPanel::top("chrome_nav")
        .exact_height(theme::CHROME_NAV_HEIGHT)
        .frame(theme::header_frame())
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.add_space(theme::SPACE_8);
                theme::show_icon(ui, Icon::Layers, theme::SIZE_UI, theme::FG_PRIMARY);
                ui.strong("RustTraycer");
                if state.demo {
                    ui.weak("демо");
                }
                ui.separator();

                nav_item(ui, state, Screen::Tasks, "Задачи", true, Icon::List);

                let canvas_label = state
                    .selected_task_title()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| crate::terminal::TERMINALS_PANE.to_string());
                let canvas_enabled = state.selected_task_id.is_some()
                    || !state.open_task_ids.is_empty()
                    || state.has_workspace();
                nav_item(
                    ui,
                    state,
                    Screen::Canvas,
                    &canvas_label,
                    canvas_enabled,
                    Icon::Message,
                );

                nav_item(ui, state, Screen::Host, "Host", true, Icon::Server);

                ui.separator();
                theme::show_icon(ui, Icon::Search, theme::SIZE_CHIP, theme::FG_SECONDARY);
                ui.label(SEARCH_LABEL);
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut state.search_q)
                        .desired_width(180.0)
                        .hint_text(SEARCH_HINT),
                );
                if resp.changed() {
                    state.mark_search_edited();
                }
                let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                if search_enter_submits(resp.has_focus(), resp.lost_focus(), enter) {
                    state.on_search_enter();
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) && state.search_popup_open() {
                    state.dismiss_search();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(theme::SPACE_8);
                    status_pill(ui, state.host_status);
                    metrics_chip(ui, state);
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

fn nav_item(
    ui: &mut egui::Ui,
    state: &mut AppState,
    screen: Screen,
    label: &str,
    enabled: bool,
    icon: Icon,
) {
    let selected = state.screen == screen;
    ui.add_enabled_ui(enabled, |ui| {
        let color = if selected {
            theme::FG_PRIMARY
        } else {
            theme::FG_SECONDARY
        };
        theme::show_icon(ui, icon, theme::SIZE_CHIP, color);
        let response = ui.selectable_label(selected, label);
        if response.clicked() {
            state.screen = screen;
        }
    });
}

fn status_pill(ui: &mut egui::Ui, status: HostStatus) {
    let (dot, bg, fg) = match status {
        HostStatus::Connecting => (
            theme::FG_SECONDARY,
            theme::CHIP_KEYBINDING,
            theme::FG_PRIMARY,
        ),
        HostStatus::Online => (theme::ACCENT, theme::BG_PAGE, theme::ACCENT),
        HostStatus::Offline => (
            theme::BANNER_OFFLINE_FG,
            theme::BANNER_OFFLINE_FILL,
            theme::BANNER_OFFLINE_FG,
        ),
    };

    egui::Frame::new()
        .fill(bg)
        .corner_radius(theme::RADIUS_NAV)
        .inner_margin(egui::Margin::symmetric(
            theme::SPACE_8 as i8,
            theme::SPACE_4 as i8,
        ))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(theme::SPACE_8, theme::SPACE_8),
                    egui::Sense::hover(),
                );
                ui.painter()
                    .circle_filled(rect.center(), theme::SPACE_4, dot);
                ui.colored_label(fg, status.label_ru());
            });
        });
}

fn metrics_chip(ui: &mut egui::Ui, state: &crate::state::AppState) {
    let value = state.metrics_chip_value();
    theme::chip_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            theme::show_icon(ui, Icon::Gauge, theme::SIZE_CHIP, theme::FG_SECONDARY);
            ui.colored_label(theme::FG_SECONDARY, format!("{METRICS_LABEL} {value}"));
        });
    });
}
