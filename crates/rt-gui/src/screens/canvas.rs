use crate::a2a::{
    self, A2A_UNAVAILABLE, DELETE_AGENT, DELIVER_BUTTON, DELIVER_HINT, INBOX_LIVE, INBOX_OFF,
    INBOX_PANE, LOOP_MAX_LABEL, LOOP_PANE, LOOP_PROMPT_HINT, LOOP_RUNNING, LOOP_START, LOOP_STOP,
    NEW_CONVERSATION,
};
use crate::artifacts::{
    self, ArtifactKind, ARTIFACTS_PANE, ARTIFACTS_UNAVAILABLE, CLEAR_CONFIRM_BODY,
    CLEAR_CONFIRM_OK, CLEAR_CONFIRM_TITLE, CLEAR_TRANSCRIPT, COMMENTS_HEADING, COMMENT_HINT,
    COMMENT_ON_SELECTION, CREATE_AS_CHILD, CREATE_BUTTON, CREATE_KIND_LABEL, CREATE_TITLE_HINT,
    DELETE_ARTIFACT, EDIT_BODY, EXPORT_MARKDOWN, EXPORT_PDF, FILTER_ALL, FILTER_KIND,
    FILTER_STATUS, NEED_TASK, REPLY_BUTTON, RESOLVED_LABEL, RESOLVE_BUTTON, SAVE_BODY,
    STATUS_VALUES, VIEW_BODY,
};
use crate::ladder::{
    AgentPolicy, PaneKind, PolicyMode, APPROVAL_ALWAYS, APPROVAL_DENY, APPROVAL_ONCE,
    APPROVAL_TITLE, CAPS_LABEL, COMMIT_BUTTON, COMMIT_HINT, OPEN_IN_EDITOR, PICKER_EMPTY,
    PICKER_HINT, PICKER_LABEL, PICKER_UNAVAILABLE, POLICY_LABEL, PUSH_BUTTON, PUSH_CONFIRM_BODY,
    PUSH_CONFIRM_OK, PUSH_CONFIRM_TITLE, REVERT_BUTTON, STAGE_BUTTON, UNSTAGE_BUTTON,
    YOLO_CONFIRM_BODY, YOLO_CONFIRM_OK, YOLO_CONFIRM_TITLE, YOLO_OFF, YOLO_ON_BUTTON,
};
use crate::model_ux::{
    EFFORT_CHOICES, EFFORT_HINT, EFFORT_LABEL, FAST_LABEL, MODEL_HINT, MODEL_LABEL,
    MODEL_UNAVAILABLE, PROFILES_LABEL, PROFILE_APPLY, PROFILE_EMPTY, PROFILE_HINT,
    PROFILE_NAME_HINT, PROFILE_SAVE, SWITCH_BUTTON,
};
use crate::rpc::HarnessCapsView;
use crate::search_ux::{GC_BUTTON, GC_CONFIRM_BODY, GC_CONFIRM_OK, GC_CONFIRM_TITLE};
use crate::state::{AppState, FileKind, FilePreview};
use crate::terminal::{
    self, AgentInterface, AgentView, AGENT_IS_CHAT, CHAT_TAB, CLOSE_TERMINAL, INTERFACE_LABEL,
    NEW_TERMINAL, NO_LIVE_SHELL, OPEN_PTY, PTY_HINT, PTY_INPUT_HINT, PTY_SUBMIT, SHELL_HINT,
    TERMINALS_PANE, TERMINAL_DISABLED_CAPS, TERMINAL_TAB, TERMINAL_UNAVAILABLE,
};
use crate::workspace_ux::{
    self, agents_md_chip, guide_preview, role_label_ru, workspace_guide_chip, GUIDE_PANE,
    ROLE_CHOICES, ROLE_LABEL, WORKSPACE_UNAVAILABLE,
};

