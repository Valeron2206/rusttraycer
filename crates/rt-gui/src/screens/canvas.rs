use crate::ladder::{
    AgentPolicy, PaneKind, PolicyMode, APPROVAL_ALWAYS, APPROVAL_DENY, APPROVAL_ONCE,
    APPROVAL_TITLE, CAPS_LABEL, COMMIT_BUTTON, COMMIT_HINT, OPEN_IN_EDITOR, PICKER_EMPTY,
    PICKER_HINT, PICKER_LABEL, PICKER_UNAVAILABLE, POLICY_LABEL, PUSH_BUTTON, PUSH_CONFIRM_BODY,
    PUSH_CONFIRM_OK, PUSH_CONFIRM_TITLE, REVERT_BUTTON, STAGE_BUTTON, UNSTAGE_BUTTON,
    YOLO_CONFIRM_BODY, YOLO_CONFIRM_OK, YOLO_CONFIRM_TITLE, YOLO_OFF, YOLO_ON_BUTTON,
};
use crate::rpc::HarnessCapsView;
use crate::state::{AgentStatus, AppState, FileKind, FilePreview};
use crate::terminal::{
    self, AgentInterface, AgentView, AGENT_IS_CHAT, CHAT_TAB, CLOSE_TERMINAL, INTERFACE_LABEL,
    NEW_TERMINAL, NO_LIVE_SHELL, OPEN_PTY, PTY_HINT, PTY_INPUT_HINT, PTY_SUBMIT, SHELL_HINT,
    TERMINALS_PANE, TERMINAL_DISABLED_CAPS, TERMINAL_TAB, TERMINAL_UNAVAILABLE,
};

pub fn show(ctx: &egui::Context, state: &mut AppState) {
    if state.selected_task_id.is_none() && state.open_task_ids.is_empty() {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.centered_and_justified(|ui| {
                ui.weak("Задача не выбрана. Вернитесь к списку «Задачи».");
            });
        });
        return;
    }

    show_task_tabs(ctx, state);

    egui::TopBottomPanel::top("canvas_header")
        .exact_height(36.0)
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                let title = state.selected_task_title().unwrap_or("Task").to_string();
                ui.strong(title);
                ui.separator();
                ui.weak(format!("host {}", state.host_id_prefix()));
                ui.separator();
                match state.selected_agent() {
                    Some(agent) => {
                        ui.label(format!(
                            "агент: {} · {}",
                            agent.status.label_ru(),
                            agent.provider
                        ));
                    }
                    None => {
                        ui.weak("агент не создан");
                    }
                }
            });
        });

    egui::SidePanel::left("canvas_sidebar")
        .resizable(true)
        .default_width(280.0)
        .min_width(220.0)
        .show(ctx, |ui| {
            show_agents(ui, state);
        });

    let left_width = state.split.left_width;
    egui::SidePanel::left("split_left")
        .resizable(true)
        .default_width(left_width)
        .min_width(220.0)
        .show(ctx, |ui| {
            show_pane(ui, ctx, state, "left", state.split.left);
            let w = ui.max_rect().width();
            if (w - state.split.left_width).abs() > 8.0 {
                state.split.left_width = w;
                state.persist_split();
            }
        });

    egui::CentralPanel::default().show(ctx, |ui| {
        show_pane(ui, ctx, state, "right", state.split.right);
    });
}

pub fn show_ladder_dialogs(ctx: &egui::Context, state: &mut AppState) {
    show_yolo_confirm(ctx, state);
    show_approval_card(ctx, state);
}

pub fn show_write_dialogs(ctx: &egui::Context, state: &mut AppState) {
    show_push_confirm(ctx, state);
}

