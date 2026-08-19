//! Thin desktop client for RustTraycer (eframe + egui).
//! No host spawn, no sqlite, no workspace walk.

mod a2a;
mod app;
mod artifacts;
mod chrome;
mod discovery;
mod ladder;
mod model_ux;
mod rpc;
mod screens;
mod state;
mod terminal;
mod workspace_ux;
mod ws;

use app::RtGuiApp;

pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 560.0])
            .with_title("RustTraycer")
            .with_app_id("rusttraycer"),
        ..Default::default()
    };

    eframe::run_native(
        "RustTraycer",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            apply_dark_polish(&cc.egui_ctx);
            Ok(Box::new(RtGuiApp::new()))
        }),
    )
}

fn apply_dark_polish(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals.window_fill = egui::Color32::from_rgb(18, 18, 22);
    style.visuals.panel_fill = egui::Color32::from_rgb(22, 22, 26);
    style.visuals.extreme_bg_color = egui::Color32::from_rgb(14, 14, 18);
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    ctx.set_style(style);
}
