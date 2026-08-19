use super::event::AgentEvent;
use super::runtime::{runtime_path, RuntimeDescriptor};
use super::AppUserEvent;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use winit::event_loop::EventLoopProxy;

const MAX_REQUEST_BYTES: usize = 64 * 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(2);

pub struct IntegrationServer {
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    runtime_path: PathBuf,
    address: SocketAddr,
}

impl IntegrationServer {
    pub fn start(proxy: EventLoopProxy<AppUserEvent>) -> Result<Self, std::io::Error> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let address = listener.local_addr()?;
        let token = generate_token()?;
        let runtime_path = runtime_path().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "could not determine CodingPet configuration directory",
            )
        })?;

        if let Some(parent) = runtime_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let descriptor = RuntimeDescriptor {
            version: 1,
            pid: std::process::id(),
            address: address.to_string(),
            token: token.clone(),
        };
        std::fs::write(&runtime_path, serde_json::to_vec_pretty(&descriptor)?)?;

        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_shutdown = Arc::clone(&shutdown);
        let thread = thread::Builder::new()
            .name("coding-pet-integration".into())
            .spawn(move || {
                run_server(
                    listener,
                    token,
                    move |event| proxy.send_event(AppUserEvent::Agent(event)).map_err(|_| ()),
                    thread_shutdown,
                )
            })?;

        tracing::info!("Integration API listening on http://{address}");
        Ok(Self {
            shutdown,
            thread: Some(thread),
            runtime_path,
            address,
        })
    }
}

impl Drop for IntegrationServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        // Wake the blocking accept call. The server checks shutdown before
        // reading this connection, so dropping never waits for a read timeout.
        let _ = TcpStream::connect_timeout(&self.address, Duration::from_millis(100));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        // Do not remove a descriptor replaced by a newer CodingPet process.
        let belongs_to_this_process = std::fs::read(&self.runtime_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<RuntimeDescriptor>(&bytes).ok())
            .is_some_and(|descriptor| descriptor.pid == std::process::id());
        if belongs_to_this_process {
            let _ = std::fs::remove_file(&self.runtime_path);
        }
    }
}