fn show_task_tabs(ctx: &egui::Context, state: &mut AppState) {
    if state.open_task_ids.is_empty() {
        return;
    }
    let tabs: Vec<(String, String)> = state
        .open_task_ids
        .iter()
        .map(|id| {
            let title = state
                .tasks
                .iter()
                .find(|t| &t.id == id)
                .map(|t| t.title.clone())
                .unwrap_or_else(|| id.clone());
            (id.clone(), title)
        })
        .collect();
    let selected = state.selected_task_id.clone();
    let mut switch = None;
    let mut close = None;
    egui::TopBottomPanel::top("task_tabs")
        .exact_height(32.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.weak("Задачи:");
                for (id, title) in &tabs {
                    let is_sel = selected.as_deref() == Some(id.as_str());
                    if ui.selectable_label(is_sel, title).clicked() {
                        switch = Some(id.clone());
                    }
                    if ui
                        .small_button("×")
                        .on_hover_text("Закрыть вкладку")
                        .clicked()
                    {
                        close = Some(id.clone());
                    }
                    ui.separator();
                }
            });
        });
    if let Some(id) = switch {
        state.switch_task_tab(id);
    }
    if let Some(id) = close {
        state.close_task_tab(&id);
    }
}

fn show_pane(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    state: &mut AppState,
    side: &str,
    kind: PaneKind,
) {
    ui.horizontal(|ui| {
        ui.weak("вид");
        let mut next = kind;
        egui::ComboBox::from_id_salt(format!("pane_{side}"))
            .selected_text(kind.label_ru())
            .show_ui(ui, |ui| {
                for pane in PaneKind::ALL {
                    ui.selectable_value(&mut next, pane, pane.label_ru());
                }
            });
        if next != kind {
            state.set_split_pane(side, next);
        }
    });
    ui.separator();
    match kind {
        PaneKind::Canvas => show_agent_panel(ui, state),
        PaneKind::Git => show_git(ui, state),
        PaneKind::Files => {
            show_file_tree(ui, state);
            ui.add_space(8.0);
            ui.separator();
            show_preview(ui, ctx, state);
        }
        PaneKind::Host => crate::screens::host::show_body(ui, state),
        PaneKind::Terminal => show_shells(ui, state),
    }
}

fn show_yolo_confirm(ctx: &egui::Context, state: &mut AppState) {
    if !state.show_yolo_confirm {
        return;
    }
    let mut open = true;
    egui::Window::new(YOLO_CONFIRM_TITLE)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(YOLO_CONFIRM_BODY);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(YOLO_CONFIRM_OK).clicked() {
                    state.confirm_yolo();
                }
                if ui.button("Отмена").clicked() {
                    state.cancel_yolo_confirm();
                }
            });
        });
    if !open {
        state.cancel_yolo_confirm();
    }
}

fn show_push_confirm(ctx: &egui::Context, state: &mut AppState) {
    if !state.show_push_confirm {
        return;
    }
    let mut open = true;
    egui::Window::new(PUSH_CONFIRM_TITLE)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(PUSH_CONFIRM_BODY);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(PUSH_CONFIRM_OK).clicked() {
                    state.confirm_push();
                }
                if ui.button("Отмена").clicked() {
                    state.cancel_push_confirm();
                }
            });
        });
    if !open {
        state.cancel_push_confirm();
    }
}

fn show_approval_card(ctx: &egui::Context, state: &mut AppState) {
    let Some(approval) = state.selected_approval().cloned() else {
        return;
    };
    let mut open = true;
    egui::Window::new(APPROVAL_TITLE)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 24.0])
        .show(ctx, |ui| {
            ui.label(format!("{} · {}", approval.kind, approval.summary));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(APPROVAL_ONCE).clicked() {
                    state.respond_approval("allow-once");
                }
                if ui.button(APPROVAL_ALWAYS).clicked() {
                    state.respond_approval("allow-always");
                }
                if ui.button(APPROVAL_DENY).clicked() {
                    state.respond_approval("deny");
                }
            });
        });
    if !open {
        // Title-bar X is deny, not a silent dismiss.
        state.close_approval_card();
    }
}

