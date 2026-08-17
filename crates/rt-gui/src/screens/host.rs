use crate::discovery;
use crate::state::AppState;

pub fn show(ctx: &egui::Context, state: &mut AppState) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Host");
        ui.weak("Диагностика. Не облако, не тема, не аккаунты. Refresh только перечитывает pid.json.");
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            if ui.button("Обновить").clicked() {
                state.request_retry();
            }
            let has_id = state.pid_info.is_some();
            ui.add_enabled_ui(has_id, |ui| {
                if ui.button("Скопировать hostId").clicked() {
                    if state.copy_host_id(ctx) {
                        state.copied_flash = Some("hostId скопирован".into());
                    }
                }
            });
            if let Some(msg) = &state.copied_flash {
                ui.weak(msg);
            }
        });

        ui.add_space(6.0);
        ui.label(format!("файл: {}", discovery::pid_json_path().display()));
        if let Some(err) = &state.discover_error {
            ui.colored_label(egui::Color32::from_rgb(230, 160, 160), err);
        }
        ui.add_space(12.0);

        let unavailable = "недоступно";
        let (host_id, pid, rpc_url, ws_url, started) = match &state.pid_info {
            Some(info) => (
                info.host_id.clone(),
                info.pid.to_string(),
                info.rpc_url.clone(),
                info.ws_url.clone().unwrap_or_else(|| unavailable.to_string()),
                info.started_at.clone().unwrap_or_else(|| unavailable.to_string()),
            ),
            None => (
                unavailable.into(),
                unavailable.into(),
                unavailable.into(),
                unavailable.into(),
                unavailable.into(),
            ),
        };

        field(ui, "hostId", &host_id);
        field(ui, "pid", &pid);
        field(ui, "rpcUrl", &rpc_url);
        field(ui, "wsUrl", &ws_url);
        field(ui, "startedAt", &started);

        // Doctor fields — this slice has no RPC, so they stay empty while offline.
        field(ui, "dbOk", unavailable);
        field(ui, "dataDir", unavailable);
        field(ui, "dbPath", unavailable);
        field(ui, "logPath", unavailable);
        field(ui, "provider cli.generic", unavailable);
        field(ui, "workspaceCount", unavailable);
        field(ui, "taskCount", unavailable);
        field(ui, "agentCount", unavailable);

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);
        ui.weak("Нет: запустить host, смена темы, аккаунты, API keys, список host.");
    });
}

fn field(ui: &mut egui::Ui, name: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.add_sized([180.0, 18.0], egui::Label::new(egui::RichText::new(name).weak()));
        ui.monospace(value);
    });
}
