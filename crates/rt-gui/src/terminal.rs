//! E4 live PTY client. Host owns the PTY; GUI is a thin byte client.
//! Chat transcript (`messages`) is a separate buffer from PTY scrollback.

pub const TERMINAL_UNAVAILABLE: &str = "терминал недоступен: host без 1.3";
pub const NEW_TERMINAL: &str = "Новый терминал";
pub const CHAT_TAB: &str = "Чат";
pub const TERMINAL_TAB: &str = "Терминал";
pub const TERMINALS_PANE: &str = "Терминалы";
pub const CLOSE_TERMINAL: &str = "Закрыть";
pub const NO_LIVE_SHELL: &str = "нет живых shell";
pub const AGENT_IS_CHAT: &str = "этот агент — чат; live PTY нет";
pub const NEED_TASK: &str = "сначала выберите задачу";
pub const PTY_HINT: &str = "ввод идёт в host PTY · GUI процесс не спавнит";
pub const TERMINAL_DISABLED_CAPS: &str = "Терминал выключен: harness без pty";
pub const INTERFACE_LABEL: &str = "Интерфейс";
pub const SHELL_HINT: &str = "Shell · не агент";
pub const TERMINAL_AGENT_COMPOSER: &str = "терминальный агент: ввод в PTY";
pub const PTY_INPUT_HINT: &str = "ввод в PTY…";
pub const PTY_SUBMIT: &str = "Ввод";
pub const OPEN_PTY: &str = "Открыть PTY";

pub const DEFAULT_COLS: u16 = 80;
pub const DEFAULT_ROWS: u16 = 24;
pub const MAX_SCROLLBACK: usize = 256 * 1024;
pub const MAX_WRITE: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentInterface {
    Chat,
    Terminal,
}

