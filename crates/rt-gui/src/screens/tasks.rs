use crate::search_ux::{search_enter_submits, SEARCH_HINT, SEARCH_LABEL};
use crate::state::{AppState, TaskFilter, TaskStatus};
use crate::theme::{self, Icon};
use crate::workspace_ux::{
    self, preset_combo_label, role_label_ru, PRESETS_UNAVAILABLE, PRESET_CREATE, PRESET_DELETE,
    PRESET_DELETE_BODY, PRESET_DELETE_OK, PRESET_DELETE_TITLE, PRESET_LABEL, PRESET_NAME_HINT,
    PRESET_NAME_LABEL, PRESET_NONE, PRESET_PROMPT_LABEL, PRESET_SAVE, PRESET_TITLE_HINT_LABEL,
    ROLE_LABEL, WORKSPACE_UNAVAILABLE,
};

pub fn show(ctx: &egui::Context, state: &mut AppState) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Задачи");
        ui.add_space(theme::SPACE_8);

        ui.horizontal(|ui| {
            ui.label("Фильтр:");
            if ui
                .selectable_label(state.task_filter == TaskFilter::Open, "Открытые")
                .clicked()
            {
                state.set_task_filter(TaskFilter::Open);
            }
            if ui
                .selectable_label(state.task_filter == TaskFilter::Archived, "Архив")
                .clicked()
            {
                state.set_task_filter(TaskFilter::Archived);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let create_enabled = state.can_create_task();
                ui.add_enabled_ui(create_enabled, |ui| {
                    if ui.button("Новая задача").clicked() {
                        state.show_new_task_dialog = true;
                        state.new_task_title.clear();
                    }
                });
                if !create_enabled {
                    ui.weak(create_disabled_hint(state));
                }
            });
        });

        ui.add_space(theme::SPACE_8);
        ui.horizontal(|ui| {
            theme::show_icon(ui, Icon::Search, theme::SIZE_CHIP, theme::FG_SECONDARY);
            ui.label(SEARCH_LABEL);
            let resp = ui.add(
                egui::TextEdit::singleline(&mut state.search_q)
                    .desired_width(220.0)
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
        });

        ui.add_space(theme::SPACE_8);
        ui.separator();
        ui.add_space(theme::SPACE_8);

        let filtered_empty = state.filtered_tasks().is_empty();
        if filtered_empty {
            show_empty_state(ui, state);
        } else {
            show_task_list(ui, state);
        }
    });
}

fn create_disabled_hint(state: &AppState) -> &'static str {
    if !state.can_rpc() {
        "создание недоступно: нет host"
    } else if !state.has_workspace() {
        "сначала добавьте папку"
    } else {
        "создание недоступно"
    }
}

fn show_empty_state(ui: &mut egui::Ui, state: &mut AppState) {
    // Three DISTINCT empties — not one generic "пусто".
    if !state.is_online() && state.tasks.is_empty() {
        empty_no_host(ui);
        return;
    }
    if state.is_online() && !state.has_workspace() {
        empty_no_workspace(ui, state);
        return;
    }
    empty_no_tasks(ui, state);
}

fn empty_card(
    ui: &mut egui::Ui,
    title: &str,
    body: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    ui.vertical_centered(|ui| {
        ui.add_space(theme::SPACE_48);
        theme::card_frame().show(ui, |ui| {
            ui.set_max_width(520.0);
            ui.heading(title);
            ui.add_space(theme::SPACE_8);
            ui.label(body);
            ui.add_space(theme::SPACE_12);
            add_contents(ui);
        });
    });
}

fn empty_no_host(ui: &mut egui::Ui) {
    empty_card(
        ui,
        "Нет host",
        "Host не запущен или не отвечает. Поднимите его через CLI (`rt-cli start`). GUI его не стартует и не умеет поднять процесс. Кнопка «Новая задача» выключена.",
        |ui| {
            ui.weak("Повторить — в баннере сверху. Discovery: ~/.rusttraycer/host/pid.json");
        },
    );
}

fn empty_no_workspace(ui: &mut egui::Ui, state: &mut AppState) {
    empty_card(
        ui,
        "Нет рабочей папки",
        "Host онлайн, но workspace ещё не добавлен. Без папки нельзя создать задачу. GUI не читает содержимое папки с диска — только передаёт абсолютный путь на host.",
        |ui| {
            if ui
                .add_sized([theme::CTA_WIDTH, theme::CTA_HEIGHT], theme::primary_button("Добавить папку"))
                .clicked()
            {
                state.pick_workspace_folder();
            }
            ui.add_space(theme::SPACE_8);
            ui.horizontal(|ui| {
                ui.label("или путь:");
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut state.workspace_path_draft)
                        .desired_width(280.0)
                        .hint_text("/абсолютный/путь"),
                );
                if ui.button("Сохранить").clicked()
                    || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    let draft = state.workspace_path_draft.clone();
                    state.set_workspace_path(draft);
                }
            });
        },
    );
}

