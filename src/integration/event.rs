use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const EVENT_SCHEMA_VERSION: u16 = 1;
pub const MAX_SOURCE_LEN: usize = 64;
pub const MAX_EVENT_LEN: usize = 96;
pub const MAX_TEXT_LEN: usize = 4096;

/// Provider-neutral event envelope used by all coding-tool adapters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    #[serde(default = "schema_version")]
    pub version: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub source: String,
    pub event: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default = "empty_object")]
    pub data: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentEventKind {
    Idle,
    Working,
    Reviewing,
    Waiting,
    Succeeded,
    Failed,
    Unknown,
}

impl AgentEvent {
    pub fn new(source: impl Into<String>, event: impl Into<String>) -> Self {
        Self {
            version: EVENT_SCHEMA_VERSION,
            id: None,
            source: source.into(),
            event: event.into(),
            timestamp: None,
            session_id: None,
            cwd: None,
            message: None,
            data: empty_object(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != EVENT_SCHEMA_VERSION {
            return Err(format!(
                "unsupported event version {}; expected {}",
                self.version, EVENT_SCHEMA_VERSION
            ));
        }
        validate_name("source", &self.source, MAX_SOURCE_LEN)?;
        validate_name("event", &self.event, MAX_EVENT_LEN)?;
        validate_optional_text("id", self.id.as_deref())?;
        validate_optional_text("timestamp", self.timestamp.as_deref())?;
        validate_optional_text("session_id", self.session_id.as_deref())?;
        validate_optional_text("cwd", self.cwd.as_deref())?;
        validate_optional_text("message", self.message.as_deref())?;
        if !self.data.is_object() {
            return Err("data must be a JSON object".into());
        }
        Ok(())
    }

    /// Normalize event names from different adapters into pet activity states.
    pub fn kind(&self) -> AgentEventKind {
        match self.event.as_str() {
            "agent.session.ended" | "agent.idle" => AgentEventKind::Idle,
            "agent.session.started" | "agent.turn.started" | "agent.task.started" => {
                AgentEventKind::Working
            }
            "agent.tool.completed" => AgentEventKind::Working,
            "agent.tool.started" | "agent.review.started" => AgentEventKind::Reviewing,
            "agent.input.required" | "agent.permission.required" | "agent.waiting" => {
                AgentEventKind::Waiting
            }
            "agent.turn.completed" | "agent.task.completed" => AgentEventKind::Succeeded,
            "agent.turn.failed" | "agent.task.failed" | "agent.tool.failed" => {
                AgentEventKind::Failed
            }
            _ => AgentEventKind::Unknown,
        }
    }
}

fn schema_version() -> u16 {
    EVENT_SCHEMA_VERSION
}

fn empty_object() -> Value {
    Value::Object(Default::default())
}

fn validate_name(field: &str, value: &str, max_len: usize) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    if value.len() > max_len {
        return Err(format!("{field} exceeds {max_len} bytes"));
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err(format!(
            "{field} may contain only ASCII letters, numbers, '.', '-' and '_'"
        ));
    }
    Ok(())
}

fn validate_optional_text(field: &str, value: Option<&str>) -> Result<(), String> {
    if value.is_some_and(|text| text.len() > MAX_TEXT_LEN) {
        return Err(format!("{field} exceeds {MAX_TEXT_LEN} bytes"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_current_schema() {
        let event: AgentEvent =
            serde_json::from_str(r#"{"source":"codex","event":"agent.turn.started"}"#).unwrap();
        assert_eq!(event.version, EVENT_SCHEMA_VERSION);
        assert_eq!(event.kind(), AgentEventKind::Working);
        assert!(event.validate().is_ok());
    }

    #[test]
    fn preserves_provider_specific_data() {
        let event: AgentEvent = serde_json::from_str(
            r#"{"source":"cursor","event":"agent.tool.started","data":{"tool":"shell"}}"#,
        )
        .unwrap();
        assert_eq!(event.data["tool"], "shell");
        assert_eq!(event.kind(), AgentEventKind::Reviewing);
    }

    #[test]
    fn rejects_unbounded_or_invalid_names() {
        let event = AgentEvent::new("bad source", "agent.turn.started");
        assert!(event.validate().is_err());
    }
}
