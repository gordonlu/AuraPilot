use reqwest::StatusCode;
use reqwest::blocking::{Client, RequestBuilder};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub struct OpenCodeServer {
    child: Child,
    client: OpenCodeClient,
    status_poll_interval: Duration,
}

#[derive(Clone)]
struct OpenCodeClient {
    http: Client,
    base_url: String,
    username: String,
    password: String,
    error_body_limit_bytes: usize,
}

impl OpenCodeServer {
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        repository: &Path,
        executable: &Path,
        request_timeout: Duration,
        status_poll_interval: Duration,
        start_attempts: usize,
        error_body_limit_bytes: usize,
    ) -> Result<Self, String> {
        if start_attempts == 0 {
            return Err("OpenCode Server start attempts must be greater than zero".into());
        }
        let username = "aurapilot".to_owned();
        let password = uuid::Uuid::new_v4().to_string();
        let mut failures = Vec::new();
        for attempt in 1..=start_attempts {
            let port = reserve_loopback_port()?;
            let mut child = Command::new(executable)
                .args([
                    "serve",
                    "--hostname",
                    "127.0.0.1",
                    "--port",
                    &port.to_string(),
                ])
                .env("OPENCODE_SERVER_USERNAME", &username)
                .env("OPENCODE_SERVER_PASSWORD", &password)
                .current_dir(repository)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|error| format!("failed to start OpenCode Server: {error}"))?;
            consume_output(&mut child);
            let client = OpenCodeClient::new(
                format!("http://127.0.0.1:{port}"),
                username.clone(),
                password.clone(),
                request_timeout,
                error_body_limit_bytes,
            )?;
            match wait_for_health(&mut child, &client, request_timeout, status_poll_interval) {
                Ok(()) => {
                    return Ok(Self {
                        child,
                        client,
                        status_poll_interval,
                    });
                }
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    failures.push(format!("attempt {attempt}: {error}"));
                }
            }
        }
        Err(format!(
            "OpenCode Server did not become ready: {}",
            failures.join("; ")
        ))
    }

    pub fn process_id(&self) -> u32 {
        self.child.id()
    }

    pub fn create_session(&self, title: &str) -> Result<String, String> {
        self.client.create_session(title)
    }

    pub fn verify_session(&self, session_id: &str) -> Result<(), String> {
        self.client.verify_session(session_id)
    }

    pub fn session_is_idle(&self, session_id: &str) -> Result<bool, String> {
        self.client.session_is_idle(session_id)
    }

    pub fn prompt_async(
        &self,
        session_id: &str,
        message_id: &str,
        prompt: &str,
    ) -> Result<(), String> {
        self.client.prompt_async(session_id, message_id, prompt)
    }

    pub fn wait_for_message_completion(
        &mut self,
        session_id: &str,
        message_id: &str,
    ) -> Result<(), String> {
        loop {
            if let Some(status) = self
                .child
                .try_wait()
                .map_err(|error| format!("failed to inspect OpenCode Server: {error}"))?
            {
                return Err(format!(
                    "OpenCode Server exited with status {status} before message {message_id} completed"
                ));
            }
            match self.client.message_completion(session_id, message_id)? {
                MessageCompletion::Pending => {
                    std::thread::sleep(self.status_poll_interval);
                }
                MessageCompletion::Completed => return Ok(()),
                MessageCompletion::Failed(error) => {
                    return Err(format!("OpenCode message failed: {error}"));
                }
            }
        }
    }
}

impl Drop for OpenCodeServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl OpenCodeClient {
    fn new(
        base_url: String,
        username: String,
        password: String,
        request_timeout: Duration,
        error_body_limit_bytes: usize,
    ) -> Result<Self, String> {
        let http = Client::builder()
            .timeout(request_timeout)
            .build()
            .map_err(|error| format!("failed to build OpenCode HTTP client: {error}"))?;
        Ok(Self {
            http,
            base_url,
            username,
            password,
            error_body_limit_bytes,
        })
    }