fn show_agents(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading("Агенты");
    ui.add_space(4.0);
    show_provider_picker(ui, state);
    ui.add_space(6.0);
    show_interface_picker(ui, state);
    ui.add_space(8.0);
    show_policy_controls(ui, state);
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(6.0);

    let agents: Vec<(String, String, AgentStatus, String)> = state
        .agents_for_selected_task()
        .into_iter()
        .map(|a| {
            (
                a.id.clone(),
                a.provider.clone(),
                a.status,
                a.interface.clone(),
            )
        })
        .collect();
    let selected = state.selected_agent().map(|a| a.id.clone());

    if agents.is_empty() {
        ui.label("Агента ещё нет.");
    } else {
        for (id, provider, status, interface) in &agents {
            let is_sel = selected.as_deref() == Some(id.as_str());
            let resp = egui::Frame::new()
                .fill(if is_sel {
                    egui::Color32::from_rgb(40, 48, 64)
                } else {
                    egui::Color32::from_rgb(32, 32, 38)
                })
                .inner_margin(egui::Margin::same(8))
                .corner_radius(6.0)
                .show(ui, |ui| {
                    ui.strong(provider);
                    ui.label(format!("статус: {}", status.label_ru()));
                    ui.weak(AgentInterface::from_wire(interface).label_ru());
                    ui.weak(id);
                    ui.weak("нажмите, чтобы выбрать");
                });
            if resp.response.interact(egui::Sense::click()).clicked() {
                state.select_agent(id.clone());
            }
        }
    }

    ui.add_space(6.0);
    ui.add_enabled_ui(state.can_create_agent(), |ui| {
        if ui
            .add_sized(
                [ui.available_width(), 28.0],
                egui::Button::new("Создать агента"),
            )
            .clicked()
        {
            state.create_agent();
        }
    });
    if !state.can_rpc() {
        ui.weak("недоступно: host offline");
    } else if state.providers.is_empty() {
        ui.weak(PICKER_EMPTY);
    } else if state.picker_provider.is_none() {
        ui.weak(PICKER_HINT);
    }
}

fn show_provider_picker(ui: &mut egui::Ui, state: &mut AppState) {
    ui.label(PICKER_LABEL);
    let providers: Vec<(String, bool, String, Option<HarnessCapsView>)> = state
        .providers
        .iter()
        .map(|p| (p.id.clone(), p.available, p.detail.clone(), p.caps.clone()))
        .collect();
    let current = state.picker_provider.clone();
    if providers.is_empty() {
        ui.weak(PICKER_EMPTY);
    } else {
        let selected_text = current.clone().unwrap_or_else(|| PICKER_HINT.into());
        let mut next = current.clone();
        egui::ComboBox::from_id_salt("provider_picker")
            .selected_text(selected_text)
            .show_ui(ui, |ui| {
                for (id, available, _, _) in &providers {
                    let label = if *available {
                        id.clone()
                    } else {
                        format!("{id} (недоступен)")
                    };
                    ui.selectable_value(&mut next, Some(id.clone()), label);
                }
            });
        if let Some(id) = next {
            if current.as_deref() != Some(id.as_str()) {
                state.set_picker_provider(id);
            }
        }
        if let Some((_, available, detail, _)) = providers
            .iter()
            .find(|(id, _, _, _)| current.as_deref() == Some(id.as_str()))
        {
            if !*available {
                ui.weak(PICKER_UNAVAILABLE);
            }
            if !detail.is_empty() {
                ui.weak(detail);
            }
        }
    }
    ui.add_space(4.0);
    ui.weak(CAPS_LABEL);
    egui::Frame::new()
        .fill(egui::Color32::from_rgb(28, 28, 34))
        .inner_margin(egui::Margin::same(8))
        .corner_radius(4.0)
        .show(ui, |ui| {
            ui.set_min_height(36.0);
            ui.set_width(ui.available_width());
            if let Some(caps) = state.selected_provider().and_then(|p| p.caps.as_ref()) {
                show_caps(ui, caps);
            }
        });
}

fn show_caps(ui: &mut egui::Ui, caps: &HarnessCapsView) {
    let flags = [
        ("oneShot", caps.one_shot, false),
        ("longLived", caps.long_lived, false),
        ("streamTokens", caps.stream_tokens, false),
        ("tools", caps.tools, false),
        ("sessionResume", caps.session_resume, false),
        ("a2aInbox", caps.a2a_inbox, true),
        ("pty", caps.pty, true),
        ("needsApiKey", caps.needs_api_key, false),
    ];
    for (name, on, grey) in flags {
        let color = if grey {
            egui::Color32::from_rgb(140, 140, 150)
        } else if on {
            egui::Color32::from_rgb(180, 210, 180)
        } else {
            egui::Color32::from_rgb(150, 150, 156)
        };
        let mark = if on { "●" } else { "○" };
        ui.colored_label(color, format!("{mark} {name}"));
    }
    if let Some(env) = &caps.api_key_env {
        ui.weak(format!("apiKeyEnv {env}"));
    }
}