pub fn show(ctx: &egui::Context, state: &mut AppState) {
    if state.selected_task_id.is_none() && state.open_task_ids.is_empty() && !state.has_workspace()
    {
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
                let title = state
                    .selected_task_title()
                    .unwrap_or(terminal::TERMINALS_PANE)
                    .to_string();
                ui.strong(title);
                if let Some(preset) = state.selected_task_preset() {
                    ui.weak(workspace_ux::preset_label_ru(preset));
                }
                ui.separator();
                ui.weak(format!("host {}", state.host_id_prefix()));
                ui.separator();
                ui.weak(agents_md_chip(state.workspace_guides.as_ref()));
                ui.weak(workspace_guide_chip(state.workspace_guides.as_ref()));
                ui.separator();
                match state.selected_agent() {
                    Some(agent) => {
                        let role = workspace_ux::role_label_ru(state.selected_agent_role());
                        let model = state
                            .selected_agent_params()
                            .and_then(|p| p.model.clone())
                            .filter(|m| !m.is_empty());
                        if let Some(model) = model {
                            ui.label(format!(
                                "агент: {} · {} · {} · {}",
                                agent.status.label_ru(),
                                agent.provider,
                                role,
                                model
                            ));
                        } else {
                            ui.label(format!(
                                "агент: {} · {} · {}",
                                agent.status.label_ru(),
                                agent.provider,
                                role
                            ));
                        }
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
    show_worktree_gc_confirm(ctx, state);
}

pub fn show_artifact_dialogs(ctx: &egui::Context, state: &mut AppState) {
    show_clear_transcript_confirm(ctx, state);
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
        PaneKind::Artifacts => show_artifacts(ui, state),
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

fn show_clear_transcript_confirm(ctx: &egui::Context, state: &mut AppState) {
    if !state.show_clear_transcript_confirm {
        return;
    }
    let mut open = true;
    egui::Window::new(CLEAR_CONFIRM_TITLE)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(CLEAR_CONFIRM_BODY);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(CLEAR_CONFIRM_OK).clicked() {
                    state.confirm_clear_transcript();
                }
                if ui.button("Отмена").clicked() {
                    state.cancel_clear_transcript();
                }
            });
        });
    if !open {
        state.cancel_clear_transcript();
    }
}

