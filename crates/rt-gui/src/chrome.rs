use crate::ladder::YOLO_BANNER;
use crate::metrics::METRICS_LABEL;
use crate::search_ux::{search_enter_submits, SEARCH_HINT, SEARCH_LABEL};
use crate::state::{AppState, HostStatus, Screen};

pub fn show(ctx: &egui::Context, state: &mut AppState) {
    egui::TopBottomPanel::top("chrome_nav")
        .exact_height(40.0)
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.add_space(8.0);
                ui.strong("RustTraycer");
                if state.demo {
                    ui.weak("демо");
                }
                ui.separator();

                nav_item(ui, state, Screen::Tasks, "Задачи", true);

                let canvas_label = state
                    .selected_task_title()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| crate::terminal::TERMINALS_PANE.to_string());
                let canvas_enabled = state.selected_task_id.is_some()
                    || !state.open_task_ids.is_empty()
                    || state.has_workspace();
                nav_item(ui, state, Screen::Canvas, &canvas_label, canvas_enabled);

                nav_item(ui, state, Screen::Host, "Host", true);

                ui.separator();
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
                    ui.add_space(10.0);
                    status_pill(ui, state.host_status);
                    metrics_chip(ui, state);
                });
            });
        });

    if state.is_offline() {
        egui::TopBottomPanel::top("offline_banner").show(ctx, |ui| {
            let fill = egui::Color32::from_rgb(92, 28, 28);
            egui::Frame::new()
                .fill(fill)
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.colored_label(
                            egui::Color32::from_rgb(255, 220, 220),
                            "Host не запущен или не отвечает. Поднимите его через CLI (`rt-cli start`) — GUI его не стартует.",
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(egui::Button::new("Повторить").fill(egui::Color32::from_rgb(140, 40, 40)))
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
                .fill(egui::Color32::from_rgb(120, 48, 16))
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.colored_label(egui::Color32::from_rgb(255, 220, 170), YOLO_BANNER);
                });
        });
    }

    if state.is_online() {
        if let Some(msg) = state.ws_banner.clone() {
            egui::TopBottomPanel::top("ws_banner").show(ctx, |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(72, 56, 16))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.colored_label(egui::Color32::from_rgb(255, 230, 170), msg);
                    });
            });
        }
    }
}

fn nav_item(ui: &mut egui::Ui, state: &mut AppState, screen: Screen, label: &str, enabled: bool) {
    let selected = state.screen == screen;
    ui.add_enabled_ui(enabled, |ui| {
        let response = ui.selectable_label(selected, label);
        if response.clicked() {
            state.screen = screen;
        }
    });
}

fn status_pill(ui: &mut egui::Ui, status: HostStatus) {
    let (dot, bg, fg) = match status {
        HostStatus::Connecting => (
            egui::Color32::from_rgb(230, 180, 60),
            egui::Color32::from_rgb(50, 40, 16),
            egui::Color32::from_rgb(240, 210, 120),
        ),
        HostStatus::Online => (
            egui::Color32::from_rgb(80, 200, 120),
            egui::Color32::from_rgb(16, 48, 28),
            egui::Color32::from_rgb(160, 230, 180),
        ),
        HostStatus::Offline => (
            egui::Color32::from_rgb(220, 80, 80),
            egui::Color32::from_rgb(48, 18, 18),
            egui::Color32::from_rgb(240, 170, 170),
        ),
    };

    egui::Frame::new()
        .fill(bg)
        .corner_radius(12.0)
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 4.0, dot);
                ui.colored_label(fg, status.label_ru());
            });
        });
}

fn metrics_chip(ui: &mut egui::Ui, state: &crate::state::AppState) {
    let value = state.metrics_chip_value();
    let bg = egui::Color32::from_rgb(28, 32, 42);
    let fg = egui::Color32::from_rgb(180, 196, 220);
    egui::Frame::new()
        .fill(bg)
        .corner_radius(12.0)
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.colored_label(fg, format!("{METRICS_LABEL} {value}"));
            });
        });
}
