use super::codex_hook::process_codex_hook;
use super::event::AgentEvent;
use super::server::{INTEGRATION_PORT, INTEGRATION_TOKEN};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
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

/// Local Codex adapter: same host-side logic as `POST /v1/codex-hook`.
/// Fail-open: transport errors never change the coding session outcome.
fn run_codex_hook(configured_hook_name: Option<String>) -> Result<(), String> {
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let payload: serde_json::Value = serde_json::from_str(&input).unwrap_or_default();
    let outcome = process_codex_hook(&payload, configured_hook_name.as_deref());
    if !outcome.stdout.is_empty() {
        let _ = std::io::stdout().write_all(&outcome.stdout);
        let _ = writeln!(std::io::stdout());
    }
    if let Some(event) = outcome.event {
        let _ = send_event(&event);
    }
    Ok(())
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
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, INTEGRATION_PORT));
    let mut stream = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT)
        .map_err(|error| format!("could not connect to CodingPet: {error}"))?;
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
    let body = serde_json::to_vec(event).map_err(|error| error.to_string())?;
    write!(
        stream,
        "POST /v1/events HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {INTEGRATION_TOKEN}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
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
}
