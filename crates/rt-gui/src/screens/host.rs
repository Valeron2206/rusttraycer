use crate::discovery;
use crate::hooks::{HOOKS_HINT, HOOKS_LABEL, HOOKS_SAVE};
use crate::state::AppState;
use crate::sync_ux::{
    EXPORT_BUTTON, IMPORT_BUTTON, IMPORT_CONFIRM_BODY, IMPORT_CONFIRM_OK, IMPORT_CONFIRM_TITLE,
    PEER_URL_HINT, PEER_URL_LABEL, PULL_BUTTON, PULL_CONFIRM_BODY, PULL_CONFIRM_OK,
    PULL_CONFIRM_TITLE, PUSH_BUTTON, SYNC_PEER_UNAVAILABLE, SYNC_SECRET_HINT, SYNC_SECTION,
    SYNC_UNAVAILABLE,
};
use crate::workspace_ux::{
    GLOBAL_GUIDE_HINT, GLOBAL_GUIDE_LABEL, GLOBAL_GUIDE_SAVE, WORKSPACE_UNAVAILABLE,
};

pub fn show(ctx: &egui::Context, state: &mut AppState) {
    egui::CentralPanel::default().show(ctx, |ui| {
        show_body(ui, state);
    });
}

pub fn show_body(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading("Host");
    ui.weak("Диагностика. Не облако, не тема, не аккаунты. Refresh только перечитывает pid.json.");
    ui.add_space(10.0);

    ui.horizontal(|ui| {
        if ui.button("Обновить").clicked() {
            state.request_retry();
        }
        let has_id = state.pid_info.is_some();
        ui.add_enabled_ui(has_id, |ui| {
            if ui.button("Скопировать hostId").clicked() && state.copy_host_id(ui.ctx())
            {
                state.copied_flash = Some("hostId скопирован".into());
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
            info.ws_url
                .clone()
                .unwrap_or_else(|| unavailable.to_string()),
            info.started_at
                .clone()
                .unwrap_or_else(|| unavailable.to_string()),
        ),
        None => (
            unavailable.into(),
            unavailable.into(),
            unavailable.into(),
            unavailable.into(),
            unavailable.into(),
        ),
    };

    let host_version = state
        .session
        .as_ref()
        .map(|s| s.host_version.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| unavailable.to_string());

    field(ui, "hostId", &host_id);
    field(ui, "hostVersion", &host_version);
    field(ui, "pid", &pid);
    field(ui, "rpcUrl", &rpc_url);
    field(ui, "wsUrl", &ws_url);
    field(ui, "startedAt", &started);

    let (db_ok, data_dir, db_path, log_path, ws_n, task_n, agent_n) = match &state.doctor {
        Some(doc) => (
            if doc.db_ok {
                "ok".to_string()
            } else {
                "нет".to_string()
            },
            empty_or(&doc.data_dir, unavailable),
            empty_or(&doc.db_path, unavailable),
            empty_or(&doc.log_path, unavailable),
            doc.workspace_count.to_string(),
            doc.task_count.to_string(),
            doc.agent_count.to_string(),
        ),
        None => (
            unavailable.into(),
            unavailable.into(),
            unavailable.into(),
            unavailable.into(),
            unavailable.into(),
            unavailable.into(),
            unavailable.into(),
        ),
    };
    if let Some(doc) = &state.doctor {
        field(ui, "doctor.hostId", &doc.host_id);
        field(ui, "doctor.pid", &doc.pid.to_string());
        field(ui, "doctor.rpcUrl", &doc.rpc_url);
    }
    field(ui, "dbOk", &db_ok);
    field(ui, "dataDir", &data_dir);
    field(ui, "dbPath", &db_path);
    field(ui, "logPath", &log_path);
    field(ui, "workspaceCount", &ws_n);
    field(ui, "taskCount", &task_n);
    field(ui, "agentCount", &agent_n);

    ui.add_space(8.0);
    ui.weak("providers (host.doctor)");
    if state.providers.is_empty() {
        field(ui, "providers", unavailable);
    } else {
        for provider in &state.providers {
            let avail = if provider.available {
                "available"
            } else {
                "нет"
            };
            field(
                ui,
                &format!("provider {}", provider.id),
                &format!("{avail} · {}", provider.detail),
            );
            match &provider.caps {
                Some(caps) => {
                    let flags = format!(
                            "oneShot={} longLived={} streamTokens={} tools={} sessionResume={} a2aInbox={} pty={} needsApiKey={}",
                            caps.one_shot,
                            caps.long_lived,
                            caps.stream_tokens,
                            caps.tools,
                            caps.session_resume,
                            caps.a2a_inbox,
                            caps.pty,
                            caps.needs_api_key
                        );
                    field(ui, "caps", &flags);
                }
                None => field(ui, "caps", ""),
            }
        }
    }

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);
    ui.heading(SYNC_SECTION);
    let sync_ok = state.sync_host_ok();
    if state.can_rpc() && !sync_ok {
        ui.weak(SYNC_UNAVAILABLE);
    }
    if let Some(status) = &state.sync_status {
        if status != SYNC_UNAVAILABLE {
            ui.weak(status);
        }
    }
    ui.add_enabled_ui(sync_ok, |ui| {
        ui.horizontal(|ui| {
            if ui.button(EXPORT_BUTTON).clicked() {
                if let Some((name, payload)) = state.export_sync() {
                    state.save_exported_sync(&name, &payload);
                }
            }
            if ui.button(IMPORT_BUTTON).clicked() {
                state.request_sync_import();
            }
        });
    });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(PEER_URL_LABEL);
        ui.add(
            egui::TextEdit::singleline(&mut state.sync_peer_url)
                .desired_width(280.0)
                .hint_text(PEER_URL_HINT)
                .password(false),
        );
        if ui.button(PUSH_BUTTON).clicked() {
            state.request_sync_push();
        }
        if ui.button(PULL_BUTTON).clicked() {
            state.request_sync_pull();
        }
    });
    ui.weak(SYNC_SECRET_HINT);
    if state.can_rpc() && sync_ok && !state.sync_peer_host_ok() {
        ui.weak(SYNC_PEER_UNAVAILABLE);
    }

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);
    ui.heading(GLOBAL_GUIDE_LABEL);
    state.ensure_settings_guide();
    let host_ok = state.workspace_host_ok();
    if state.can_rpc() && !host_ok {
        ui.weak(WORKSPACE_UNAVAILABLE);
    }
    ui.add_enabled_ui(host_ok, |ui| {
        if !state.settings_guide_path.is_empty() {
            ui.weak(&state.settings_guide_path);
        }
        ui.add(
            egui::TextEdit::multiline(&mut state.settings_guide_draft)
                .desired_width(ui.available_width())
                .desired_rows(8)
                .hint_text(GLOBAL_GUIDE_HINT),
        );
        if ui.button(GLOBAL_GUIDE_SAVE).clicked() {
            state.save_settings_guide();
        }
    });
    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);
    ui.heading(HOOKS_LABEL);
    state.ensure_hooks();
    if !state.hooks_path.is_empty() {
        ui.weak(&state.hooks_path);
    }
    ui.add(
        egui::TextEdit::multiline(&mut state.hooks_draft)
            .desired_width(ui.available_width())
            .desired_rows(8)
            .hint_text(HOOKS_HINT),
    );
    if ui.button(HOOKS_SAVE).clicked() {
        state.save_hooks();
    }
    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);
    ui.weak("Нет: запустить host, смена темы, аккаунты, API keys, список host.");
}