fn show_worktree_gc_confirm(ctx: &egui::Context, state: &mut AppState) {
    if !state.show_worktree_gc_confirm {
        return;
    }
    let mut open = true;
    egui::Window::new(GC_CONFIRM_TITLE)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(GC_CONFIRM_BODY);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(GC_CONFIRM_OK).clicked() {
                    state.confirm_worktree_gc();
                }
                if ui.button("Отмена").clicked() {
                    state.cancel_worktree_gc();
                }
            });
        });
    if !open {
        state.cancel_worktree_gc();
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
    show_workspace_guides(ui, state);
    ui.add_space(6.0);
    show_provider_picker(ui, state);
    ui.add_space(6.0);
    show_role_picker(ui, state);
    ui.add_space(6.0);
    show_model_ux(ui, state);
    ui.add_space(6.0);
    show_interface_picker(ui, state);
    ui.add_space(8.0);
    show_policy_controls(ui, state);
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(6.0);

    let listed = state
        .agents_for_selected_task()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let tree = a2a::build_agent_tree(&listed);
    let selected = state.selected_agent().map(|a| a.id.clone());
    let mut pick = None;
    let mut delete = None;

    if listed.is_empty() {
        ui.label("Агента ещё нет.");
    }

    let live_ids: std::collections::HashSet<String> = listed
        .iter()
        .filter(|a| state.agent_has_inbox(a))
        .map(|a| a.id.clone())
        .collect();
    if !listed.is_empty() {
        fn paint(
            ui: &mut egui::Ui,
            agents: &[crate::state::AgentStub],
            node: &a2a::AgentTreeNode,
            selected: Option<&str>,
            live_ids: &std::collections::HashSet<String>,
            pick: &mut Option<String>,
            depth: usize,
        ) {
            let Some(agent) = agents.iter().find(|a| a.id == node.id) else {
                return;
            };
            let is_sel = selected == Some(agent.id.as_str());
            let inbox_live = live_ids.contains(&agent.id);
            ui.horizontal(|ui| {
                ui.add_space(depth as f32 * 12.0);
                let resp = egui::Frame::new()
                    .fill(if is_sel {
                        egui::Color32::from_rgb(40, 48, 64)
                    } else {
                        egui::Color32::from_rgb(32, 32, 38)
                    })
                    .inner_margin(egui::Margin::same(8))
                    .corner_radius(6.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.strong(&agent.provider);
                            let (dot, color, label) = if inbox_live {
                                ("●", egui::Color32::from_rgb(80, 200, 120), INBOX_LIVE)
                            } else {
                                ("○", egui::Color32::from_rgb(120, 120, 128), INBOX_OFF)
                            };
                            ui.colored_label(color, format!("{dot} {label}"));
                        });
                        ui.label(format!("статус: {}", agent.status.label_ru()));
                        ui.weak(AgentInterface::from_wire(&agent.interface).label_ru());
                        ui.weak(&agent.id);
                        ui.weak("нажмите, чтобы выбрать");
                    });
                if resp.response.interact(egui::Sense::click()).clicked() {
                    *pick = Some(agent.id.clone());
                }
            });
            for child in &node.children {
                paint(ui, agents, child, selected, live_ids, pick, depth + 1);
            }
        }
        for node in &tree {
            paint(
                ui,
                &listed,
                node,
                selected.as_deref(),
                &live_ids,
                &mut pick,
                0,
            );
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
    ui.add_enabled_ui(state.can_create_child(), |ui| {
        if ui
            .add_sized(
                [ui.available_width(), 28.0],
                egui::Button::new(NEW_CONVERSATION),
            )
            .clicked()
        {
            state.create_child_conversation();
        }
    });
    if state.selected_agent().is_some() && ui.small_button(DELETE_AGENT).clicked() {
        if let Some(id) = state.selected_agent().map(|a| a.id.clone()) {
            delete = Some(id);
        }
    }
    if !state.can_rpc() {
        ui.weak("недоступно: host offline");
    } else if state.providers.is_empty() {
        ui.weak(PICKER_EMPTY);
    } else if state.picker_provider.is_none() {
        ui.weak(PICKER_HINT);
    }
    if state.can_rpc() && !state.a2a_host_ok() {
        ui.weak(A2A_UNAVAILABLE);
    }
    if let Some(id) = pick {
        state.select_agent(id);
    }
    if let Some(id) = delete {
        state.remove_agent(&id);
    }
    ui.add_space(8.0);
    ui.separator();
    show_inbox(ui, state);
    ui.add_space(8.0);
    ui.separator();
    show_loop(ui, state);
}

fn show_inbox(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading(INBOX_PANE);
    let live = state.selected_inbox_live();
    let (dot, color, label) = if live {
        ("●", egui::Color32::from_rgb(80, 200, 120), INBOX_LIVE)
    } else {
        ("○", egui::Color32::from_rgb(120, 120, 128), INBOX_OFF)
    };
    ui.colored_label(color, format!("{dot} {label}"));
    let items: Vec<(String, String, String)> = state
        .inbox_for_selected()
        .into_iter()
        .map(|i| {
            (
                i.from_agent_id.clone(),
                i.message_id.clone(),
                i.content.clone(),
            )
        })
        .collect();
    if items.is_empty() {
        ui.weak("нет входящих");
    } else {
        for (from, mid, content) in items {
            ui.label(format!("от {from}"));
            if !content.is_empty() {
                ui.weak(content);
            } else {
                ui.weak(mid);
            }
        }
    }
}

