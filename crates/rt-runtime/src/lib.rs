//! Coding-agent adapters. `cli.generic` + `cli.codex`. Does not open a database.

mod cli_generic;
mod cli_codex;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::pin::Pin;

use futures::Stream;
use serde::{Deserialize, Serialize};

pub use cli_codex::CliCodex;
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

    pub const CLI_CODEX: Self = Self {
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
    /// Abort this agent's inflight turn. No child → Ok. Default is a no-op
    /// so host test backends keep compiling.
    fn cancel_turn(&self, _agent_id: &str) -> Result<(), CancelErr> {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelErr {
    pub message: String,
}

#[cfg(test)]
mod cancel_tests {
    use super::*;
    use std::pin::Pin;

    struct NoChild;

    impl AgentBackend for NoChild {
        fn id(&self) -> &'static str {
            "test.noop"
        }
        fn available(&self) -> Availability {
            Availability {
                available: false,
                detail: "none".into(),
            }
        }
        fn start_turn(
            &self,
            _req: TurnRequest,
        ) -> Pin<Box<dyn Stream<Item = TurnEvent> + Send>> {
            Box::pin(futures::stream::empty())
        }
    }

    #[test]
    fn cancel_without_child_is_ok() {
        assert!(NoChild.cancel_turn("no-such-agent").is_ok());
    }
}