fn empty_no_tasks(ui: &mut egui::Ui, state: &mut AppState) {
    match state.task_filter {
        TaskFilter::Archived => {
            empty_card(
                ui,
                "нет архивных",
                "В архиве нет задач. Создавать из этого фильтра не нужно.",
                |_ui| {},
            );
        }
        TaskFilter::Open => {
            empty_card(
                ui,
                "Нет задач",
                "Workspace есть, открытых задач нет.",
                |ui| {
                    ui.add_enabled_ui(state.can_create_task(), |ui| {
                        if ui
                            .add_sized(
                                [theme::CTA_WIDTH, theme::CTA_HEIGHT],
                                theme::primary_button("Новая задача"),
                            )
                            .clicked()
                        {
                            state.show_new_task_dialog = true;
                            state.new_task_title.clear();
                        }
                    });
                },
            );
        }
    }
}

fn show_task_list(ui: &mut egui::Ui, state: &mut AppState) {
    let rows: Vec<(String, String, TaskStatus, String)> = state
        .filtered_tasks()
        .into_iter()
        .map(|t| {
            (
                t.id.clone(),
                t.title.clone(),
                t.status,
                t.updated_at.clone(),
            )
        })
        .collect();

    let mut select_id: Option<String> = None;
    let mut rename: Option<(String, String)> = None;
    let mut archive_id: Option<String> = None;

    theme::content_frame().show(ui, |ui| {
        egui::Grid::new("task_header")
            .num_columns(5)
            .spacing([theme::SPACE_12, theme::SPACE_8])
            .min_col_width(80.0)
            .show(ui, |ui| {
                ui.weak("Название");
                ui.weak("Статус");
                ui.weak("updatedAt");
                ui.weak("");
                ui.weak("");
                ui.end_row();
            });
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            for (id, title, status, updated) in &rows {
                let selected = state.selected_task_id.as_deref() == Some(id.as_str());
                ui.horizontal(|ui| {
                    if ui.selectable_label(selected, title).clicked() {
                        select_id = Some(id.clone());
                    }
                    ui.add_space(theme::SPACE_16);
                    ui.label(status.label_ru());
                    ui.add_space(theme::SPACE_16);
                    ui.weak(updated);
                    ui.add_space(theme::SPACE_12);
                    if ui.small_button("Переименовать").clicked() {
                        rename = Some((id.clone(), title.clone()));
                    }
                    if *status == TaskStatus::Open && ui.small_button("В архив").clicked() {
                        archive_id = Some(id.clone());
                    }
                });
                ui.separator();
            }
        });
    });

    if let Some(id) = select_id {
        state.open_task(id);
    }
    if let Some((id, title)) = rename {
        state.begin_rename(id, title);
    }
    if let Some(id) = archive_id {
        state.archive_task(&id);
    }
}

pub fn show_new_task_dialog(ctx: &egui::Context, state: &mut AppState) {
    if !state.show_new_task_dialog {
        return;
    }
    let mut open = true;
    egui::Window::new("Новая задача")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label("Название");
            let _resp = ui.add(
                egui::TextEdit::singleline(&mut state.new_task_title)
                    .desired_width(320.0)
                    .hint_text("Например, починить логин"),
            );
            ui.add_space(theme::SPACE_8);
            ui.label(PRESET_LABEL);
            let host_ok = state.workspace_host_ok();
            ui.add_enabled_ui(host_ok, |ui| {
                let mut preset = state.new_task_preset.clone();
                let selected = preset
                    .as_deref()
                    .and_then(|id| {
                        state
                            .presets
                            .iter()
                            .find(|item| item.id == id)
                            .map(preset_combo_label)
                    })
                    .unwrap_or_else(|| PRESET_NONE.to_string());
                egui::ComboBox::from_id_salt("task_preset")
                    .selected_text(selected)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut preset, None, PRESET_NONE);
                        for item in &state.presets {
                            let label = preset_combo_label(item);
                            ui.selectable_value(&mut preset, Some(item.id.clone()), label);
                        }
                    });
                if preset != state.new_task_preset {
                    state.set_new_task_preset(preset);
                }
            });
            if state.can_rpc() && !host_ok {
                ui.weak(WORKSPACE_UNAVAILABLE);
            }
            ui.add_space(theme::SPACE_8);
            show_user_preset_editor(ui, state);
            ui.add_space(theme::SPACE_8);
            ui.horizontal(|ui| {
                let can = state.can_create_task() && !state.new_task_title.trim().is_empty();
                ui.add_enabled_ui(can, |ui| {
                    if ui.button("Создать").clicked() {
                        state.create_task();
                    }
                });
                if ui.button("Отмена").clicked() {
                    state.show_new_task_dialog = false;
                }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                state.create_task();
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                state.show_new_task_dialog = false;
            }
        });
    if !open {
        state.show_new_task_dialog = false;
    }
}