fn show_policy_controls(ui: &mut egui::Ui, state: &mut AppState) {
    ui.label(POLICY_LABEL);
    let policy: AgentPolicy = state.selected_policy();
    let has_agent = state.selected_agent().is_some();
    ui.add_enabled_ui(has_agent && state.can_rpc(), |ui| {
        let mut next = policy.mode;
        egui::ComboBox::from_id_salt("policy_mode")
            .selected_text(policy.mode.label_ru())
            .show_ui(ui, |ui| {
                for mode in PolicyMode::ALL {
                    ui.selectable_value(&mut next, mode, mode.label_ru());
                }
            });
        if next != policy.mode {
            state.set_policy_mode(next);
        }
        ui.horizontal(|ui| {
            if policy.yolo {
                if ui.button(YOLO_OFF).clicked() {
                    state.set_yolo_off();
                }
            } else if ui.button(YOLO_ON_BUTTON).clicked() {
                state.request_yolo_on();
            }
        });
    });
    if let Some(status) = &state.ladder_status {
        ui.weak(status.clone());
    } else if !has_agent {
        ui.weak("сначала создайте агента");
    }
}

fn show_git(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading("Git");
    ui.weak("host git · GUI git не спавнит");
    ui.add_space(4.0);
    ui.add_enabled_ui(state.can_isolate_agent(), |ui| {
        if ui
            .add_sized(
                [ui.available_width(), 28.0],
                egui::Button::new("Изолировать"),
            )
            .clicked()
        {
            state.isolate_selected_agent();
        }
    });
    if let Some(wt) = &state.worktree {
        ui.weak(format!(
            "worktree {} · {} · {} · {}",
            wt.branch, wt.id, wt.path, wt.created_at
        ));
    } else {
        ui.weak("worktree нет (local)");
    }
    if let Some(note) = &state.git_note {
        ui.label(note.clone());
    }
    if let Some(status) = &state.write_status {
        ui.weak(status.clone());
    }
    if let Some(status) = state.git_status.clone() {
        ui.label(format!(
            "{}{}",
            status.branch,
            if status.dirty {
                " · dirty"
            } else {
                " · clean"
            }
        ));
        if status.truncated {
            ui.weak("список усечён");
        }
        let selected = state.git_selected_path.clone();
        let mut stage_path = None;
        let mut unstage_path = None;
        let mut select_path = None;
        for entry in &status.entries {
            ui.horizontal(|ui| {
                let mut checked = state.git_staged.contains(&entry.path);
                if ui.checkbox(&mut checked, "").changed() {
                    if checked {
                        stage_path = Some(entry.path.clone());
                    } else {
                        unstage_path = Some(entry.path.clone());
                    }
                }
                let label = format!("{}  {}", entry.status, entry.path);
                if ui
                    .selectable_label(selected.as_deref() == Some(entry.path.as_str()), label)
                    .clicked()
                {
                    select_path = Some(entry.path.clone());
                }
            });
        }
        if let Some(path) = select_path {
            state.select_git_path(path);
        }
        if let Some(path) = stage_path {
            state.stage_paths(vec![path]);
        }
        if let Some(path) = unstage_path {
            state.unstage_paths(vec![path]);
        }
    }
    ui.add_space(6.0);
    ui.add(
        egui::TextEdit::singleline(&mut state.git_commit_message)
            .desired_width(f32::INFINITY)
            .hint_text(COMMIT_HINT),
    );
    ui.horizontal(|ui| {
        if ui.button(COMMIT_BUTTON).clicked() {
            state.commit_git();
        }
        if ui.button(PUSH_BUTTON).clicked() {
            state.request_push();
        }
    });
    ui.add_space(6.0);
    ui.strong("diff");
    ui.horizontal(|ui| {
        if ui.button(STAGE_BUTTON).clicked() {
            if let Some(path) = state.git_selected_path.clone() {
                state.stage_paths(vec![path]);
            }
        }
        if ui.button(UNSTAGE_BUTTON).clicked() {
            if let Some(path) = state.git_selected_path.clone() {
                state.unstage_paths(vec![path]);
            }
        }
        if ui.button(REVERT_BUTTON).clicked() {
            state.restore_selected();
        }
    });
    if let Some(diff) = &state.git_diff {
        if diff.truncated {
            ui.weak("патч усечён");
        }
        egui::ScrollArea::vertical()
            .max_height(180.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                if diff.files.is_empty() {
                    ui.weak("нет diff");
                }
                for file in &diff.files {
                    ui.weak(&file.path);
                    match &file.patch {
                        Some(patch) => {
                            ui.add(
                                egui::Label::new(egui::RichText::new(patch).monospace())
                                    .wrap()
                                    .selectable(true),
                            );
                        }
                        None => {
                            ui.weak("(binary)");
                        }
                    }
                }
            });
    }
}

