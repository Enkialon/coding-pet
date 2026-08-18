use super::event::AgentEvent;
use super::runtime::{runtime_path, RuntimeDescriptor};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

const CONNECT_TIMEOUT: Duration = Duration::from_millis(300);
const IO_TIMEOUT: Duration = Duration::from_secs(1);

pub enum CliOutcome {
    NotCli,
    Handled(Result<(), String>),
}

pub fn try_run_cli() -> CliOutcome {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        return CliOutcome::NotCli;
    };

    let result = match command.as_str() {
        "emit" => build_event(args.collect()).and_then(|event| send_event(&event)),
        "emit-json" => read_event_from_stdin().and_then(|event| send_event(&event)),
        "codex-hook" => run_codex_hook(args.next()),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        _ => Err(format!(
            "unknown command '{command}'; use 'coding-pet help' for integration commands"
        )),
    };
    CliOutcome::Handled(result)
}

/// Translate a Codex hook payload into the provider-neutral event envelope.
/// Hook delivery is deliberately fail-open: telemetry must never block Codex.
fn run_codex_hook(configured_hook_name: Option<String>) -> Result<(), String> {
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let payload: serde_json::Value = serde_json::from_str(&input).unwrap_or_default();
    let hook_name = payload
        .get("hook_event_name")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .or(configured_hook_name);
    let Some(hook_name) = hook_name else {
        return Ok(());
    };
    let Some(event_name) = codex_event_name(&hook_name) else {
        return Ok(());
    };

    let mut event = AgentEvent::new("codex", event_name);
    event.id = string_field(&payload, "turn_id");
    event.session_id = string_field(&payload, "session_id");
    event.cwd = string_field(&payload, "cwd");
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
        if let Some(value) = string_field(&payload, field) {
            data.insert(field.into(), serde_json::Value::String(value));
        }
    }
    event.data = serde_json::Value::Object(data);

    // Ignore validation and transport failures so a visual accessory can
    // never change the outcome of a coding session.
    if event.validate().is_ok() {
        let _ = send_event(&event);
    }
    Ok(())
}

fn codex_event_name(hook_name: &str) -> Option<&'static str> {
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

fn build_event(args: Vec<String>) -> Result<AgentEvent, String> {
    let Some(event_name) = args.first() else {
        return Err("usage: coding-pet emit <event> [options]".into());
    };
    if event_name.starts_with('-') {
        return Err("the first emit argument must be an event name".into());
    }

    let mut event = AgentEvent::new("manual", event_name);
    event.cwd = std::env::current_dir()
        .ok()
        .map(|path| path.to_string_lossy().into_owned());

    let mut index = 1;
    while index < args.len() {
        let flag = &args[index];
        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--source" => event.source = value.clone(),
            "--id" => event.id = Some(value.clone()),
            "--session" => event.session_id = Some(value.clone()),
            "--cwd" => event.cwd = Some(value.clone()),
            "--message" => event.message = Some(value.clone()),
            "--timestamp" => event.timestamp = Some(value.clone()),
            "--data" => {
                event.data = serde_json::from_str(value)
                    .map_err(|error| format!("invalid --data JSON: {error}"))?;
            }
            _ => return Err(format!("unknown emit option: {flag}")),
        }
        index += 2;
    }
    event.validate()?;
    Ok(event)
}

fn read_event_from_stdin() -> Result<AgentEvent, String> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|error| format!("could not read stdin: {error}"))?;
    let event: AgentEvent =
        serde_json::from_str(&input).map_err(|error| format!("invalid event JSON: {error}"))?;
    event.validate()?;
    Ok(event)
}

fn send_event(event: &AgentEvent) -> Result<(), String> {
    let path = runtime_path().ok_or("could not determine CodingPet configuration directory")?;
    let descriptor: RuntimeDescriptor =
        serde_json::from_slice(&std::fs::read(&path).map_err(|_| {
            "CodingPet is not running (integration.json was not found)".to_string()
        })?)
        .map_err(|error| format!("invalid integration.json: {error}"))?;
    if descriptor.version != 1 {
        return Err(format!(
            "unsupported integration runtime version {}",
            descriptor.version
        ));
    }
    let address: SocketAddr = descriptor
        .address
        .parse()
        .map_err(|_| "integration.json contains an invalid address".to_string())?;
    if !address.ip().is_loopback() {
        return Err("refusing to send an integration event to a non-loopback address".into());
    }

    let mut stream = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT)
        .map_err(|error| format!("could not connect to CodingPet: {error}"))?;
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
    let body = serde_json::to_vec(event).map_err(|error| error.to_string())?;
    write!(
        stream,
        "POST /v1/events HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        descriptor.address,
        descriptor.token,
        body.len()
    )
    .and_then(|_| stream.write_all(&body))
    .map_err(|error| format!("could not send event: {error}"))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|error| format!("could not read CodingPet response: {error}"))?;
    let status = response.lines().next().unwrap_or_default();
    if !status.contains(" 202 ") {
        return Err(format!("CodingPet rejected the event: {status}"));
    }
    Ok(())
}

fn print_help() {
    eprintln!(
        "CodingPet integration commands:\n\
         coding-pet emit <event> [--source NAME] [--session ID] [--message TEXT]\n\
         coding-pet emit-json    # reads one canonical JSON event from stdin\n\
         coding-pet codex-hook <HookEventName>    # Codex lifecycle adapter"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_provider_neutral_emit_arguments() {
        let event = build_event(vec![
            "agent.turn.completed".into(),
            "--source".into(),
            "claude-code".into(),
            "--session".into(),
            "session-1".into(),
            "--data".into(),
            r#"{"duration_ms":42}"#.into(),
        ])
        .unwrap();
        assert_eq!(event.source, "claude-code");
        assert_eq!(event.session_id.as_deref(), Some("session-1"));
        assert_eq!(event.data["duration_ms"], 42);
    }

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
}