pub fn show_rename_dialog(ctx: &egui::Context, state: &mut AppState) {
    if !state.show_rename_dialog {
        return;
    }
    let mut open = true;
    egui::Window::new("Переименовать")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label("Название");
            let _resp = ui
                .add(egui::TextEdit::singleline(&mut state.rename_task_title).desired_width(320.0));
            ui.add_space(theme::SPACE_8);
            ui.horizontal(|ui| {
                let can = state.can_rpc() && !state.rename_task_title.trim().is_empty();
                ui.add_enabled_ui(can, |ui| {
                    if ui.button("Сохранить").clicked() {
                        state.commit_rename();
                    }
                });
                if ui.button("Отмена").clicked() {
                    state.show_rename_dialog = false;
                }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                state.commit_rename();
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                state.show_rename_dialog = false;
            }
        });
    if !open {
        state.show_rename_dialog = false;
    }
}

fn show_user_preset_editor(ui: &mut egui::Ui, state: &mut AppState) {
    ui.label(PRESET_NAME_LABEL);
    ui.add(
        egui::TextEdit::singleline(&mut state.preset_name_draft)
            .desired_width(320.0)
            .hint_text(PRESET_NAME_HINT),
    );
    ui.add_space(theme::SPACE_4);
    ui.label(ROLE_LABEL);
    let mut role = state.preset_role_draft.clone();
    let role_text = role_label_ru(&role).to_string();
    egui::ComboBox::from_id_salt("user_preset_role")
        .selected_text(role_text)
        .show_ui(ui, |ui| {
            for choice in workspace_ux::ROLE_CHOICES {
                ui.selectable_value(&mut role, (*choice).to_string(), role_label_ru(choice));
            }
        });
    if role != state.preset_role_draft {
        state.preset_role_draft = role;
    }
    ui.add_space(theme::SPACE_4);
    ui.label(PRESET_TITLE_HINT_LABEL);
    ui.add(
        egui::TextEdit::singleline(&mut state.preset_title_hint_draft)
            .desired_width(320.0)
            .hint_text(PRESET_TITLE_HINT_LABEL),
    );
    ui.add_space(theme::SPACE_4);
    ui.label(PRESET_PROMPT_LABEL);
    ui.add(
        egui::TextEdit::multiline(&mut state.preset_prompt_draft)
            .desired_width(320.0)
            .desired_rows(3)
            .hint_text(PRESET_PROMPT_LABEL),
    );
    ui.add_space(theme::SPACE_8);
    ui.horizontal(|ui| {
        if ui.button(PRESET_CREATE).clicked() {
            state.create_user_preset();
        }
        let user_selected = state.selected_user_preset_id().is_some();
        ui.add_enabled_ui(user_selected, |ui| {
            if ui.button(PRESET_SAVE).clicked() {
                state.save_user_preset();
            }
            if ui.button(PRESET_DELETE).clicked() {
                state.request_delete_user_preset();
            }
        });
    });
    if state.can_rpc() && !state.preset_crud_host_ok() {
        ui.weak(PRESETS_UNAVAILABLE);
    }
}

pub fn show_preset_delete_confirm(ctx: &egui::Context, state: &mut AppState) {
    if !state.show_preset_delete_confirm {
        return;
    }
    let mut open = true;
    egui::Window::new(PRESET_DELETE_TITLE)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(PRESET_DELETE_BODY);
            ui.add_space(theme::SPACE_8);
            ui.horizontal(|ui| {
                if ui.button(PRESET_DELETE_OK).clicked() {
                    state.confirm_delete_user_preset();
                }
                if ui.button("Отмена").clicked() {
                    state.cancel_delete_user_preset();
                }
            });
        });
    if !open {
        state.cancel_delete_user_preset();
    }
}
