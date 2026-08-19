use crate::chrome;
use crate::screens;
use crate::search_ux::{SEARCH_EMPTY, SEARCH_LABEL};
use crate::state::{AppState, Screen};

pub struct RtGuiApp {
    pub state: AppState,
}

impl RtGuiApp {
    pub fn new() -> Self {
        Self {
            state: AppState::new(),
        }
    }
}

impl eframe::App for RtGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.state.tick_discovery();
        self.state.tick_ws();
        self.state.tick_rpc();
        chrome::show(ctx, &mut self.state);

        match self.state.screen {
            Screen::Tasks => screens::tasks::show(ctx, &mut self.state),
            Screen::Canvas => screens::canvas::show(ctx, &mut self.state),
            Screen::Host => screens::host::show(ctx, &mut self.state),
        }

        screens::tasks::show_new_task_dialog(ctx, &mut self.state);
        screens::tasks::show_rename_dialog(ctx, &mut self.state);
        screens::canvas::show_ladder_dialogs(ctx, &mut self.state);
        screens::canvas::show_write_dialogs(ctx, &mut self.state);
        screens::canvas::show_artifact_dialogs(ctx, &mut self.state);
        screens::host::show_sync_import_confirm(ctx, &mut self.state);
        screens::canvas::show_stash_palette(ctx, &mut self.state);
        show_search_results(ctx, &mut self.state);

        if let Some(toast) = self.state.toast.clone() {
            egui::Window::new("Сообщение")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -16.0])
                .show(ctx, |ui| {
                    ui.label(toast);
                    if ui.button("Закрыть").clicked() {
                        self.state.toast = None;
                    }
                });
        }

        if self.state.wants_repaint() {
            ctx.request_repaint_after(std::time::Duration::from_millis(80));
        }
    }
}

fn show_search_results(ctx: &egui::Context, state: &mut AppState) {
    if !state.search_ran && state.search_items.is_empty() {
        return;
    }
    let mut clicked = None;
    egui::Window::new(SEARCH_LABEL)
        .collapsible(false)
        .resizable(true)
        .default_width(360.0)
        .anchor(egui::Align2::LEFT_TOP, [220.0, 48.0])
        .show(ctx, |ui| {
            if state.search_items.is_empty() {
                ui.weak(SEARCH_EMPTY);
                return;
            }
            for (idx, item) in state.search_items.iter().enumerate() {
                let line = if item.hint.is_empty() {
                    format!("{} · {}", item.kind_label_ru(), item.title)
                } else {
                    format!("{} · {} · {}", item.kind_label_ru(), item.title, item.hint)
                };
                if ui.selectable_label(false, line).clicked() {
                    clicked = Some(idx);
                }
            }
        });
    if let Some(idx) = clicked {
        state.activate_search_result(idx);
    }
}
