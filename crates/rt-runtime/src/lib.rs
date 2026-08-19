//! Coding-agent adapters. `cli.generic` + `cli.claude` + `cli.codex`. Does not open a database.

mod cli_claude;
mod cli_codex;
mod cli_generic;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::pin::Pin;

use futures::Stream;
use serde::{Deserialize, Serialize};

pub use cli_claude::CliClaude;
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
    Token {
        text: String,
    },
    Tool {
        name: String,
        payload: serde_json::Value,
    },
    Finished {
        exit_code: i32,
    },
    Failed {
        message: String,
    },
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

    pub const CLI_CLAUDE: Self = Self {
        one_shot: true,
        long_lived: false,
        stream_tokens: true,
        tools: false,
        session_resume: true,
        a2a_inbox: true,
        pty: true,
        needs_api_key: false,
        api_key_env: None,
    };

    pub const CLI_CODEX: Self = Self {
        one_shot: true,
        long_lived: false,
        stream_tokens: true,
        tools: false,
        session_resume: true,
        a2a_inbox: false,
        pty: true,
        needs_api_key: false,
        api_key_env: None,
    };
}

/// Host stores `providerSessionId` and puts it here on the next turn / pty.open.
/// Adapters map it to vendor argv (`claude --resume ID`, `codex exec resume ID`).
pub const PROVIDER_SESSION_ENV: &str = "RUSTTRAYCER_PROVIDER_SESSION_ID";

