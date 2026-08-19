use crate::state::{AgentStatus, AppState, FileKind, FilePreview};

pub fn show(ctx: &egui::Context, state: &mut AppState) {
    if state.selected_task_id.is_none() {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.centered_and_justified(|ui| {
                ui.weak("Задача не выбрана. Вернитесь к списку «Задачи».");
            });
        });
        return;
    }

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
        .default_width(260.0)
        .min_width(200.0)
        .show(ctx, |ui| {
            show_agents(ui, state);
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            show_file_tree(ui, state);
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            show_git(ui, state);
        });

    egui::SidePanel::right("canvas_preview")
        .resizable(true)
        .default_width(280.0)
        .min_width(160.0)
        .show(ctx, |ui| {
            show_preview(ui, ctx, state);
        });

    egui::CentralPanel::default().show(ctx, |ui| {
        show_chat(ui, state);
    });
}

fn show_agents(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading("Агенты");
    ui.add_space(4.0);

    let agents: Vec<(String, String, AgentStatus)> = state
        .agents_for_selected_task()
        .into_iter()
        .map(|a| (a.id.clone(), a.provider.clone(), a.status))
        .collect();
    let selected = state.selected_agent_id.clone();

    if agents.is_empty() {
        ui.label("Агента ещё нет.");
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
        } else {
            ui.weak("Провайдер MVP: cli.generic. Один агент на задачу.");
        }
    } else {
        let many = agents.len() > 1;
        for (id, provider, status) in &agents {
            let is_sel = selected.as_deref() == Some(id.as_str()) || (!many && selected.is_none());
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
                    ui.weak(id);
                    if many {
                        ui.weak("нажмите, чтобы выбрать");
                    }
                });
            if many && resp.response.interact(egui::Sense::click()).clicked() {
                state.select_agent(id.clone());
            }
        }
        ui.add_space(4.0);
        if agents.len() == 1 {
            ui.weak("Второй агент в UI не предлагается.");
        } else {
            ui.weak("Несколько агентов (созданы вне GUI). Чат — у выбранного.");
        }
    }
}

fn show_git(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading("Git");
    ui.weak("только чтение · git не спавним");
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
        ui.weak(format!("worktree {} · {}", wt.branch, wt.id));
    } else {
        ui.weak("worktree нет (local)");
    }
    if let Some(note) = &state.git_note {
        ui.label(note.clone());
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
        for entry in status.entries {
            let label = format!("{}  {}", entry.status, entry.path);
            if ui
                .selectable_label(selected.as_deref() == Some(entry.path.as_str()), label)
                .clicked()
            {
                state.select_git_path(entry.path);
            }
        }
    }
    if let Some(diff) = &state.git_diff {
        ui.add_space(6.0);
        ui.strong("diff");
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
    ui.weak("только чтение, без std::fs");
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
    match &state.file_preview {
        Some(FilePreview::Text {
            path,
            content,
            truncated,
        }) => {
            ui.label(path);
            ui.add_space(4.0);
            if ui.small_button("Копировать путь").clicked() {
                ctx.copy_text(path.clone());
                state.copied_flash = Some("путь скопирован".into());
            }
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
            if ui.small_button("Копировать путь").clicked() {
                ctx.copy_text(path.clone());
                state.copied_flash = Some("путь скопирован".into());
            }
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
}

fn show_chat(ui: &mut egui::Ui, state: &mut AppState) {
    let composer_h = 88.0;
    let avail = ui.available_height();
    let transcript_h = (avail - composer_h).max(80.0);

    ui.heading("Чат");
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
        if show_stop {
            if ui.button("Стоп").clicked() {
                state.cancel_running_agent();
            }
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
