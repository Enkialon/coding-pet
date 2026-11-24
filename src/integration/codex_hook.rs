//! Shared Codex hook handling for the local CLI adapter and the LAN forward API.
//! Decision logic (including auto-approve) always runs on the CodingPet host.

use super::event::AgentEvent;
use crate::config::Config;

/// Result of processing one Codex hook payload on the CodingPet host.
#[derive(Debug, Default)]
pub struct CodexHookOutcome {
    /// Bytes that a Codex hook must print to stdout (decision JSON, or empty).
    pub stdout: Vec<u8>,
    /// Optional animation event to dispatch into the pet UI.
    pub event: Option<AgentEvent>,
}

/// Process a Codex hook JSON payload using the host's local config.
///
/// Remote thin scripts must only forward the payload here and echo `stdout`.
/// Question / input prompts are never auto-answered — only `PermissionRequest`
/// can return an allow decision when `auto_approve_permissions` is enabled.
pub fn process_codex_hook(
    payload: &serde_json::Value,
    configured_hook_name: Option<&str>,
) -> CodexHookOutcome {
    let hook_name = payload
        .get("hook_event_name")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .or_else(|| configured_hook_name.map(str::to_owned));
    let Some(hook_name) = hook_name else {
        return CodexHookOutcome::default();
    };

    if hook_name == "PermissionRequest" && Config::load().auto_approve_permissions {
        return CodexHookOutcome {
            stdout: permission_request_allow_stdout().as_bytes().to_vec(),
            event: None,
        };
    }

    let Some(event_name) = codex_event_name(&hook_name) else {
        return CodexHookOutcome::default();
    };

    let mut event = AgentEvent::new("codex", event_name);
    event.id = string_field(payload, "turn_id");
    event.session_id = string_field(payload, "session_id");
    event.cwd = string_field(payload, "cwd");
    let mut data = serde_json::Map::new();
    data.insert("hook".into(), serde_json::Value::String(hook_name));
    for field in [
        "tool_name",
        "model",
        "source",
        "reason",
        "agent_id",
        "agent_type",
    ] {
        if let Some(value) = string_field(payload, field) {
            data.insert(field.into(), serde_json::Value::String(value));
        }
    }
    event.data = serde_json::Value::Object(data);

    CodexHookOutcome {
        stdout: Vec::new(),
        event: event.validate().ok().map(|_| event),
    }
}

pub fn permission_request_allow_stdout() -> &'static str {
    r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}"#
}

pub fn codex_event_name(hook_name: &str) -> Option<&'static str> {
    match hook_name {
        "SessionStart" => Some("agent.session.started"),
        "SessionEnd" => Some("agent.session.ended"),
        "UserPromptSubmit" => Some("agent.turn.started"),
        "PreToolUse" => Some("agent.tool.started"),
        "PostToolUse" => Some("agent.tool.completed"),
        "PermissionRequest" => Some("agent.permission.required"),
        "PreCompact" => Some("agent.review.started"),
        "PostCompact" => Some("agent.turn.started"),
        "SubagentStart" => Some("agent.task.started"),
        "SubagentStop" => Some("agent.task.completed"),
        "Stop" => Some("agent.turn.completed"),
        _ => None,
    }
}

fn string_field(payload: &serde_json::Value, field: &str) -> Option<String> {
    payload
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_all_supported_codex_hooks() {
        let cases = [
            ("SessionStart", "agent.session.started"),
            ("SessionEnd", "agent.session.ended"),
            ("UserPromptSubmit", "agent.turn.started"),
            ("PreToolUse", "agent.tool.started"),
            ("PostToolUse", "agent.tool.completed"),
            ("PermissionRequest", "agent.permission.required"),
            ("PreCompact", "agent.review.started"),
            ("PostCompact", "agent.turn.started"),
            ("SubagentStart", "agent.task.started"),
            ("SubagentStop", "agent.task.completed"),
            ("Stop", "agent.turn.completed"),
        ];
        for (hook, expected) in cases {
            assert_eq!(codex_event_name(hook), Some(expected));
        }
    }

    #[test]
    fn permission_request_allow_output_is_valid_json() {
        let value: serde_json::Value =
            serde_json::from_str(permission_request_allow_stdout()).unwrap();
        assert_eq!(
            value["hookSpecificOutput"]["hookEventName"],
            "PermissionRequest"
        );
        assert_eq!(value["hookSpecificOutput"]["decision"]["behavior"], "allow");
    }

    #[test]
    fn unknown_hook_is_a_no_op() {
        let payload = serde_json::json!({"hook_event_name": "NotARealHook"});
        let outcome = process_codex_hook(&payload, None);
        assert!(outcome.stdout.is_empty());
        assert!(outcome.event.is_none());
    }
}
