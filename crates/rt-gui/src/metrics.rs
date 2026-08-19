//! C63 metrics chip. `GET /metrics` only — not a handshake method.
//! Prometheus text or JSON. Failure is `—`, never a panic.

use serde_json::Value;

pub const METRICS_LABEL: &str = "метрики";
pub const METRICS_EMPTY: &str = "—";
pub const METRICS_PATH: &str = "/metrics";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MetricsChip {
    pub agents: Option<u64>,
    pub rss: Option<u64>,
    pub rpc: Option<u64>,
}

impl MetricsChip {
    pub fn is_empty(&self) -> bool {
        self.agents.is_none() && self.rss.is_none() && self.rpc.is_none()
    }

    pub fn short_value(&self) -> String {
        if self.is_empty() {
            return METRICS_EMPTY.to_string();
        }
        let parts = [
            match self.agents {
                Some(n) => n.to_string(),
                None => METRICS_EMPTY.to_string(),
            },
            match self.rss {
                Some(n) => format_rss(n),
                None => METRICS_EMPTY.to_string(),
            },
            match self.rpc {
                Some(n) => n.to_string(),
                None => METRICS_EMPTY.to_string(),
            },
        ];
        parts.join("/")
    }
}

fn format_rss(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{}M", bytes / (1024 * 1024))
    } else if bytes >= 1024 {
        format!("{}K", bytes / 1024)
    } else {
        bytes.to_string()
    }
}

/// Parse Prometheus text 0.0.4 or a JSON object. Garbage → empty chip.
pub fn parse_metrics(body: &str) -> MetricsChip {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return MetricsChip::default();
    }
    if trimmed.starts_with('{') {
        if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
            return parse_json(&value);
        }
    }
    parse_prometheus(trimmed)
}

fn parse_json(value: &Value) -> MetricsChip {
    let agents = first_u64(
        value,
        &["agents", "rusttraycer_agents", "agentCount", "agent_count"],
    )
    .or_else(|| {
        value
            .get("agents")
            .and_then(Value::as_object)
            .map(|obj| obj.values().filter_map(json_u64).sum())
    });
    MetricsChip {
        agents,
        rss: first_u64(
            value,
            &[
                "rss",
                "rssBytes",
                "rss_bytes",
                "process_resident_memory_bytes",
                "rusttraycer_process_resident_memory_bytes",
            ],
        ),
        rpc: first_u64(value, &["rpc", "rpcCount", "rpc_count", "rusttraycer_rpc"]),
    }
}

fn first_u64(value: &Value, keys: &[&str]) -> Option<u64> {
    for key in keys {
        if let Some(n) = value.get(*key).and_then(json_u64) {
            return Some(n);
        }
    }
    None
}

fn json_u64(value: &Value) -> Option<u64> {
    if let Some(n) = value.as_u64() {
        return Some(n);
    }
    if let Some(n) = value.as_i64() {
        return u64::try_from(n).ok();
    }
    if let Some(n) = value.as_f64() {
        if n.is_finite() && n >= 0.0 {
            return Some(n as u64);
        }
    }
    value.as_str().and_then(|s| s.trim().parse().ok())
}

fn parse_prometheus(body: &str) -> MetricsChip {
    let mut agents: Option<u64> = None;
    let mut rss: Option<u64> = None;
    let mut rpc: Option<u64> = None;
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, rest)) = split_metric_line(line) else {
            continue;
        };
        let Some(n) = parse_metric_number(rest) else {
            continue;
        };
        let base = metric_base(name);
        if base == "rusttraycer_agents" || base == "agents" {
            agents = Some(agents.unwrap_or(0).saturating_add(n));
        } else if base.ends_with("resident_memory_bytes")
            || base == "rss"
            || base == "rusttraycer_rss"
        {
            rss = Some(n);
        } else if base == "rusttraycer_rpc" || base == "rpc" || base.ends_with("_rpc_total") {
            rpc = Some(rpc.unwrap_or(0).saturating_add(n));
        }
    }
    MetricsChip { agents, rss, rpc }
}

fn split_metric_line(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    if let Some(end) = line.find('}') {
        let rest = line.get(end + 1..)?.trim();
        return Some((line, rest));
    }
    let mut parts = line.splitn(2, char::is_whitespace);
    let name = parts.next()?;
    let rest = parts.next().unwrap_or("").trim();
    Some((name, rest))
}

fn metric_base(name: &str) -> &str {
    match name.find('{') {
        Some(i) => &name[..i],
        None => name.split_whitespace().next().unwrap_or(name),
    }
}

fn parse_metric_number(rest: &str) -> Option<u64> {
    let token = rest.split_whitespace().next()?.trim();
    if let Ok(n) = token.parse::<u64>() {
        return Some(n);
    }
    token.parse::<f64>().ok().and_then(|n| {
        if n.is_finite() && n >= 0.0 {
            Some(n as u64)
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_copy_is_locked() {
        assert_eq!(METRICS_LABEL, "метрики");
        assert_eq!(METRICS_EMPTY, "—");
        assert_eq!(METRICS_PATH, "/metrics");
    }

    #[test]
    fn parse_prometheus_sums_agents_and_keeps_rss_rpc() {
        let body = r#"
# TYPE rusttraycer_up gauge
rusttraycer_up 1
# TYPE rusttraycer_agents gauge
rusttraycer_agents{status="idle"} 2
rusttraycer_agents{status="running"} 1
rusttraycer_agents{status="error"} 0
process_resident_memory_bytes 2097152
rusttraycer_rpc_total 7
"#;
        let chip = parse_metrics(body);
        assert_eq!(chip.agents, Some(3));
        assert_eq!(chip.rss, Some(2097152));
        assert_eq!(chip.rpc, Some(7));
        assert_eq!(chip.short_value(), "3/2M/7");
    }

    #[test]
    fn parse_json_aliases() {
        let chip = parse_metrics(r#"{"agents": 4, "rss": 512, "rpc": 1}"#);
        assert_eq!(chip.agents, Some(4));
        assert_eq!(chip.rss, Some(512));
        assert_eq!(chip.rpc, Some(1));
        assert_eq!(chip.short_value(), "4/512/1");
    }

    #[test]
    fn garbage_or_empty_is_dash_not_panic() {
        assert!(parse_metrics("").is_empty());
        assert!(parse_metrics("not a metric").is_empty());
        assert!(parse_metrics("{").is_empty());
        assert_eq!(parse_metrics("").short_value(), METRICS_EMPTY);
    }
}
