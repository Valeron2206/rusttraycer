//! E5 artifacts GUI: tree, viewer, comments, markdown/PDF export. No host spawn.

use std::collections::HashMap;

use crate::rpc::{ArtifactOk, CommentThreadOk};

pub const ARTIFACTS_UNAVAILABLE: &str = "артефакты недоступны: host без 1.4";
pub const ARTIFACTS_PANE: &str = "Артефакты";
pub const NEED_TASK: &str = "сначала выберите задачу";
pub const CREATE_BUTTON: &str = "Создать";
pub const CREATE_KIND_LABEL: &str = "Тип";
pub const CREATE_TITLE_HINT: &str = "заголовок артефакта";
pub const CREATE_AS_CHILD: &str = "как дочерний";
pub const FILTER_KIND: &str = "Тип";
pub const FILTER_STATUS: &str = "Статус";
pub const FILTER_ALL: &str = "все";
pub const SAVE_BODY: &str = "Сохранить";
pub const EXPORT_MARKDOWN: &str = "Экспорт Markdown";
pub const EXPORT_PDF: &str = "Экспорт PDF";
pub const DELETE_ARTIFACT: &str = "Удалить";
pub const EDIT_BODY: &str = "Редактировать";
pub const VIEW_BODY: &str = "Просмотр";
pub const COMMENTS_HEADING: &str = "Комментарии";
pub const COMMENT_ON_SELECTION: &str = "Комментировать выделение";
pub const COMMENT_HINT: &str = "текст комментария";
pub const REPLY_BUTTON: &str = "Ответить";
pub const RESOLVE_BUTTON: &str = "Решить";
pub const RESOLVED_LABEL: &str = "решено";
pub const NO_SELECTION: &str = "выделите текст в теле артефакта";
pub const CLEAR_TRANSCRIPT: &str = "Очистить транскрипт";
pub const CLEAR_CONFIRM_TITLE: &str = "Очистить транскрипт?";
pub const CLEAR_CONFIRM_BODY: &str = "Сообщения агента будут удалены. Артефакты останутся.";
pub const CLEAR_CONFIRM_OK: &str = "Очистить";
pub const EXPORT_SAVED: &str = "Markdown сохранён";
pub const EXPORT_PDF_SAVED: &str = "PDF сохранён";
pub const EXPORT_PDF_EMPTY: &str = "host вернул пустой PDF";
pub const EXPORT_PDF_BAD_BYTES: &str = "не удалось декодировать PDF";
pub const KIND_SPEC: &str = "spec";
pub const KIND_TICKET: &str = "ticket";
pub const KIND_STORY: &str = "story";
pub const KIND_REVIEW: &str = "review";

pub const CREATE_KINDS: [&str; 4] = [KIND_SPEC, KIND_TICKET, KIND_STORY, KIND_REVIEW];
pub const STATUS_VALUES: [&str; 3] = ["todo", "in_progress", "done"];

pub const EXPORT_FORMAT: &str = "md";
pub const EXPORT_FORMAT_PDF: &str = "pdf";

/// Decode a 1.9 PDF payload. Prefer `bytes` (raw `%PDF` or base64); else markdown as base64 or raw UTF-8.
pub fn decode_export_pdf(bytes: &str, markdown: &str) -> Result<Vec<u8>, String> {
    if bytes.starts_with("%PDF") {
        return Ok(bytes.as_bytes().to_vec());
    }
    let payload = bytes.trim();
    if payload.starts_with("%PDF") {
        return Ok(payload.as_bytes().to_vec());
    }
    if !payload.is_empty() {
        return crate::terminal::decode_b64(payload)
            .ok_or_else(|| EXPORT_PDF_BAD_BYTES.to_string());
    }
    let md = markdown.trim();
    if md.is_empty() {
        return Err(EXPORT_PDF_EMPTY.to_string());
    }
    if let Some(decoded) = crate::terminal::decode_b64(md) {
        return Ok(decoded);
    }
    Ok(markdown.as_bytes().to_vec())
}