fn show_file_tree(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading("Файлы");
    ui.weak("без std::fs · превью RO · редактор внешний");
    ui.add_space(4.0);
    ui.add_enabled_ui(state.selected_file.is_some() && state.can_rpc(), |ui| {
        if ui.button(OPEN_IN_EDITOR).clicked() {
            if let Some(path) = state.selected_file.clone() {
                state.open_in_editor(path);
            }
        }
    });
    ui.add_space(4.0);

    if !state.can_rpc() && state.file_tree.is_empty() {
        ui.label("нет данных (host offline)");
        return;
    }
    if state.file_tree.is_empty() {
        ui.label("пусто");
        return;
    }

    enum Action {
        Toggle(String),
        Open(String),
    }
    let mut action = None;
    show_nodes(ui, state, &state.file_tree.clone(), &mut action);
    if state.file_tree_truncated {
        ui.weak("список усечён");
    }
    match action {
        Some(Action::Toggle(path)) => state.toggle_dir(path),
        Some(Action::Open(path)) => state.open_file(path),
        None => {}
    }

    fn show_nodes(
        ui: &mut egui::Ui,
        state: &AppState,
        nodes: &[crate::state::FileNode],
        action: &mut Option<Action>,
    ) {
        for node in nodes {
            match node.kind {
                FileKind::Dir => {
                    let expanded = state.file_expanded.contains(&node.path);
                    let icon = if expanded { "📂" } else { "📁" };
                    if ui
                        .selectable_label(false, format!("{icon}  {}", node.name))
                        .clicked()
                    {
                        *action = Some(Action::Toggle(node.path.clone()));
                    }
                    if expanded {
                        if let Some(kids) = state.file_children.get(&node.path) {
                            let kids = kids.clone();
                            ui.indent(egui::Id::new(("dir", &node.path)), |ui| {
                                show_nodes(ui, state, &kids, action);
                            });
                        }
                    }
                }
                FileKind::File => {
                    let selected = state.selected_file.as_deref() == Some(node.path.as_str());
                    if ui
                        .selectable_label(selected, format!("📄  {}", node.name))
                        .clicked()
                    {
                        *action = Some(Action::Open(node.path.clone()));
                    }
                }
            }
        }
    }
}

fn show_preview(ui: &mut egui::Ui, ctx: &egui::Context, state: &mut AppState) {
    ui.heading("Превью");
    ui.add_space(6.0);
    let mut open_editor = None;
    match &state.file_preview {
        Some(FilePreview::Text {
            path,
            content,
            truncated,
        }) => {
            ui.label(path);
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.small_button("Копировать путь").clicked() {
                    ctx.copy_text(path.clone());
                    state.copied_flash = Some("путь скопирован".into());
                }
                if ui.small_button(OPEN_IN_EDITOR).clicked() {
                    open_editor = Some(path.clone());
                }
            });
            if *truncated {
                ui.weak("файл усечён");
            }
            ui.add_space(6.0);
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.monospace(content);
                });
        }
        Some(FilePreview::Message { path, text }) => {
            ui.label(path);
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.small_button("Копировать путь").clicked() {
                    ctx.copy_text(path.clone());
                    state.copied_flash = Some("путь скопирован".into());
                }
                if ui.small_button(OPEN_IN_EDITOR).clicked() {
                    open_editor = Some(path.clone());
                }
            });
            ui.add_space(8.0);
            ui.colored_label(egui::Color32::from_rgb(230, 180, 140), text);
        }
        None => match &state.selected_file {
            Some(path) => {
                ui.label(path);
                ui.add_space(8.0);
                ui.weak("Выберите файл ещё раз, чтобы прочитать через host.");
            }
            None => {
                ui.weak("Выберите файл в дереве. Превью — сплит, не модалка.");
            }
        },
    }
    if let Some(path) = open_editor {
        state.open_in_editor(path);
    }
}