fn run_server<F>(listener: TcpListener, token: String, dispatch: F, shutdown: Arc<AtomicBool>)
where
    F: Fn(AgentEvent) -> Result<(), ()>,
{
    while !shutdown.load(Ordering::Acquire) {
        match listener.accept() {
            Ok(_) if shutdown.load(Ordering::Acquire) => break,
            Ok((stream, _)) => handle_connection(stream, &token, &dispatch),
            Err(error) => {
                tracing::warn!("Integration API accept failed: {error}");
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

fn handle_connection<F>(mut stream: TcpStream, token: &str, dispatch: &F)
where
    F: Fn(AgentEvent) -> Result<(), ()>,
{
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let _ = stream.set_write_timeout(Some(READ_TIMEOUT));

    let request = match read_request(&mut stream) {
        Ok(request) => request,
        Err((status, message)) => {
            let _ = write_json_response(&mut stream, status, &message);
            return;
        }
    };

    if request.method == "GET" && request.path == "/health" {
        let _ = write_json_response(&mut stream, 200, "ok");
        return;
    }
    if request.method != "POST" || request.path != "/v1/events" {
        let _ = write_json_response(&mut stream, 404, "not found");
        return;
    }
    if request.authorization.as_deref() != Some(token) {
        let _ = write_json_response(&mut stream, 401, "invalid bearer token");
        return;
    }
    if request.content_type.as_deref() != Some("application/json") {
        let _ = write_json_response(&mut stream, 415, "content-type must be application/json");
        return;
    }

    let event: AgentEvent = match serde_json::from_slice(&request.body) {
        Ok(event) => event,
        Err(error) => {
            let _ = write_json_response(&mut stream, 400, &format!("invalid JSON: {error}"));
            return;
        }
    };
    if let Err(message) = event.validate() {
        let _ = write_json_response(&mut stream, 422, &message);
        return;
    }

    if dispatch(event).is_err() {
        let _ = write_json_response(&mut stream, 503, "pet event loop is unavailable");
        return;
    }
    let _ = write_json_response(&mut stream, 202, "accepted");
}

struct HttpRequest {
    method: String,
    path: String,
    authorization: Option<String>,
    content_type: Option<String>,
    body: Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> Result<HttpRequest, (u16, String)> {
    let mut bytes = Vec::with_capacity(1024);
    let header_end = loop {
        if bytes.len() >= MAX_REQUEST_BYTES {
            return Err((413, "request is too large".into()));
        }
        let mut chunk = [0u8; 1024];
        let count = stream
            .read(&mut chunk)
            .map_err(|error| (400, format!("could not read request: {error}")))?;
        if count == 0 {
            return Err((400, "incomplete HTTP request".into()));
        }
        bytes.extend_from_slice(&chunk[..count]);
        if let Some(index) = find_header_end(&bytes) {
            break index;
        }
    };

    let headers = std::str::from_utf8(&bytes[..header_end])
        .map_err(|_| (400, "HTTP headers must be UTF-8".into()))?;
    let mut lines = headers.split("\r\n");
    let mut request_line = lines
        .next()
        .ok_or_else(|| (400, "missing request line".into()))?
        .split_whitespace();
    let method = request_line.next().unwrap_or_default().to_string();
    let path = request_line.next().unwrap_or_default().to_string();
    if request_line.next().is_none() {
        return Err((400, "invalid request line".into()));
    }

    let mut content_length = 0usize;
    let mut authorization = None;
    let mut content_type = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim();
        match name.as_str() {
            "content-length" => {
                content_length = value
                    .parse()
                    .map_err(|_| (400, "invalid content-length".into()))?;
            }
            "content-type" => {
                content_type = Some(
                    value
                        .split(';')
                        .next()
                        .unwrap_or_default()
                        .trim()
                        .to_ascii_lowercase(),
                );
            }
            "authorization" => {
                authorization = value.strip_prefix("Bearer ").map(str::to_owned);
            }
            _ => {}
        }
    }
    if header_end + 4 + content_length > MAX_REQUEST_BYTES {
        return Err((413, "request is too large".into()));
    }

    let body_start = header_end + 4;
    while bytes.len() < body_start + content_length {
        let mut chunk = [0u8; 4096];
        let count = stream
            .read(&mut chunk)
            .map_err(|error| (400, format!("could not read request body: {error}")))?;
        if count == 0 {
            return Err((400, "incomplete request body".into()));
        }
        bytes.extend_from_slice(&chunk[..count]);
    }

    Ok(HttpRequest {
        method,
        path,
        authorization,
        content_type,
        body: bytes[body_start..body_start + content_length].to_vec(),
    })
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn write_json_response(stream: &mut TcpStream, status: u16, message: &str) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        422 => "Unprocessable Content",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let body = serde_json::to_vec(&serde_json::json!({
        "ok": status < 400,
        "message": message,
    }))?;
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(&body)
}

fn generate_token() -> Result<String, std::io::Error> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|error| std::io::Error::other(format!("secure random failed: {error}")))?;

    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn locates_http_header_boundary() {
        assert_eq!(find_header_end(b"GET / HTTP/1.1\r\nX: y\r\n\r\n"), Some(20));
    }

    #[test]
    fn generated_tokens_are_random_and_fixed_length() {
        let first = generate_token().unwrap();
        let second = generate_token().unwrap();
        assert_eq!(first.len(), 64);
        assert_ne!(first, second);
    }

    #[test]
    fn accepts_authenticated_event_over_http() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let body = r#"{"source":"codex","event":"agent.turn.started"}"#;
        let client = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            write!(
                stream,
                "POST /v1/events HTTP/1.1\r\nAuthorization: Bearer test-token\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();
            response
        });

        let (events_tx, events_rx) = mpsc::channel();
        let (stream, _) = listener.accept().unwrap();
        handle_connection(stream, "test-token", &|event| {
            events_tx.send(event).map_err(|_| ())
        });

        let event = events_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(event.source, "codex");
        assert_eq!(event.event, "agent.turn.started");
        assert!(client.join().unwrap().starts_with("HTTP/1.1 202"));
    }
}