pub fn export_suggested_filename(id: &str, host_filename: &str, ext: &str) -> String {
    if host_filename.trim().is_empty() {
        format!("{id}.{ext}")
    } else {
        host_filename.to_string()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtifactKind {
    Spec,
    Ticket,
    Story,
    Review,
}

impl ArtifactKind {
    pub fn as_wire(self) -> &'static str {
        match self {
            Self::Spec => KIND_SPEC,
            Self::Ticket => KIND_TICKET,
            Self::Story => KIND_STORY,
            Self::Review => KIND_REVIEW,
        }
    }

    pub fn from_wire(s: &str) -> Option<Self> {
        match s {
            KIND_SPEC => Some(Self::Spec),
            KIND_TICKET => Some(Self::Ticket),
            KIND_STORY => Some(Self::Story),
            KIND_REVIEW => Some(Self::Review),
            _ => None,
        }
    }

    pub fn label_ru(self) -> &'static str {
        match self {
            Self::Spec => "spec",
            Self::Ticket => "ticket",
            Self::Story => "story",
            Self::Review => "review",
        }
    }

    pub fn allows_status(self) -> bool {
        matches!(self, Self::Ticket | Self::Story)
    }

    pub const ALL: [Self; 4] = [Self::Spec, Self::Ticket, Self::Story, Self::Review];
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactStub {
    pub id: String,
    pub task_id: String,
    pub parent_id: Option<String>,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub status: Option<String>,
    pub assignee: Option<String>,
    pub source_message_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<ArtifactOk> for ArtifactStub {
    fn from(ok: ArtifactOk) -> Self {
        Self {
            id: ok.id,
            task_id: ok.task_id,
            parent_id: ok.parent_id,
            kind: ok.kind,
            title: ok.title,
            body: ok.body,
            status: ok.status,
            assignee: ok.assignee,
            source_message_id: ok.source_message_id,
            created_at: ok.created_at,
            updated_at: ok.updated_at,
        }
    }
}

impl ArtifactStub {
    pub fn kind_enum(&self) -> Option<ArtifactKind> {
        ArtifactKind::from_wire(&self.kind)
    }

    pub fn allows_status(&self) -> bool {
        self.kind_enum()
            .map(ArtifactKind::allows_status)
            .unwrap_or(false)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommentItem {
    pub id: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommentThread {
    pub id: String,
    pub artifact_id: String,
    pub anchor_start: i64,
    pub anchor_end: i64,
    pub resolved: bool,
    pub comments: Vec<CommentItem>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<CommentThreadOk> for CommentThread {
    fn from(ok: CommentThreadOk) -> Self {
        Self {
            id: ok.id,
            artifact_id: ok.artifact_id,
            anchor_start: ok.anchor_start,
            anchor_end: ok.anchor_end,
            resolved: ok.resolved,
            comments: ok
                .comments
                .into_iter()
                .map(|c| CommentItem {
                    id: c.id,
                    body: c.body,
                    created_at: c.created_at,
                })
                .collect(),
            created_at: ok.created_at,
            updated_at: ok.updated_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactTreeNode {
    pub id: String,
    pub children: Vec<ArtifactTreeNode>,
}

/// Filter by optional kind and status. Empty / "все" means no filter.
pub fn matches_filter(item: &ArtifactStub, kind: Option<&str>, status: Option<&str>) -> bool {
    if let Some(kind) = kind {
        if !kind.is_empty() && kind != FILTER_ALL && item.kind != kind {
            return false;
        }
    }
    if let Some(status) = status {
        if !status.is_empty() && status != FILTER_ALL {
            return item.status.as_deref() == Some(status);
        }
    }
    true
}

/// Nest by `parentId`. Nodes whose parent is missing or filtered-out become roots.
pub fn build_tree(
    items: &[ArtifactStub],
    kind: Option<&str>,
    status: Option<&str>,
) -> Vec<ArtifactTreeNode> {
    let visible: Vec<&ArtifactStub> = items
        .iter()
        .filter(|a| matches_filter(a, kind, status))
        .collect();
    let ids: std::collections::HashSet<&str> = visible.iter().map(|a| a.id.as_str()).collect();
    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    let mut roots = Vec::new();
    for item in &visible {
        match item.parent_id.as_deref() {
            Some(parent) if ids.contains(parent) => {
                children
                    .entry(parent.to_string())
                    .or_default()
                    .push(item.id.clone());
            }
            _ => roots.push(item.id.clone()),
        }
    }
    fn walk(id: String, children: &HashMap<String, Vec<String>>) -> ArtifactTreeNode {
        let kids = children
            .get(&id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|cid| walk(cid, children))
            .collect();
        ArtifactTreeNode { id, children: kids }
    }
    roots.into_iter().map(|id| walk(id, &children)).collect()
}

pub fn utf8_range(start: usize, end: usize) -> Option<(usize, usize)> {
    if end > start {
        Some((start, end))
    } else {
        None
    }
}

pub fn show_markdown(ui: &mut egui::Ui, src: &str) {
    let mut in_code = false;
    let mut code_buf = String::new();
    for line in src.lines() {
        if line.starts_with("```") {
            if in_code {
                ui.monospace(code_buf.trim_end_matches('\n'));
                code_buf.clear();
                in_code = false;
            } else {
                in_code = true;
            }
            continue;
        }
        if in_code {
            code_buf.push_str(line);
            code_buf.push('\n');
            continue;
        }
        if let Some(text) = line.strip_prefix("### ") {
            ui.strong(text);
        } else if let Some(text) = line.strip_prefix("## ") {
            ui.heading(text);
        } else if let Some(text) = line.strip_prefix("# ") {
            ui.heading(text);
        } else if let Some(text) = line.strip_prefix("- ") {
            ui.label(format!("• {text}"));
        } else if let Some(text) = line.strip_prefix("* ") {
            ui.label(format!("• {text}"));
        } else if line.trim().is_empty() {
            ui.add_space(6.0);
        } else {
            ui.label(line);
        }
    }
    if !code_buf.is_empty() {
        ui.monospace(code_buf.trim_end_matches('\n'));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stub(id: &str, parent: Option<&str>, kind: &str, status: Option<&str>) -> ArtifactStub {
        ArtifactStub {
            id: id.into(),
            task_id: "task-1".into(),
            parent_id: parent.map(str::to_string),
            kind: kind.into(),
            title: id.into(),
            body: String::new(),
            status: status.map(str::to_string),
            assignee: None,
            source_message_id: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn ui_copy_is_locked() {
        assert_eq!(ARTIFACTS_UNAVAILABLE, "артефакты недоступны: host без 1.4");
        assert_eq!(NEED_TASK, "сначала выберите задачу");
        assert_eq!(EXPORT_MARKDOWN, "Экспорт Markdown");
        assert_eq!(EXPORT_PDF, "Экспорт PDF");
        assert_eq!(CLEAR_CONFIRM_TITLE, "Очистить транскрипт?");
        assert_eq!(
            CLEAR_CONFIRM_BODY,
            "Сообщения агента будут удалены. Артефакты останутся."
        );
        assert_eq!(CLEAR_CONFIRM_OK, "Очистить");
        assert_eq!(CLEAR_TRANSCRIPT, "Очистить транскрипт");
        assert_eq!(CREATE_KINDS, ["spec", "ticket", "story", "review"]);
        assert_eq!(EXPORT_FORMAT, "md");
        assert_eq!(EXPORT_FORMAT_PDF, "pdf");
        assert_eq!(EXPORT_PDF_SAVED, "PDF сохранён");
        assert_eq!(crate::rpc::METHOD_ARTIFACT_UPDATE, "artifact.update");
        assert_eq!(crate::rpc::METHOD_ARTIFACT_EXPORT, "artifact.export");
        assert_eq!(
            crate::rpc::METHOD_CLEAR_TRANSCRIPT,
            "agent.clear_transcript"
        );
        assert_eq!(crate::rpc::ARTIFACT_METHODS.len(), 10);
    }

    #[test]
    fn tree_nests_by_parent_id() {
        let items = vec![
            stub("root", None, "spec", None),
            stub("child", Some("root"), "ticket", Some("todo")),
            stub("leaf", Some("child"), "story", Some("done")),
            stub("orphan", Some("missing"), "review", None),
        ];
        let tree = build_tree(&items, None, None);
        assert_eq!(tree.len(), 2);
        assert_eq!(tree[0].id, "root");
        assert_eq!(tree[0].children.len(), 1);
        assert_eq!(tree[0].children[0].id, "child");
        assert_eq!(tree[0].children[0].children[0].id, "leaf");
        assert_eq!(tree[1].id, "orphan");
    }

    #[test]
    fn filter_kind_and_status_keeps_create_kinds() {
        let items = vec![
            stub("s", None, "spec", None),
            stub("t", None, "ticket", Some("todo")),
            stub("p", None, "ticket", Some("in_progress")),
            stub("d", None, "story", Some("done")),
        ];
        let only_ticket = build_tree(&items, Some("ticket"), None);
        assert_eq!(only_ticket.len(), 2);
        assert!(only_ticket.iter().all(|n| n.id == "t" || n.id == "p"));
        let only_done = build_tree(&items, None, Some("done"));
        assert_eq!(only_done.len(), 1);
        assert_eq!(only_done[0].id, "d");
        assert_eq!(CREATE_KINDS.len(), 4);
        assert!(CREATE_KINDS.contains(&"spec"));
        assert!(CREATE_KINDS.contains(&"review"));
    }

    #[test]
    fn utf8_range_requires_end_after_start() {
        assert_eq!(utf8_range(0, 12), Some((0, 12)));
        assert_eq!(utf8_range(4, 4), None);
        assert_eq!(utf8_range(9, 3), None);
    }

    #[test]
    fn decode_export_pdf_prefers_bytes_then_markdown() {
        let raw = b"%PDF-1.4 test";
        let b64 = crate::terminal::encode_b64(raw);
        assert_eq!(decode_export_pdf(&b64, "ignored").unwrap(), raw);
        assert_eq!(decode_export_pdf("", &b64).unwrap(), raw);
        assert_eq!(decode_export_pdf("", "# Auth").unwrap(), b"# Auth");
        assert_eq!(
            decode_export_pdf("   ", "  ").unwrap_err(),
            EXPORT_PDF_EMPTY
        );
        assert_eq!(
            decode_export_pdf("!!!!", "").unwrap_err(),
            EXPORT_PDF_BAD_BYTES
        );
        assert_eq!(export_suggested_filename("art-1", "", "pdf"), "art-1.pdf");
        assert_eq!(
            export_suggested_filename("art-1", "spec.pdf", "pdf"),
            "spec.pdf"
        );
    }

    #[test]
    fn decode_export_pdf_accepts_raw_percent_pdf_or_base64() {
        let raw = "%PDF-1.4\n1 0 obj\n<<>>\nendobj\n%%EOF\n";
        let got = decode_export_pdf(raw, "ignored").unwrap();
        assert!(got.starts_with(b"%PDF"), "raw payload must stay literal");
        assert_eq!(got, raw.as_bytes());

        let tiny = b"%PDF-1.4 tiny";
        let b64 = crate::terminal::encode_b64(tiny);
        assert_eq!(decode_export_pdf(&b64, "").unwrap(), tiny);

        assert_eq!(
            decode_export_pdf("not-valid-b64!!!", "").unwrap_err(),
            EXPORT_PDF_BAD_BYTES
        );
        assert_eq!(
            decode_export_pdf("!!!!", "").unwrap_err(),
            EXPORT_PDF_BAD_BYTES
        );
    }
}
