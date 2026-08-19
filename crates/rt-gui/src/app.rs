use crate::chrome;
use crate::screens;
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