impl AgentInterface {
    pub fn as_wire(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Terminal => "terminal",
        }
    }

    pub fn from_wire(s: &str) -> Self {
        if s == "terminal" {
            Self::Terminal
        } else {
            Self::Chat
        }
    }

    pub fn label_ru(self) -> &'static str {
        match self {
            Self::Chat => CHAT_TAB,
            Self::Terminal => TERMINAL_TAB,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentView {
    Chat,
    Terminal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellStub {
    pub id: String,
    pub pty_id: Option<String>,
    pub cwd: String,
}

pub fn append_scrollback(buf: &mut String, chunk: &str) {
    buf.push_str(chunk);
    if buf.len() > MAX_SCROLLBACK {
        let cut = buf.len() - MAX_SCROLLBACK;
        buf.drain(..cut);
    }
}

pub fn estimate_pty_size(width: f32, height: f32) -> (u16, u16) {
    let cols = (width / 8.0).floor() as i32;
    let rows = (height / 16.0).floor() as i32;
    let cols = cols.clamp(1, 500) as u16;
    let rows = rows.clamp(1, 500) as u16;
    (cols, rows)
}

pub fn encode_b64(input: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    let mut i = 0;
    while i < input.len() {
        let b0 = input[i];
        let b1 = if i + 1 < input.len() { input[i + 1] } else { 0 };
        let b2 = if i + 2 < input.len() { input[i + 2] } else { 0 };
        let n = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        if i + 1 < input.len() {
            out.push(T[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if i + 2 < input.len() {
            out.push(T[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
        i += 3;
    }
    out
}

pub fn decode_b64(s: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let cleaned: Vec<u8> = s.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if cleaned.len() % 4 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(cleaned.len() / 4 * 3);
    for chunk in cleaned.chunks(4) {
        let pad = chunk.iter().filter(|b| **b == b'=').count();
        if pad > 2 {
            return None;
        }
        let mut n = 0u32;
        for (i, b) in chunk.iter().enumerate() {
            if *b == b'=' {
                if i < 2 {
                    return None;
                }
                continue;
            }
            n |= u32::from(val(*b)?) << (18 - i * 6);
        }
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
    }
    Some(out)
}

pub fn decode_pty_data(b64: &str) -> String {
    match decode_b64(b64) {
        Some(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        None => String::new(),
    }
}

pub fn clamp_write(bytes: &[u8]) -> &[u8] {
    if bytes.len() > MAX_WRITE {
        &bytes[..MAX_WRITE]
    } else {
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ChatMessage;
    use crate::ws::{apply_event, parse_event, ApplyOutcome};

    #[test]
    fn ui_copy_is_locked() {
        assert_eq!(TERMINAL_UNAVAILABLE, "терминал недоступен: host без 1.3");
        assert_eq!(NEW_TERMINAL, "Новый терминал");
        assert_eq!(CHAT_TAB, "Чат");
        assert_eq!(TERMINAL_TAB, "Терминал");
        assert_eq!(TERMINALS_PANE, "Терминалы");
        assert_eq!(NEED_TASK, "сначала выберите задачу");
        assert_eq!(AGENT_IS_CHAT, "этот агент — чат; live PTY нет");
        assert_eq!(SHELL_HINT, "Shell · не агент");
    }

    #[test]
    fn b64_roundtrip() {
        let cases: &[&[u8]] = &[b"", b"a", b"ab", b"abc", b"ls\n", b"\x1b[31mhi"];
        for raw in cases {
            let enc = encode_b64(raw);
            let dec = decode_b64(&enc).expect("decode");
            assert_eq!(&dec, raw, "enc={enc}");
        }
        assert_eq!(encode_b64(b"ls\n"), "bHMK");
    }

    #[test]
    fn pty_output_does_not_enter_chat_messages() {
        let mut messages = vec![ChatMessage {
            id: "keep".into(),
            role: "user".into(),
            content: "hello".into(),
        }];
        let ev =
            parse_event(r#"{"type":"pty.data","ptyId":"pty-1","data":"bHMK"}"#).expect("parse");
        let outcome = apply_event(&mut messages, &ev, Some("task-1"), Some("ag-1"));
        assert_eq!(outcome, ApplyOutcome::PtyData);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "hello");
        assert!(!messages.iter().any(|m| m.content.contains("ls")));

        let exit = parse_event(r#"{"type":"pty.exit","ptyId":"pty-1","code":0}"#).expect("exit");
        assert_eq!(
            apply_event(&mut messages, &exit, None, None),
            ApplyOutcome::PtyExit
        );
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn scrollback_is_separate_from_messages() {
        let mut scrollback = String::new();
        let messages = Vec::<ChatMessage>::new();
        append_scrollback(&mut scrollback, &decode_pty_data("bHMK"));
        assert_eq!(scrollback, "ls\n");
        assert!(messages.is_empty());
    }

    #[test]
    fn gui_prod_does_not_spawn_local_pty_or_command() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        let mut stack = vec![root];
        while let Some(dir) = stack.pop() {
            let entries = std::fs::read_dir(&dir).expect("read src");
            for entry in entries {
                let entry = entry.expect("entry");
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                    files.push(path);
                }
            }
        }
        assert!(!files.is_empty());
        for path in files {
            let raw = std::fs::read_to_string(&path).expect("read");
            let prod = strip_cfg_test(&raw);
            let name = path.display().to_string();
            assert!(
                !prod.contains("portable-pty"),
                "{name} must not depend on portable-pty"
            );
            assert!(
                !prod.contains("std::process::Command"),
                "{name} prod must not spawn Command"
            );
            assert!(
                !prod.contains("Command::new"),
                "{name} prod must not spawn Command"
            );
            assert!(
                !prod.contains("PtyPair") && !prod.contains("native_pty_system"),
                "{name} must not open a local PTY"
            );
        }
    }

    fn strip_cfg_test(src: &str) -> String {
        let marker = "#[cfg(test)]";
        match src.find(marker) {
            Some(idx) => src[..idx].to_string(),
            None => src.to_string(),
        }
    }
}