    fn request(&self, request: RequestBuilder, operation: &str) -> Result<Value, String> {
        let response = request
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .map_err(|error| format!("OpenCode {operation} request failed: {error}"))?;
        let status = response.status();
        let body = response
            .text()
            .map_err(|error| format!("failed to read OpenCode {operation} response: {error}"))?;
        if !status.is_success() {
            return Err(format!(
                "OpenCode {operation} returned HTTP {status}: {}",
                truncate(&body, self.error_body_limit_bytes)
            ));
        }
        if status == StatusCode::NO_CONTENT || body.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&body)
            .map_err(|error| format!("invalid OpenCode {operation} JSON: {error}"))
    }

    fn get(&self, path: &str, operation: &str) -> Result<Value, String> {
        self.request(self.http.get(format!("{}{path}", self.base_url)), operation)
    }

    fn create_session(&self, title: &str) -> Result<String, String> {
        let response = self.request(
            self.http
                .post(format!("{}/session", self.base_url))
                .json(&json!({ "title": title })),
            "create Session",
        )?;
        response
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| "OpenCode create Session response did not contain id".to_owned())
    }

    fn verify_session(&self, session_id: &str) -> Result<(), String> {
        validate_path_id(session_id)?;
        let response = self.get(&format!("/session/{session_id}"), "read Session")?;
        let actual = response
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| "OpenCode read Session response did not contain id".to_owned())?;
        if actual != session_id {
            return Err(format!(
                "OpenCode read unexpected Session {actual}; expected {session_id}"
            ));
        }
        Ok(())
    }

    fn session_is_idle(&self, session_id: &str) -> Result<bool, String> {
        validate_path_id(session_id)?;
        let statuses = self.get("/session/status", "read Session status")?;
        let status = statuses
            .get(session_id)
            .ok_or_else(|| format!("OpenCode Session status was unavailable for {session_id}"))?;
        Ok(status_type(status) == Some("idle"))
    }

    fn prompt_async(&self, session_id: &str, message_id: &str, prompt: &str) -> Result<(), String> {
        validate_path_id(session_id)?;
        self.request(
            self.http
                .post(format!(
                    "{}/session/{session_id}/prompt_async",
                    self.base_url
                ))
                .json(&json!({
                    "messageID": message_id,
                    "parts": [{ "type": "text", "text": prompt }]
                })),
            "send asynchronous prompt",
        )?;
        Ok(())
    }

    fn message_completion(
        &self,
        session_id: &str,
        message_id: &str,
    ) -> Result<MessageCompletion, String> {
        validate_path_id(session_id)?;
        let messages = self.get(
            &format!("/session/{session_id}/message"),
            "list Session messages",
        )?;
        message_completion(&messages, message_id)
    }
}

#[derive(Debug, PartialEq, Eq)]
enum MessageCompletion {
    Pending,
    Completed,
    Failed(String),
}

fn message_completion(messages: &Value, message_id: &str) -> Result<MessageCompletion, String> {
    let messages = messages
        .as_array()
        .ok_or_else(|| "OpenCode message list response was not an array".to_owned())?;
    for message in messages {
        let Some(info) = message.get("info") else {
            continue;
        };
        if info.get("role").and_then(Value::as_str) != Some("assistant")
            || info.get("parentID").and_then(Value::as_str) != Some(message_id)
        {
            continue;
        }
        if let Some(error) = info.get("error") {
            return Ok(MessageCompletion::Failed(error.to_string()));
        }
        if info.pointer("/time/completed").is_some() {
            return Ok(MessageCompletion::Completed);
        }
    }
    Ok(MessageCompletion::Pending)
}

fn status_type(status: &Value) -> Option<&str> {
    status
        .as_str()
        .or_else(|| status.get("type").and_then(Value::as_str))
}

fn reserve_loopback_port() -> Result<u16, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("failed to reserve an OpenCode loopback port: {error}"))?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(|error| format!("failed to read the OpenCode loopback port: {error}"))
}

fn wait_for_health(
    child: &mut Child,
    client: &OpenCodeClient,
    timeout: Duration,
    poll_interval: Duration,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    let mut last_error = "health endpoint was not ready".to_owned();
    while Instant::now() < deadline {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("failed to inspect OpenCode Server: {error}"))?
        {
            return Err(format!("OpenCode Server exited with status {status}"));
        }
        match client.get("/global/health", "health check") {
            Ok(response) if response.get("healthy").and_then(Value::as_bool) == Some(true) => {
                return Ok(());
            }
            Ok(_) => last_error = "health response did not report healthy=true".into(),
            Err(error) => last_error = error,
        }
        std::thread::sleep(poll_interval);
    }
    Err(format!(
        "OpenCode Server startup timed out after {} ms; last error: {last_error}",
        timeout.as_millis()
    ))
}