fn show_loop(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading(LOOP_PANE);
    if let Some(status) = state.a2a_status.clone() {
        ui.weak(status);
    }
    let agents: Vec<(String, String)> = state
        .agents_for_selected_task()
        .into_iter()
        .map(|a| (a.id.clone(), a.provider.clone()))
        .collect();
    ui.horizontal(|ui| {
        ui.weak("A");
        let mut a = state.loop_agent_a.clone();
        egui::ComboBox::from_id_salt("loop_agent_a")
            .selected_text(a.clone().unwrap_or_else(|| "—".into()))
            .show_ui(ui, |ui| {
                for (id, provider) in &agents {
                    ui.selectable_value(&mut a, Some(id.clone()), format!("{provider} · {id}"));
                }
            });
        state.loop_agent_a = a;
    });
    ui.horizontal(|ui| {
        ui.weak("B");
        let mut b = state.loop_agent_b.clone();
        egui::ComboBox::from_id_salt("loop_agent_b")
            .selected_text(b.clone().unwrap_or_else(|| "—".into()))
            .show_ui(ui, |ui| {
                for (id, provider) in &agents {
                    ui.selectable_value(&mut b, Some(id.clone()), format!("{provider} · {id}"));
                }
            });
        state.loop_agent_b = b;
    });
    ui.horizontal(|ui| {
        ui.weak(LOOP_MAX_LABEL);
        ui.add(
            egui::TextEdit::singleline(&mut state.loop_max_draft)
                .desired_width(48.0)
                .hint_text("2"),
        );
        ui.weak(format!("1…{}", a2a::MAX_ITERATIONS));
    });
    ui.add(
        egui::TextEdit::singleline(&mut state.loop_prompt)
            .desired_width(ui.available_width())
            .hint_text(LOOP_PROMPT_HINT),
    );
    ui.horizontal(|ui| {
        ui.add_enabled_ui(state.can_start_loop(), |ui| {
            if ui.button(LOOP_START).clicked() {
                state.start_loop();
            }
        });
        let running = state.loop_state.as_ref().is_some_and(|l| l.is_running());
        ui.add_enabled_ui(running, |ui| {
            if ui.button(LOOP_STOP).clicked() {
                state.stop_loop();
            }
        });
    });
    if let Some(loop_state) = &state.loop_state {
        if loop_state.is_running() {
            ui.colored_label(egui::Color32::from_rgb(220, 180, 80), LOOP_RUNNING);
        }
        ui.label(loop_state.counter_label());
        if let Some(reason) = &loop_state.reason {
            ui.weak(reason);
        }
    }
}

fn show_workspace_guides(ui: &mut egui::Ui, state: &mut AppState) {
    ui.label(GUIDE_PANE);
    ui.horizontal(|ui| {
        ui.weak(agents_md_chip(state.workspace_guides.as_ref()));
        ui.weak(workspace_guide_chip(state.workspace_guides.as_ref()));
    });
    if state.can_rpc() && !state.workspace_host_ok() {
        ui.weak(WORKSPACE_UNAVAILABLE);
    }
    if let Some(status) = state.workspace_status.clone() {
        if status != WORKSPACE_UNAVAILABLE {
            ui.weak(status);
        }
    }
    if let Some(content) = guide_preview(
        state
            .workspace_guides
            .as_ref()
            .and_then(|g| g.agents_md.as_ref()),
    ) {
        if !content.is_empty() {
            ui.add(egui::Label::new(content).wrap());
        }
    }
}

fn show_role_picker(ui: &mut egui::Ui, state: &mut AppState) {
    let host_ok = state.workspace_host_ok();
    ui.add_enabled_ui(host_ok, |ui| {
        ui.label(ROLE_LABEL);
        let mut role = state.picker_role.clone();
        egui::ComboBox::from_id_salt("agent_role")
            .selected_text(role_label_ru(&role))
            .show_ui(ui, |ui| {
                for choice in ROLE_CHOICES {
                    ui.selectable_value(&mut role, (*choice).to_string(), role_label_ru(choice));
                }
            });
        if role != state.picker_role {
            state.set_picker_role(role);
        }
    });
    if state.can_rpc() && !host_ok {
        ui.weak(WORKSPACE_UNAVAILABLE);
    }
}

