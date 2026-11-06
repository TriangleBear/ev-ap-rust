//! A tiny HTTP server that impersonates the Google Apps Script Web App.
//!
//! The Apps Script itself cannot run in CI, so the Sheets backend is tested
//! against a local server that speaks the same HTTP contract: the `action`
//! query parameter selects the handler and the response is the JSON object the
//! script would return.
//!
//! A hand written server (rather than a mocking library) is used so the tests
//! can produce the exact failure modes that matter — a 404, a 302, an HTML
//! sign-in page instead of JSON — which are precisely the cases the error
//! messages have to cover.

#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// A request the fake Apps Script received.
#[derive(Debug, Clone)]
pub struct RecordedRequest {
    pub method: String,
    pub path: String,
    pub query: String,
    pub body: String,
}

impl RecordedRequest {
    /// The `action` query parameter.
    pub fn action(&self) -> String {
        self.param("action").unwrap_or_default()
    }

    /// A decoded query parameter.
    pub fn param(&self, key: &str) -> Option<String> {
        for pair in self.query.split('&') {
            let mut parts = pair.splitn(2, '=');
            let name = parts.next()?;
            if name == key {
                return Some(percent_decode(parts.next().unwrap_or_default()));
            }
        }
        None
    }

    /// The request body parsed as JSON.
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).unwrap_or(serde_json::Value::Null)
    }
}

/// A canned HTTP response.
#[derive(Debug, Clone)]
pub struct CannedResponse {
    pub status: u16,
    pub body: String,
    pub content_type: String,
}

impl CannedResponse {
    /// A JSON response.
    pub fn json(status: u16, body: impl Into<String>) -> Self {
        CannedResponse {
            status,
            body: body.into(),
            content_type: "application/json".to_string(),
        }
    }

    /// A response with an arbitrary content type — used for the HTML sign-in
    /// page and the non-JSON failure modes.
    pub fn text(status: u16, content_type: &str, body: impl Into<String>) -> Self {
        CannedResponse {
            status,
            body: body.into(),
            content_type: content_type.to_string(),
        }
    }
}

/// The fake Apps Script server.
pub struct FakeAppsScript {
    /// Deployment URL to hand to the backend.
    pub url: String,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl FakeAppsScript {
    /// Start a server that answers the given `action` → response routes.
    ///
    /// Unknown actions get a `404` JSON body, the way the Apps Script's
    /// `default:` branch answers an unknown action with `success: false`.
    pub fn start(routes: HashMap<String, CannedResponse>) -> Self {
        FakeAppsScript::start_with_default(routes, None)
    }

    /// Start a server that answers every action — known or not — with the
    /// same response.
    pub fn always(response: CannedResponse) -> Self {
        FakeAppsScript::start_with_default(HashMap::new(), Some(response))
    }

    /// Start a server with an explicit fallback for unknown actions.
    pub fn start_with_default(
        routes: HashMap<String, CannedResponse>,
        default_response: Option<CannedResponse>,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let url = format!("http://{}/exec", listener.local_addr().unwrap());
        listener.set_nonblocking(true).expect("set nonblocking");

        let requests = Arc::new(Mutex::new(Vec::new()));
        let shutdown = Arc::new(AtomicBool::new(false));

        let thread_requests = Arc::clone(&requests);
        let thread_shutdown = Arc::clone(&shutdown);
        let handle = std::thread::spawn(move || {
            while !thread_shutdown.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let _ = handle_connection(
                            stream,
                            &routes,
                            default_response.as_ref(),
                            &thread_requests,
                        );
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(2));
                    }
                    Err(_) => break,
                }
            }
        });

        FakeAppsScript {
            url,
            requests,
            shutdown,
            handle: Some(handle),
        }
    }

    /// Every request the server received, in order.
    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.requests
            .lock()
            .map(|log| log.clone())
            .unwrap_or_default()
    }

    /// The most recent request.
    pub fn last_request(&self) -> Option<RecordedRequest> {
        self.requests().last().cloned()
    }

    /// How many requests were received.
    pub fn request_count(&self) -> usize {
        self.requests.lock().map(|log| log.len()).unwrap_or(0)
    }
}

impl Drop for FakeAppsScript {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn handle_connection(
    mut stream: TcpStream,
    routes: &HashMap<String, CannedResponse>,
    default_response: Option<&CannedResponse>,
    requests: &Arc<Mutex<Vec<RecordedRequest>>>,
) -> std::io::Result<()> {
    // The listener is non-blocking so the accept loop can be shut down; the
    // accepted socket inherits that flag on some platforms and must be put
    // back into blocking mode before reading from it.
    stream.set_nonblocking(false)?;
    let mut reader = BufReader::new(stream.try_clone()?);

    let mut request_line = String::new();
    if reader.read_line(&mut request_line)? == 0 {
        return Ok(());
    }

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or_default().to_string();

    let mut content_length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 {
            break;
        }
        let trimmed = header.trim_end();
        if trimmed.is_empty() {
            break;
        }
        if let Some((name, value)) = trimmed.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap_or(0);
            }
        }
    }

    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }
    let body = String::from_utf8_lossy(&body).into_owned();

    let (path, query) = match target.split_once('?') {
        Some((path, query)) => (path.to_string(), query.to_string()),
        None => (target.clone(), String::new()),
    };

    let recorded = RecordedRequest {
        method: method.clone(),
        path: path.clone(),
        query: query.clone(),
        body,
    };

    let action = recorded.action();
    if let Ok(mut log) = requests.lock() {
        log.push(recorded);
    }

    let response = routes.get(&action).cloned().unwrap_or_else(|| {
        default_response.cloned().unwrap_or_else(|| {
            // The real Apps Script answers an unknown action with HTTP 200 and
            // a JSON body carrying `success: false`.
            CannedResponse::json(
                200,
                format!("{{\"success\": false, \"error\": \"Unknown action: {action}\"}}"),
            )
        })
    });

    let reason = reason_phrase(response.status);
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response.status,
        reason,
        response.content_type,
        response.body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(response.body.as_bytes())?;
    stream.flush()?;
    Ok(())
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Unknown",
    }
}

/// Decode `%xx` escapes and `+` in a query parameter.
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(byte) => {
                        out.push(byte);
                        index += 3;
                    }
                    Err(_) => {
                        out.push(bytes[index]);
                        index += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