fn consume_output(child: &mut Child) {
    if let Some(stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                eprintln!("opencode: {line}");
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                eprintln!("opencode: {line}");
            }
        });
    }
}

fn validate_path_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || !id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_".contains(character))
    {
        return Err("OpenCode Session ID contains unsupported URL path characters".into());
    }
    Ok(())
}

fn truncate(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let mut end = max_bytes.min(value.len());
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &value[..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::sync::mpsc;

    fn serve_once(status: &str, body: &str) -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 4096];
            loop {
                let read = stream.read(&mut buffer).unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
                let text = String::from_utf8_lossy(&request);
                let header_end = text.find("\r\n\r\n");
                let content_length = text
                    .lines()
                    .filter_map(|line| line.split_once(':'))
                    .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                    .and_then(|(_, value)| value.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                let chunked = text
                    .to_ascii_lowercase()
                    .contains("transfer-encoding: chunked");
                let complete = if chunked {
                    request.ends_with(b"\r\n0\r\n\r\n")
                } else {
                    header_end.is_some_and(|end| request.len() >= end + 4 + content_length)
                };
                if complete {
                    break;
                }
            }
            let _ = sender.send(String::from_utf8(request).unwrap());
            stream.write_all(response.as_bytes()).unwrap();
        });
        (format!("http://{address}"), receiver)
    }

    fn client(base_url: String) -> OpenCodeClient {
        OpenCodeClient::new(
            base_url,
            "aurapilot".into(),
            "secret".into(),
            Duration::from_secs(1),
            32,
        )
        .unwrap()
    }

    #[test]
    fn creates_session_and_reads_the_provider_id() {
        let (base_url, request) = serve_once("200 OK", r#"{"id":"ses_123"}"#);
        assert_eq!(
            client(base_url).create_session("TASK-001").unwrap(),
            "ses_123"
        );
        let request = request.recv().unwrap();
        assert!(request.starts_with("POST /session HTTP/1.1"));
        assert!(request.contains(r#"{"title":"TASK-001"}"#), "{request:?}");
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: basic ")
        );
    }

    #[test]
    fn sends_the_pointer_as_a_text_part_with_the_local_message_id() {
        let (base_url, request) = serve_once("204 No Content", "");
        client(base_url)
            .prompt_async("ses_123", "msg_local", "pointer")
            .unwrap();
        let request = request.recv().unwrap();
        assert!(request.starts_with("POST /session/ses_123/prompt_async HTTP/1.1"));
        assert!(
            request.contains(r#""messageID":"msg_local""#),
            "{request:?}"
        );
        assert!(request.contains(r#""parts":[{"text":"pointer","type":"text"}]"#));
    }

    #[test]
    fn completion_parser_distinguishes_pending_success_and_failure() {
        let pending = json!([]);
        assert_eq!(
            message_completion(&pending, "msg_1").unwrap(),
            MessageCompletion::Pending
        );
        let completed = json!([{
            "info": { "role": "assistant", "parentID": "msg_1", "time": { "completed": 1 } }
        }]);
        assert_eq!(
            message_completion(&completed, "msg_1").unwrap(),
            MessageCompletion::Completed
        );
        let failed = json!([{
            "info": { "role": "assistant", "parentID": "msg_1", "error": { "name": "APIError" } }
        }]);
        assert!(matches!(
            message_completion(&failed, "msg_1").unwrap(),
            MessageCompletion::Failed(error) if error.contains("APIError")
        ));
    }

    #[test]
    fn http_errors_are_bounded_and_visible() {
        let (base_url, _) = serve_once(
            "500 Internal Server Error",
            "abcdefghijklmnopqrstuvwxyz0123456789",
        );
        let error = client(base_url).create_session("TASK-001").unwrap_err();
        assert!(error.contains("HTTP 500 Internal Server Error"));
        assert!(error.ends_with('…'));
    }
}
