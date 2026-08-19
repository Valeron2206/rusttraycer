//! E6 A2A GUI: child conversations, inbox, bounded loops. No host spawn.

use crate::state::AgentStub;

pub const A2A_UNAVAILABLE: &str = "a2a недоступен: host без 1.5";
pub const NEW_CONVERSATION: &str = "Новый разговор";
pub const INBOX_PANE: &str = "Входящие";
pub const INBOX_LIVE: &str = "inbox";
pub const INBOX_OFF: &str = "inbox выкл";
pub const DELIVER_BUTTON: &str = "Отправить @";
pub const DELIVER_HINT: &str = "сообщение агенту";
pub const LOOP_PANE: &str = "Цикл";
pub const LOOP_START: &str = "Старт";
pub const LOOP_STOP: &str = "Стоп";
pub const LOOP_MAX_LABEL: &str = "макс. итераций";
pub const LOOP_RUNNING: &str = "цикл идёт";
pub const LOOP_PROMPT_HINT: &str = "промпт цикла";
pub const DELETE_AGENT: &str = "Удалить";
pub const A2A_PREFIX: &str = "a2a:";

pub const MIN_ITERATIONS: u32 = 1;
pub const MAX_ITERATIONS: u32 = 32;
pub const DEFAULT_ITERATIONS: u32 = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InboxItem {
    pub from_agent_id: String,
    pub to_agent_id: String,
    pub message_id: String,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoopView {
    pub id: String,
    pub iteration: u32,
    pub max_iterations: u32,
    pub turns: u32,
    pub status: String,
    pub reason: Option<String>,
}

impl LoopView {
    pub fn counter_label(&self) -> String {
        format!("{} / {}", self.iteration, self.max_iterations)
    }

    pub fn is_running(&self) -> bool {
        self.status == "running"
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentTreeNode {
    pub id: String,
    pub children: Vec<AgentTreeNode>,
}

/// Nest by `parentId`. Nodes whose parent is missing become roots (C46).
/// Deleting a parent must not hide surviving children.
pub fn build_agent_tree(agents: &[AgentStub]) -> Vec<AgentTreeNode> {
    let ids: std::collections::HashSet<&str> = agents.iter().map(|a| a.id.as_str()).collect();
    let mut children: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut roots = Vec::new();
    for agent in agents {
        match agent.parent_id.as_deref() {
            Some(parent) if ids.contains(parent) => {
                children
                    .entry(parent.to_string())
                    .or_default()
                    .push(agent.id.clone());
            }
            _ => roots.push(agent.id.clone()),
        }
    }
    fn walk(
        id: String,
        children: &std::collections::HashMap<String, Vec<String>>,
    ) -> AgentTreeNode {
        let kids = children
            .get(&id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|cid| walk(cid, children))
            .collect();
        AgentTreeNode { id, children: kids }
    }
    roots.into_iter().map(|id| walk(id, &children)).collect()
}

pub fn clamp_max_iterations(raw: i64) -> u32 {
    raw.clamp(i64::from(MIN_ITERATIONS), i64::from(MAX_ITERATIONS)) as u32
}

pub fn parse_max_iterations(draft: &str) -> u32 {
    let n = draft
        .trim()
        .parse::<i64>()
        .unwrap_or(i64::from(DEFAULT_ITERATIONS));
    clamp_max_iterations(n)
}

/// GUI never offers infinite loops. Always a visible 1..=32 number.
pub fn allows_infinite_loop() -> bool {
    false
}

pub fn is_a2a_system_message(role: &str, content: &str) -> bool {
    role == "system" && content.starts_with(A2A_PREFIX)
}

/// Parse host `a2a:<fromAgentId>\n…` system line. Not an artifact.
pub fn parse_a2a_prefix(content: &str) -> Option<(String, String)> {
    let rest = content.strip_prefix(A2A_PREFIX)?;
    let (from, body) = rest.split_once('\n').unwrap_or((rest, ""));
    if from.is_empty() {
        return None;
    }
    Some((from.to_string(), body.to_string()))
}

pub fn inbox_item_from_message(
    to_agent_id: &str,
    message_id: &str,
    role: &str,
    content: &str,
) -> Option<InboxItem> {
    if !is_a2a_system_message(role, content) {
        return None;
    }
    let (from, body) = parse_a2a_prefix(content)?;
    Some(InboxItem {
        from_agent_id: from,
        to_agent_id: to_agent_id.to_string(),
        message_id: message_id.to_string(),
        content: body,
    })
}

pub fn merge_inbox_item(items: &mut Vec<InboxItem>, item: InboxItem) {
    if items
        .iter()
        .any(|e| e.message_id == item.message_id && !item.message_id.is_empty())
    {
        return;
    }
    items.push(item);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AgentStatus;

    fn stub(id: &str, parent: Option<&str>) -> AgentStub {
        AgentStub {
            id: id.into(),
            task_id: "task-1".into(),
            parent_id: parent.map(str::to_string),
            provider: "cli.claude".into(),
            status: AgentStatus::Idle,
            interface: "chat".into(),
        }
    }

    #[test]
    fn ui_copy_is_locked() {
        assert_eq!(A2A_UNAVAILABLE, "a2a недоступен: host без 1.5");
        assert_eq!(NEW_CONVERSATION, "Новый разговор");
        assert_eq!(INBOX_PANE, "Входящие");
        assert_eq!(INBOX_LIVE, "inbox");
        assert_eq!(INBOX_OFF, "inbox выкл");
        assert_eq!(DELIVER_BUTTON, "Отправить @");
        assert_eq!(LOOP_PANE, "Цикл");
        assert_eq!(LOOP_START, "Старт");
        assert_eq!(LOOP_STOP, "Стоп");
        assert_eq!(LOOP_MAX_LABEL, "макс. итераций");
        assert_eq!(LOOP_RUNNING, "цикл идёт");
        assert_eq!(DELETE_AGENT, "Удалить");
        assert_eq!(crate::rpc::METHOD_A2A_DELIVER, "a2a.deliver");
        assert_eq!(crate::rpc::METHOD_LOOP_START, "loop.start");
        assert_eq!(crate::rpc::METHOD_LOOP_STOP, "loop.stop");
        assert_eq!(crate::rpc::A2A_METHODS.len(), 5);
        assert!(!allows_infinite_loop());
        assert_eq!(MIN_ITERATIONS, 1);
        assert_eq!(MAX_ITERATIONS, 32);
    }

    #[test]
    fn tree_keeps_orphans_when_parent_missing() {
        let items = vec![
            stub("parent", None),
            stub("child", Some("parent")),
            stub("orphan", Some("gone")),
        ];
        let tree = build_agent_tree(&items);
        assert_eq!(tree.len(), 2);
        assert_eq!(tree[0].id, "parent");
        assert_eq!(tree[0].children.len(), 1);
        assert_eq!(tree[0].children[0].id, "child");
        assert_eq!(tree[1].id, "orphan");

        let after: Vec<AgentStub> = items.into_iter().filter(|a| a.id != "parent").collect();
        let tree = build_agent_tree(&after);
        assert_eq!(tree.len(), 2);
        assert!(tree.iter().any(|n| n.id == "child"));
        assert!(tree.iter().any(|n| n.id == "orphan"));
        assert!(!tree.iter().any(|n| n.id == "parent"));
    }

    #[test]
    fn max_iterations_never_infinite() {
        assert_eq!(parse_max_iterations(""), DEFAULT_ITERATIONS);
        assert_eq!(parse_max_iterations("0"), MIN_ITERATIONS);
        assert_eq!(parse_max_iterations("-3"), MIN_ITERATIONS);
        assert_eq!(parse_max_iterations("99"), MAX_ITERATIONS);
        assert_eq!(parse_max_iterations("2"), 2);
        assert!(!allows_infinite_loop());
    }

    #[test]
    fn a2a_system_line_is_inbox_not_artifact() {
        let item =
            inbox_item_from_message("child-1", "msg-9", "system", "a2a:parent-1\nreview this")
                .expect("inbox");
        assert_eq!(item.from_agent_id, "parent-1");
        assert_eq!(item.to_agent_id, "child-1");
        assert_eq!(item.content, "review this");
        assert!(inbox_item_from_message("c", "m", "user", "hello").is_none());
        assert!(inbox_item_from_message("c", "m", "system", "plain").is_none());
    }
}
