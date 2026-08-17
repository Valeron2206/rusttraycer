//! Coding-agent adapters. MVP: `cli.generic` only. Does not open a database.
//!
//! Capability field exists (`HarnessCaps` / `AgentBackend::caps`); Core ignores it in MVP.
//! `cli.generic` is the only backend.

mod cli_generic;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::pin::Pin;

use futures::Stream;
use serde::{Deserialize, Serialize};

pub use cli_generic::CliGeneric;

#[derive(Debug, Clone)]
pub struct Availability {
    pub available: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WireRole {
    User,
    Assistant,
    System,
    Tool,
}

impl WireRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::System => "system",
            Self::Tool => "tool",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireMessage {
    pub role: WireRole,
    pub content: String,
}

pub type RuntimeMessage = WireMessage;

#[derive(Debug, Clone)]
pub struct TurnRequest {
    pub agent_id: String,
    pub task_id: String,
    pub workspace_path: PathBuf,
    pub messages: Vec<WireMessage>,
    pub extra_env: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub enum TurnEvent {
    Token { text: String },
    Tool { name: String, payload: serde_json::Value },
    Finished { exit_code: i32 },
    Failed { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessCaps {
    pub one_shot: bool,
    pub long_lived: bool,
    pub stream_tokens: bool,
    pub tools: bool,
    pub session_resume: bool,
    pub a2a_inbox: bool,
    pub pty: bool,
    pub needs_api_key: bool,
    pub api_key_env: Option<&'static str>,
}

impl HarnessCaps {
    pub const CLI_GENERIC: Self = Self {
        one_shot: true,
        long_lived: false,
        stream_tokens: true,
        tools: false,
        session_resume: false,
        a2a_inbox: false,
        pty: false,
        needs_api_key: false,
        api_key_env: None,
    };
}

pub trait AgentBackend: Send + Sync {
    fn id(&self) -> &'static str;
    fn available(&self) -> Availability;
    fn caps(&self) -> HarnessCaps {
        HarnessCaps::CLI_GENERIC
    }
    fn start_turn(&self, req: TurnRequest) -> Pin<Box<dyn Stream<Item = TurnEvent> + Send>>;
}