fn show_model_ux(ui: &mut egui::Ui, state: &mut AppState) {
    let host_ok = state.model_ux_host_ok();
    ui.add_enabled_ui(host_ok, |ui| {
        ui.label(MODEL_LABEL);
        ui.add(
            egui::TextEdit::singleline(&mut state.picker_model)
                .desired_width(ui.available_width())
                .hint_text(MODEL_HINT),
        );
        ui.horizontal(|ui| {
            ui.label(EFFORT_LABEL);
            let mut effort = state.picker_effort.clone();
            let effort_text = if effort.is_empty() {
                EFFORT_HINT.to_string()
            } else {
                effort.clone()
            };
            egui::ComboBox::from_id_salt("model_effort")
                .selected_text(effort_text)
                .show_ui(ui, |ui| {
                    for choice in EFFORT_CHOICES {
                        let label = if choice.is_empty() { "—" } else { *choice };
                        ui.selectable_value(&mut effort, (*choice).to_string(), label);
                    }
                });
            state.picker_effort = effort;
            ui.checkbox(&mut state.picker_fast, FAST_LABEL);
        });
        ui.add_enabled_ui(state.can_switch_agent(), |ui| {
            if ui
                .add_sized(
                    [ui.available_width(), 28.0],
                    egui::Button::new(SWITCH_BUTTON),
                )
                .clicked()
            {
                state.switch_selected_agent();
            }
        });
        ui.add_space(6.0);
        ui.label(PROFILES_LABEL);
        let profiles: Vec<(String, String)> = state
            .profiles
            .iter()
            .map(|p| (p.id.clone(), p.name.clone()))
            .collect();
        let current = state.selected_profile_id.clone();
        let selected_text = current
            .as_ref()
            .and_then(|id| {
                profiles
                    .iter()
                    .find(|(pid, _)| pid == id)
                    .map(|(_, n)| n.clone())
            })
            .unwrap_or_else(|| {
                if profiles.is_empty() {
                    PROFILE_EMPTY.into()
                } else {
                    PROFILE_HINT.into()
                }
            });
        let mut next = current.clone();
        egui::ComboBox::from_id_salt("model_profile")
            .selected_text(selected_text)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut next, None, PROFILE_HINT);
                for (id, name) in &profiles {
                    ui.selectable_value(&mut next, Some(id.clone()), name);
                }
            });
        if next != current {
            state.select_profile(next);
        }
        ui.add(
            egui::TextEdit::singleline(&mut state.profile_name_draft)
                .desired_width(ui.available_width())
                .hint_text(PROFILE_NAME_HINT),
        );
        ui.add_enabled_ui(state.can_create_profile(), |ui| {
            if ui
                .add_sized(
                    [ui.available_width(), 28.0],
                    egui::Button::new(PROFILE_SAVE),
                )
                .clicked()
            {
                state.create_profile_from_picker();
            }
        });
        ui.add_enabled_ui(state.can_apply_profile(), |ui| {
            if ui
                .add_sized(
                    [ui.available_width(), 28.0],
                    egui::Button::new(PROFILE_APPLY),
                )
                .clicked()
            {
                state.apply_selected_profile();
            }
        });
    });
    if state.can_rpc() && !host_ok {
        ui.weak(MODEL_UNAVAILABLE);
    }
    if let Some(status) = state.model_status.clone() {
        if status != MODEL_UNAVAILABLE {
            ui.weak(status);
        }
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
    ui.add_enabled_ui(state.can_rpc(), |ui| {
        if ui
            .add_sized([ui.available_width(), 28.0], egui::Button::new(GC_BUTTON))
            .clicked()
        {
            state.request_worktree_gc();
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
    if !state.has_workspace() {
        ui.weak(terminal::NEED_WORKSPACE);
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
    show_deliver(ui, state);
    ui.horizontal(|ui| {
        ui.add_enabled_ui(enabled, |ui| {
            if ui.button("Отправить").clicked() {
                state.send_composer();
            }
        });
        if show_stop && ui.button("Стоп").clicked() {
            state.cancel_running_agent();
        }
        ui.add_enabled_ui(state.can_clear_transcript(), |ui| {
            if ui.button(CLEAR_TRANSCRIPT).clicked() {
                state.request_clear_transcript();
            }
        });
        ui.weak("один активный turn · очередь не строится");
    });
    if let Some(reason) = reason {
        ui.weak(reason);
    }
}

fn show_deliver(ui: &mut egui::Ui, state: &mut AppState) {
    let targets: Vec<(String, String, bool)> = state
        .mention_targets()
        .into_iter()
        .map(|a| {
            (
                a.id.clone(),
                a.provider.clone(),
                state.can_deliver_to(&a.id),
            )
        })
        .collect();
    if targets.len() < 2 {
        return;
    }
    ui.horizontal(|ui| {
        ui.weak("@");
        let mut target = state.deliver_target.clone();
        egui::ComboBox::from_id_salt("deliver_target")
            .selected_text(target.clone().unwrap_or_else(|| "агент".into()))
            .show_ui(ui, |ui| {
                for (id, provider, inbox) in &targets {
                    let label = if *inbox {
                        format!("{provider} · {id}")
                    } else {
                        format!("{provider} · {id} ({INBOX_OFF})")
                    };
                    ui.selectable_value(&mut target, Some(id.clone()), label);
                }
            });
        state.deliver_target = target;
        ui.add(
            egui::TextEdit::singleline(&mut state.deliver_text)
                .desired_width(160.0)
                .hint_text(DELIVER_HINT),
        );
        let can = state
            .deliver_target
            .as_deref()
            .is_some_and(|id| state.can_deliver_to(id));
        ui.add_enabled_ui(can, |ui| {
            if ui.button(DELIVER_BUTTON).clicked() {
                state.deliver_to_selected_target();
            }
        });
    });
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

fn show_artifacts(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading(ARTIFACTS_PANE);
    if let Some(status) = state.artifacts_status.clone() {
        ui.weak(status);
    } else if state.can_rpc() && !state.artifacts_host_ok() {
        ui.weak(ARTIFACTS_UNAVAILABLE);
    }
    if state.selected_task_id.is_none() {
        ui.weak(NEED_TASK);
    }
    ui.add_space(6.0);
    show_artifact_filters(ui, state);
    ui.add_space(6.0);
    show_artifact_create(ui, state);
    ui.add_space(6.0);
    ui.separator();
    egui::ScrollArea::vertical()
        .id_salt("artifact_tree")
        .max_height(180.0)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            show_artifact_tree(ui, state);
        });
    ui.add_space(6.0);
    ui.separator();
    show_artifact_viewer(ui, state);
    if state.comments_visible() {
        ui.add_space(8.0);
        ui.separator();
        show_comments(ui, state);
    }
}

fn show_artifact_filters(ui: &mut egui::Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.weak(FILTER_KIND);
        let mut kind = state.artifact_kind_filter.clone();
        egui::ComboBox::from_id_salt("artifact_kind_filter")
            .selected_text(kind.clone())
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut kind, FILTER_ALL.into(), FILTER_ALL);
                for k in ArtifactKind::ALL {
                    ui.selectable_value(&mut kind, k.as_wire().to_string(), k.label_ru());
                }
            });
        state.artifact_kind_filter = kind;
        ui.weak(FILTER_STATUS);
        let mut status = state.artifact_status_filter.clone();
        egui::ComboBox::from_id_salt("artifact_status_filter")
            .selected_text(status.clone())
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut status, FILTER_ALL.into(), FILTER_ALL);
                for s in STATUS_VALUES {
                    ui.selectable_value(&mut status, (*s).to_string(), s);
                }
            });
        state.artifact_status_filter = status;
    });
}