fn show_interface_picker(ui: &mut egui::Ui, state: &mut AppState) {
    ui.label(INTERFACE_LABEL);
    let current = state.picker_interface;
    let pty_ok = state.picker_allows_terminal();
    ui.horizontal(|ui| {
        if ui
            .selectable_label(current == AgentInterface::Chat, CHAT_TAB)
            .clicked()
        {
            state.set_picker_interface(AgentInterface::Chat);
        }
        ui.add_enabled_ui(pty_ok, |ui| {
            if ui
                .selectable_label(current == AgentInterface::Terminal, TERMINAL_TAB)
                .clicked()
            {
                state.set_picker_interface(AgentInterface::Terminal);
            }
        });
    });
    if !pty_ok {
        ui.weak(TERMINAL_DISABLED_CAPS);
    }
}

fn show_agent_panel(ui: &mut egui::Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        let view = state.agent_view;
        if ui
            .selectable_label(view == AgentView::Chat, CHAT_TAB)
            .clicked()
        {
            state.set_agent_view(AgentView::Chat);
        }
        if ui
            .selectable_label(view == AgentView::Terminal, TERMINAL_TAB)
            .clicked()
        {
            state.set_agent_view(AgentView::Terminal);
        }
    });
    ui.add_space(4.0);
    match state.agent_view {
        AgentView::Chat => show_chat(ui, state),
        AgentView::Terminal => show_agent_terminal(ui, state),
    }
}

fn show_agent_terminal(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading(TERMINAL_TAB);
    ui.weak(PTY_HINT);
    if let Some(status) = state.terminal_status.clone() {
        ui.weak(status);
    }
    let is_terminal = state.selected_agent().is_some_and(|a| a.is_terminal());
    if state.selected_agent().is_none() {
        ui.weak("сначала создайте агента");
        return;
    }
    if !is_terminal {
        ui.weak(AGENT_IS_CHAT);
        return;
    }
    if !state.terminal_host_ok() {
        ui.weak(TERMINAL_UNAVAILABLE);
        return;
    }
    if state.selected_agent_pty_id().is_none() {
        if ui.button(OPEN_PTY).clicked() {
            state.ensure_agent_pty();
        }
        return;
    }
    if let Some(pty_id) = state.selected_agent_pty_id().map(str::to_owned) {
        show_pty_view(ui, state, &pty_id);
    }
}

fn show_shells(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading(TERMINALS_PANE);
    ui.weak(SHELL_HINT);
    ui.weak(PTY_HINT);
    if let Some(status) = state.terminal_status.clone() {
        ui.weak(status);
    } else if !state.terminal_host_ok() && state.can_rpc() {
        ui.weak(TERMINAL_UNAVAILABLE);
    }
    ui.add_space(4.0);
    ui.add_enabled_ui(state.can_create_shell() && state.terminal_host_ok(), |ui| {
        if ui
            .add_sized(
                [ui.available_width(), 28.0],
                egui::Button::new(NEW_TERMINAL),
            )
            .clicked()
        {
            state.create_shell();
        }
    });
    if state.selected_task_id.is_none() {
        ui.weak(terminal::NEED_TASK);
    } else if !state.terminal_host_ok() && state.can_rpc() {
        ui.weak(TERMINAL_UNAVAILABLE);
    }
    ui.add_space(6.0);
    let shells: Vec<(String, String)> = state
        .shells
        .iter()
        .map(|s| {
            let cwd = if s.cwd.is_empty() {
                s.id.clone()
            } else {
                format!("{} · {}", s.id, s.cwd)
            };
            (s.id.clone(), cwd)
        })
        .collect();
    let selected = state.selected_shell_id.clone();
    if shells.is_empty() {
        ui.weak(NO_LIVE_SHELL);
    } else {
        for (id, label) in &shells {
            if ui
                .selectable_label(selected.as_deref() == Some(id.as_str()), label)
                .clicked()
            {
                state.select_shell(id.clone());
            }
        }
        ui.add_space(4.0);
        if ui.button(CLOSE_TERMINAL).clicked() {
            state.close_selected_shell();
        }
    }
    ui.add_space(8.0);
    ui.separator();
    if state.selected_shell().is_some() && state.selected_shell_pty_id().is_none() {
        state.ensure_shell_pty();
    }
    if let Some(pty_id) = state.selected_shell_pty_id().map(str::to_owned) {
        show_pty_view(ui, state, &pty_id);
    }
}