/// Non-empty trimmed vendor session id from `extra_env`, if host supplied one.
pub fn provider_session_id(extra_env: &BTreeMap<String, String>) -> Option<&str> {
    extra_env
        .get(PROVIDER_SESSION_ENV)
        .map(String::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
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
    /// Terminal A2A transcript: vendor session on this host, never PTY scrollback.
    /// Missing or silent provider session → Err, not an empty list.
    fn read_vendor_transcript(
        &self,
        _req: VendorTranscriptRequest,
    ) -> Result<Vec<WireMessage>, VendorTranscriptErr> {
        Err(VendorTranscriptErr {
            message: format!("{} has no vendor transcript", self.id()),
        })
    }
}

/// Host passes stored `providerSessionId` + workspace. Integration reads the vendor file.
#[derive(Debug, Clone)]
pub struct VendorTranscriptRequest {
    pub session_id: String,
    pub workspace_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VendorTranscriptErr {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelErr {
    pub message: String,
}

/// Local availability probe for doctor (no host / sqlite).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessProbe {
    pub id: String,
    pub available: bool,
    pub detail: String,
}

fn probe_one(backend: &impl AgentBackend) -> HarnessProbe {
    let avail = backend.available();
    HarnessProbe {
        id: backend.id().to_string(),
        available: avail.available,
        detail: avail.detail,
    }
}

/// Probe the three shipped adapters via `AgentBackend::available`.
/// Order: `cli.generic`, `cli.claude`, `cli.codex`.
pub fn probe_harnesses() -> Vec<HarnessProbe> {
    vec![
        probe_one(&CliGeneric::from_env()),
        probe_one(&CliClaude::from_env()),
        probe_one(&CliCodex::from_env()),
    ]
}

#[cfg(test)]
pub(crate) static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
        fn start_turn(&self, _req: TurnRequest) -> Pin<Box<dyn Stream<Item = TurnEvent> + Send>> {
            Box::pin(futures::stream::empty())
        }
    }

    #[test]
    fn cancel_without_child_is_ok() {
        assert!(NoChild.cancel_turn("no-such-agent").is_ok());
    }

    #[test]
    fn default_caps_on_backend_without_override() {
        assert_eq!(NoChild.caps(), HarnessCaps::CLI_GENERIC);
        assert_eq!(NoChild.id(), "test.noop");
        assert!(!NoChild.available().available);
        assert_eq!(NoChild.available().detail, "none");
    }

    #[test]
    fn cancel_err_equality() {
        let a = CancelErr {
            message: "cancelled".into(),
        };
        let b = CancelErr {
            message: "cancelled".into(),
        };
        let c = CancelErr {
            message: "other".into(),
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn wire_role_as_str_all_variants() {
        assert_eq!(WireRole::User.as_str(), "user");
        assert_eq!(WireRole::Assistant.as_str(), "assistant");
        assert_eq!(WireRole::System.as_str(), "system");
        assert_eq!(WireRole::Tool.as_str(), "tool");
    }

    fn assert_one_shot_cli_caps(
        caps: HarnessCaps,
        pty: bool,
        session_resume: bool,
        a2a_inbox: bool,
    ) {
        assert!(caps.one_shot);
        assert!(!caps.long_lived);
        assert!(caps.stream_tokens);
        assert!(!caps.tools);
        assert_eq!(caps.session_resume, session_resume);
        assert_eq!(caps.a2a_inbox, a2a_inbox);
        assert_eq!(caps.pty, pty);
        assert!(!caps.needs_api_key);
        assert!(caps.api_key_env.is_none());
    }

    #[test]
    fn harness_caps_cli_generic_fields() {
        assert_one_shot_cli_caps(HarnessCaps::CLI_GENERIC, false, false, false);
    }

    #[test]
    fn harness_caps_cli_claude_fields() {
        assert_one_shot_cli_caps(HarnessCaps::CLI_CLAUDE, true, true, true);
        assert_ne!(HarnessCaps::CLI_CLAUDE, HarnessCaps::CLI_GENERIC);
    }

    #[test]
    fn harness_caps_cli_codex_fields() {
        assert_one_shot_cli_caps(HarnessCaps::CLI_CODEX, true, true, false);
        assert_ne!(HarnessCaps::CLI_CODEX, HarnessCaps::CLI_CLAUDE);
        assert_ne!(HarnessCaps::CLI_CODEX, HarnessCaps::CLI_GENERIC);
    }

    #[test]
    fn default_backend_has_no_vendor_transcript() {
        let err = NoChild
            .read_vendor_transcript(VendorTranscriptRequest {
                session_id: "sess".into(),
                workspace_path: PathBuf::from("/tmp"),
            })
            .expect_err("default is no vendor history");
        assert!(err.message.contains("test.noop"), "{}", err.message);
    }

    #[test]
    fn generic_has_no_inbox_or_vendor_transcript() {
        let backend = CliGeneric::new("/bin/true");
        assert!(!backend.caps().a2a_inbox);
        let err = backend
            .read_vendor_transcript(VendorTranscriptRequest {
                session_id: "sess".into(),
                workspace_path: PathBuf::from("/tmp"),
            })
            .expect_err("generic has no A2A");
        assert!(err.message.contains("cli.generic"), "{}", err.message);
    }

    #[test]
    fn provider_session_id_trims_and_skips_empty() {
        let mut env = BTreeMap::new();
        assert_eq!(provider_session_id(&env), None);
        env.insert(PROVIDER_SESSION_ENV.into(), "  ".into());
        assert_eq!(provider_session_id(&env), None);
        env.insert(PROVIDER_SESSION_ENV.into(), "  sess-abc  ".into());
        assert_eq!(provider_session_id(&env), Some("sess-abc"));
    }
}

#[cfg(test)]
mod probe_tests {
    use super::*;

    #[test]
    fn probe_harnesses_returns_three_ids_in_order() {
        let _g = crate::TEST_ENV_LOCK.lock().unwrap();
        let probes = probe_harnesses();
        let ids: Vec<&str> = probes.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["cli.generic", "cli.claude", "cli.codex"]);
    }

    #[test]
    fn probe_harnesses_generic_unavailable_when_env_unset() {
        let _g = crate::TEST_ENV_LOCK.lock().unwrap();
        let prev = std::env::var("RUSTTRAYCER_GENERIC_CMD").ok();
        std::env::remove_var("RUSTTRAYCER_GENERIC_CMD");
        let probes = probe_harnesses();
        match prev {
            Some(v) => std::env::set_var("RUSTTRAYCER_GENERIC_CMD", v),
            None => std::env::remove_var("RUSTTRAYCER_GENERIC_CMD"),
        }
        let generic = probes
            .iter()
            .find(|p| p.id == "cli.generic")
            .expect("generic probe");
        assert!(!generic.available);
        assert!(
            generic.detail.contains("unset") || generic.detail.contains("not found"),
            "detail={}",
            generic.detail
        );
    }
}
