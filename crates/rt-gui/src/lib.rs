//! Thin desktop client for RustTraycer (eframe + egui).
//! No host spawn, no sqlite, no workspace walk.

mod a2a;
mod account_ux;
mod app;
mod artifacts;
mod chrome;
mod discovery;
mod hooks;
mod ladder;
mod metrics;
mod model_ux;
mod pr_ux;
mod rpc;
mod screens;
mod search_ux;
mod stash;
mod state;
mod sync_ux;
mod terminal;
pub mod theme;
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
            theme::apply(&cc.egui_ctx);
            Ok(Box::new(RtGuiApp::new()))
        }),
    )
}