fn empty_or(value: &str, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}

fn field(ui: &mut egui::Ui, name: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [180.0, 18.0],
            egui::Label::new(egui::RichText::new(name).weak()),
        );
        ui.monospace(value);
    });
}

pub fn show_sync_import_confirm(ctx: &egui::Context, state: &mut AppState) {
    if !state.show_sync_import_confirm {
        return;
    }
    let mut open = true;
    egui::Window::new(IMPORT_CONFIRM_TITLE)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(IMPORT_CONFIRM_BODY);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(IMPORT_CONFIRM_OK).clicked() {
                    state.confirm_sync_import();
                }
                if ui.button("Отмена").clicked() {
                    state.cancel_sync_import();
                }
            });
        });
    if !open {
        state.cancel_sync_import();
    }
}

pub fn show_sync_pull_confirm(ctx: &egui::Context, state: &mut AppState) {
    if !state.show_sync_pull_confirm {
        return;
    }
    let mut open = true;
    egui::Window::new(PULL_CONFIRM_TITLE)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(PULL_CONFIRM_BODY);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(PULL_CONFIRM_OK).clicked() {
                    state.confirm_sync_pull();
                }
                if ui.button("Отмена").clicked() {
                    state.cancel_sync_pull();
                }
            });
        });
    if !open {
        state.cancel_sync_pull();
    }
}