fn show_artifact_create(ui: &mut egui::Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.weak(CREATE_KIND_LABEL);
        let mut kind = state.artifact_create_kind;
        egui::ComboBox::from_id_salt("artifact_create_kind")
            .selected_text(kind.label_ru())
            .show_ui(ui, |ui| {
                for k in ArtifactKind::ALL {
                    ui.selectable_value(&mut kind, k, k.label_ru());
                }
            });
        state.artifact_create_kind = kind;
        ui.add(
            egui::TextEdit::singleline(&mut state.artifact_create_title)
                .hint_text(CREATE_TITLE_HINT)
                .desired_width(160.0),
        );
        ui.checkbox(&mut state.artifact_create_as_child, CREATE_AS_CHILD);
        ui.add_enabled_ui(state.can_create_artifact(), |ui| {
            if ui.button(CREATE_BUTTON).clicked() {
                state.create_artifact();
            }
        });
    });
}

fn show_artifact_tree(ui: &mut egui::Ui, state: &mut AppState) {
    let kind = if state.artifact_kind_filter == FILTER_ALL {
        None
    } else {
        Some(state.artifact_kind_filter.as_str())
    };
    let status = if state.artifact_status_filter == FILTER_ALL {
        None
    } else {
        Some(state.artifact_status_filter.as_str())
    };
    let tree = artifacts::build_tree(&state.artifacts, kind, status);
    if tree.is_empty() {
        ui.weak("нет артефактов");
        return;
    }
    let selected = state.selected_artifact_id.clone();
    let mut pick = None;
    fn row(
        ui: &mut egui::Ui,
        state: &AppState,
        node: &artifacts::ArtifactTreeNode,
        selected: Option<&str>,
        pick: &mut Option<String>,
        depth: usize,
    ) {
        let title = state
            .artifacts
            .iter()
            .find(|a| a.id == node.id)
            .map(|a| format!("{} · {}", a.kind, a.title))
            .unwrap_or_else(|| node.id.clone());
        ui.horizontal(|ui| {
            ui.add_space(depth as f32 * 12.0);
            if ui
                .selectable_label(selected == Some(node.id.as_str()), title)
                .clicked()
            {
                *pick = Some(node.id.clone());
            }
        });
        for child in &node.children {
            row(ui, state, child, selected, pick, depth + 1);
        }
    }
    for node in &tree {
        row(ui, state, node, selected.as_deref(), &mut pick, 0);
    }
    if let Some(id) = pick {
        state.select_artifact(id);
    }
}