fn show_pty_view(ui: &mut egui::Ui, state: &mut AppState, pty_id: &str) {
    let avail = ui.available_size();
    let (cols, rows) = terminal::estimate_pty_size(avail.x, (avail.y - 52.0).max(32.0));
    state.maybe_resize_pty(pty_id, cols, rows);
    let scrollback = state.pty_scrollback(pty_id).to_string();
    let input_h = 52.0;
    let view_h = (ui.available_height() - input_h).max(80.0);
    egui::ScrollArea::vertical()
        .max_height(view_h)
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            if scrollback.is_empty() {
                ui.weak("PTY scrollback пуст. Это не чат и не messages.");
            } else {
                ui.add(
                    egui::Label::new(egui::RichText::new(scrollback).monospace())
                        .wrap()
                        .selectable(true),
                );
            }
        });
    ui.separator();
    ui.horizontal(|ui| {
        let resp = ui.add(
            egui::TextEdit::singleline(&mut state.pty_input)
                .desired_width(ui.available_width() - 72.0)
                .hint_text(PTY_INPUT_HINT)
                .font(egui::TextStyle::Monospace),
        );
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui.button(PTY_SUBMIT).clicked() || enter {
            state.submit_pty_input(pty_id);
            resp.request_focus();
        }
    });
}

fn show_chat(ui: &mut egui::Ui, state: &mut AppState) {
    let composer_h = 88.0;
    let avail = ui.available_height();
    let transcript_h = (avail - composer_h).max(80.0);

    ui.heading(CHAT_TAB);
    ui.add_space(4.0);

    egui::ScrollArea::vertical()
        .max_height(transcript_h - 28.0)
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            if state.messages.is_empty() {
                ui.add_space(24.0);
                ui.vertical_centered(|ui| {
                    ui.weak("Нет сообщений. Транскрипт живёт на host; после reconnect его подтянет agent.get_context.");
                });
            } else {
                for msg in &state.messages {
                    bubble(ui, &msg.id, &msg.role, &msg.content);
                    ui.add_space(6.0);
                }
            }
        });

    ui.separator();
    ui.add_space(4.0);

    let enabled = state.composer_enabled();
    let reason = state.composer_disabled_reason();
    let show_stop = state.show_stop_button();
    ui.add_enabled_ui(enabled, |ui| {
        ui.add(
            egui::TextEdit::multiline(&mut state.composer_text)
                .desired_width(f32::INFINITY)
                .desired_rows(3)
                .hint_text("Написать сообщение…"),
        );
    });
    ui.horizontal(|ui| {
        ui.add_enabled_ui(enabled, |ui| {
            if ui.button("Отправить").clicked() {
                state.send_composer();
            }
        });
        if show_stop && ui.button("Стоп").clicked() {
            state.cancel_running_agent();
        }
        ui.weak("один активный turn · очередь не строится");
    });
    if let Some(reason) = reason {
        ui.weak(reason);
    }
}

fn bubble(ui: &mut egui::Ui, _id: &str, role: &str, content: &str) {
    let (label, fill, align_right) = match role {
        "user" => ("вы", egui::Color32::from_rgb(28, 48, 80), true),
        "assistant" => ("агент", egui::Color32::from_rgb(36, 36, 42), false),
        "tool" => ("tool", egui::Color32::from_rgb(40, 36, 24), false),
        _ => ("system", egui::Color32::from_rgb(32, 32, 32), false),
    };

    let layout = if align_right {
        egui::Layout::right_to_left(egui::Align::Min)
    } else {
        egui::Layout::left_to_right(egui::Align::Min)
    };

    ui.with_layout(layout, |ui| {
        egui::Frame::new()
            .fill(fill)
            .corner_radius(8.0)
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width() * 0.85);
                ui.weak(label);
                ui.label(content);
            });
    });
}