fn show_artifact_viewer(ui: &mut egui::Ui, state: &mut AppState) {
    let Some(id) = state.selected_artifact_id.clone() else {
        ui.weak("выберите артефакт");
        return;
    };
    let meta = state.artifacts.iter().find(|a| a.id == id).map(|a| {
        (
            a.kind.clone(),
            a.allows_status(),
            a.status.clone(),
            a.created_at.clone(),
            a.updated_at.clone(),
        )
    });
    let Some((kind, allows_status, status, created_at, updated_at)) = meta else {
        ui.weak("выберите артефакт");
        return;
    };
    ui.horizontal(|ui| {
        ui.strong(&state.artifact_title_draft);
        ui.weak(kind);
        if let Some(st) = status {
            ui.weak(st);
        }
        if !updated_at.is_empty() {
            ui.weak(updated_at);
        } else if !created_at.is_empty() {
            ui.weak(created_at);
        }
    });
    ui.add(
        egui::TextEdit::singleline(&mut state.artifact_title_draft).desired_width(f32::INFINITY),
    );
    if allows_status {
        ui.horizontal(|ui| {
            let mut next = state.artifact_status_draft.clone();
            egui::ComboBox::from_id_salt("artifact_status")
                .selected_text(if next.is_empty() {
                    "todo"
                } else {
                    next.as_str()
                })
                .show_ui(ui, |ui| {
                    for s in STATUS_VALUES {
                        ui.selectable_value(&mut next, (*s).to_string(), s);
                    }
                });
            if next != state.artifact_status_draft {
                state.set_artifact_status(next);
            }
            ui.add(
                egui::TextEdit::singleline(&mut state.artifact_assignee_draft)
                    .hint_text("assignee")
                    .desired_width(120.0),
            );
        });
    }
    ui.horizontal(|ui| {
        if ui
            .selectable_label(!state.artifact_editing, VIEW_BODY)
            .clicked()
        {
            state.artifact_editing = false;
        }
        if ui
            .selectable_label(state.artifact_editing, EDIT_BODY)
            .clicked()
        {
            state.artifact_editing = true;
        }
        if ui.button(SAVE_BODY).clicked() {
            state.save_artifact_body();
        }
        if ui.button(EXPORT_MARKDOWN).clicked() {
            if let Some((filename, markdown)) = state.export_selected_markdown() {
                state.save_exported_markdown(&filename, &markdown);
            }
        }
        if ui.button(EXPORT_PDF).clicked() {
            if let Some((filename, bytes)) = state.export_selected_pdf() {
                state.save_exported_pdf(&filename, &bytes);
            }
        }
        if ui.button(DELETE_ARTIFACT).clicked() {
            state.delete_selected_artifact();
        }
    });
    ui.add_space(4.0);
    if state.artifact_editing {
        let output = egui::TextEdit::multiline(&mut state.artifact_body_draft)
            .desired_width(f32::INFINITY)
            .desired_rows(12)
            .show(ui);
        if let Some(range) = output.cursor_range {
            let a = range.primary.ccursor.index;
            let b = range.secondary.ccursor.index;
            let (start, end) = if a <= b { (a, b) } else { (b, a) };
            state.artifact_selection = Some((start, end));
        }
    } else {
        let output = egui::TextEdit::multiline(&mut state.artifact_body_draft)
            .desired_width(f32::INFINITY)
            .desired_rows(4)
            .hint_text("выделите текст для комментария")
            .show(ui);
        if let Some(range) = output.cursor_range {
            let a = range.primary.ccursor.index;
            let b = range.secondary.ccursor.index;
            let (start, end) = if a <= b { (a, b) } else { (b, a) };
            state.artifact_selection = Some((start, end));
        }
        ui.add_space(6.0);
        artifacts::show_markdown(ui, &state.artifact_body_draft);
    }
}

fn show_comments(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading(COMMENTS_HEADING);
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut state.artifact_comment_draft)
                .hint_text(COMMENT_HINT)
                .desired_width(220.0),
        );
        if ui.button(COMMENT_ON_SELECTION).clicked() {
            state.open_comment_thread();
        }
    });
    let thread_ids: Vec<String> = state
        .artifact_threads
        .iter()
        .map(|t| t.id.clone())
        .collect();
    for id in thread_ids {
        let Some((resolved, start, end, created_at, updated_at, comments)) =
            state.artifact_threads.iter().find(|t| t.id == id).map(|t| {
                (
                    t.resolved,
                    t.anchor_start,
                    t.anchor_end,
                    t.created_at.clone(),
                    t.updated_at.clone(),
                    t.comments
                        .iter()
                        .map(|c| (c.body.clone(), c.created_at.clone()))
                        .collect::<Vec<_>>(),
                )
            })
        else {
            continue;
        };
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(30, 30, 36))
            .inner_margin(egui::Margin::same(8))
            .corner_radius(6.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.weak(format!("{start}–{end}"));
                    if !created_at.is_empty() {
                        ui.weak(&created_at);
                    } else if !updated_at.is_empty() {
                        ui.weak(&updated_at);
                    }
                    if resolved {
                        ui.weak(RESOLVED_LABEL);
                    } else if ui.small_button(RESOLVE_BUTTON).clicked() {
                        state.resolve_comment(id.clone());
                    }
                });
                for (body, at) in &comments {
                    ui.label(body);
                    if !at.is_empty() {
                        ui.weak(at);
                    }
                }
                let mut draft = state
                    .artifact_reply_drafts
                    .get(&id)
                    .cloned()
                    .unwrap_or_default();
                let mut send_reply = false;
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut draft)
                            .hint_text(COMMENT_HINT)
                            .desired_width(180.0),
                    );
                    if ui.small_button(REPLY_BUTTON).clicked() {
                        send_reply = true;
                    }
                });
                state.artifact_reply_drafts.insert(id.clone(), draft);
                if send_reply {
                    state.reply_comment(id.clone());
                }
            });
        ui.add_space(4.0);
    }
}
