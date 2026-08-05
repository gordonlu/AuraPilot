use serde_json::{Value, json};
use std::collections::HashMap;
#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::fs::OpenOptions;
#[cfg(test)]
use std::io::{BufRead, Write};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
#[cfg(unix)]
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::Duration;
#[cfg(unix)]
use tungstenite::{Message, WebSocket, client, protocol::Role};

#[cfg(unix)]
const MANAGED_DAEMON_START_ATTEMPTS: usize = 40;
#[cfg(unix)]
const MANAGED_DAEMON_POLL_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManagedDaemonStatus {
    pub running: bool,
    pub detail: String,
}

#[cfg(unix)]
pub fn ensure_managed_daemon() -> Result<ManagedDaemonStatus, String> {
    let paths = managed_paths()?;
    fs::create_dir_all(&paths.directory)
        .map_err(|error| format!("failed to create the managed Codex directory: {error}"))?;
    if managed_socket_ready(&paths.socket) {
        return managed_daemon_status();
    }
    remove_stale_socket(&paths.socket)?;
    let log = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&paths.log)
        .map_err(|error| format!("failed to open the managed Codex log: {error}"))?;
    let mut command = Command::new("codex");
    command
        .arg("app-server")
        .arg("--listen")
        .arg(format!("unix://{}", paths.socket.display()))
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone().map_err(|error| {
            format!("failed to clone the managed Codex log: {error}")
        })?))
        .stderr(Stdio::from(log));
    use std::os::unix::process::CommandExt;
    command.process_group(0);
    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to start the managed Codex App Server: {error}"))?;
    for _ in 0..MANAGED_DAEMON_START_ATTEMPTS {
        if managed_socket_ready(&paths.socket) {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            return managed_daemon_status();
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("failed to inspect the managed Codex App Server: {error}"))?
        {
            return Err(format!(
                "managed Codex App Server exited with {status} before becoming ready; inspect {}",
                paths.log.display()
            ));
        }
        std::thread::sleep(MANAGED_DAEMON_POLL_INTERVAL);
    }
    Err(format!(
        "managed Codex App Server did not become ready; inspect {} and retry",
        paths.log.display()
    ))
}

#[cfg(not(unix))]
pub fn ensure_managed_daemon() -> Result<ManagedDaemonStatus, String> {
    Err("managed Codex CLI is currently supported only on Unix platforms".into())
}

pub fn managed_daemon_status() -> Result<ManagedDaemonStatus, String> {
    let paths = managed_paths()?;
    let running = managed_socket_ready(&paths.socket);
    Ok(ManagedDaemonStatus {
        running,
        detail: if running {
            format!("listening on unix://{}", paths.socket.display())
        } else {
            format!("not listening; log: {}", paths.log.display())
        },
    })
}

pub fn managed_remote_endpoint() -> Result<String, String> {
    Ok(format!("unix://{}", managed_paths()?.socket.display()))
}

#[cfg(unix)]
pub fn open_managed_tui(repository: &Path, thread_id: &str) -> Result<ExitStatus, String> {
    let endpoint = managed_remote_endpoint()?;
    Command::new("codex")
        .args(["resume", "--remote", &endpoint])
        .arg(thread_id)
        .arg("--cd")
        .arg(repository)
        .status()
        .map_err(|error| format!("failed to open the managed Codex CLI: {error}"))
}

#[cfg(not(unix))]
pub fn open_managed_tui(_repository: &Path, _thread_id: &str) -> Result<ExitStatus, String> {
    Err("managed Codex CLI is currently supported only on Unix platforms".into())
}

struct ManagedPaths {
    directory: PathBuf,
    socket: PathBuf,
    log: PathBuf,
}

fn managed_paths() -> Result<ManagedPaths, String> {
    let directory = dirs::home_dir()
        .ok_or_else(|| "home directory unavailable".to_owned())?
        .join(".aurapilot")
        .join("codex");
    Ok(ManagedPaths {
        socket: directory.join("app-server.sock"),
        log: directory.join("app-server.log"),
        directory,
    })
}

#[cfg(unix)]
fn managed_socket_ready(path: &Path) -> bool {
    std::os::unix::net::UnixStream::connect(path).is_ok()
}

#[cfg(not(unix))]
fn managed_socket_ready(_path: &Path) -> bool {
    false
}

#[cfg(unix)]
fn remove_stale_socket(path: &Path) -> Result<(), String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "failed to inspect the managed Codex socket: {error}"
            ));
        }
    };
    use std::os::unix::fs::FileTypeExt;
    if !metadata.file_type().is_socket() {
        return Err(format!(
            "managed Codex socket path is not a Unix socket: {}",
            path.display()
        ));
    }
    fs::remove_file(path)
        .map_err(|error| format!("failed to remove the stale managed Codex socket: {error}"))?;
    Ok(())
}

pub struct CodexAppSession {
    client: Arc<CodexClient>,
    events: mpsc::Receiver<Value>,
    pub thread_id: String,
}

#[derive(Clone)]
pub struct CodexLiveHandle {
    client: Arc<CodexClient>,
    thread_id: String,
}

struct PendingRequest {
    method: String,
    response: mpsc::SyncSender<Result<Value, String>>,
}

struct CodexClient {
    sink: Mutex<Box<dyn CodexMessageSink>>,
    pending: Mutex<HashMap<u64, PendingRequest>>,
    pending_changed: Condvar,
    next_request_id: AtomicU64,
    timeout: Duration,
}

#[derive(Clone, Debug)]
pub struct StartedTurn {
    pub turn_id: String,
    pub process_id: Option<u32>,
}

trait CodexMessageSink: Send {
    fn send(&mut self, message: &Value) -> Result<(), String>;
}

#[cfg(test)]
struct WriterSink<W: Write + Send>(W);

#[cfg(test)]
impl<W: Write + Send> CodexMessageSink for WriterSink<W> {
    fn send(&mut self, message: &Value) -> Result<(), String> {
        write_message_io(&mut self.0, message)
    }
}

#[cfg(unix)]
struct WebSocketSink(WebSocket<UnixStream>);

#[cfg(unix)]
impl CodexMessageSink for WebSocketSink {
    fn send(&mut self, message: &Value) -> Result<(), String> {
        let encoded = serde_json::to_string(message)
            .map_err(|error| format!("failed to encode Codex App Server request: {error}"))?;
        self.0
            .send(Message::Text(encoded.into()))
            .map_err(|error| format!("failed to write Codex App Server request: {error}"))
    }
}

impl CodexAppSession {
    pub fn verify_thread(thread_id: &str, timeout: Duration) -> Result<(), String> {
        let session = Self::connect(timeout)?;
        let result = session.request(
            "thread/read",
            json!({ "threadId": thread_id, "includeTurns": false }),
        )?;
        let found = result
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                "Codex thread/read response did not contain result.thread.id".to_owned()
            })?;
        if found != thread_id {
            return Err(format!(
                "Codex read unexpected thread {found}; expected {thread_id}"
            ));
        }
        Ok(())
    }

    pub fn create(repository: &Path, timeout: Duration) -> Result<Self, String> {
        let mut session = Self::connect(timeout)?;
        let result = session.request(
            "thread/start",
            json!({
                "cwd": repository,
                "serviceName": "aurapilot"
            }),
        )?;
        session.thread_id = result
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                "Codex thread/start response did not contain result.thread.id".to_owned()
            })?
            .to_owned();
        Ok(session)
    }

    pub fn resume(thread_id: &str, timeout: Duration) -> Result<Self, String> {
        let mut session = Self::connect(timeout)?;
        let result = session.request("thread/resume", json!({ "threadId": thread_id }))?;
        let resumed = result
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                "Codex thread/resume response did not contain result.thread.id".to_owned()
            })?;
        if resumed != thread_id {
            return Err(format!(
                "Codex resumed unexpected thread {resumed}; expected {thread_id}"
            ));
        }
        session.thread_id = resumed.to_owned();
        Ok(session)
    }

    pub fn fork(
        thread_id: &str,
        last_turn_id: Option<&str>,
        timeout: Duration,
    ) -> Result<Self, String> {
        let mut session = Self::connect(timeout)?;
        let mut params = json!({ "threadId": thread_id });
        if let Some(last_turn_id) = last_turn_id {
            params["lastTurnId"] = Value::String(last_turn_id.to_owned());
        }
        let result = session.request("thread/fork", params)?;
        session.thread_id = result
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                "Codex thread/fork response did not contain result.thread.id".to_owned()
            })?
            .to_owned();
        Ok(session)
    }

    fn connect(timeout: Duration) -> Result<Self, String> {
        #[cfg(not(unix))]
        {
            let _ = timeout;
            Err("managed Codex CLI is currently supported only on Unix platforms".into())
        }
        #[cfg(unix)]
        {
            ensure_managed_daemon()?;
            let (socket, reader_socket) = connect_managed_socket()?;
            let client = Arc::new(CodexClient {
                sink: Mutex::new(Box::new(WebSocketSink(socket))),
                pending: Mutex::new(HashMap::new()),
                pending_changed: Condvar::new(),
                next_request_id: AtomicU64::new(1),
                timeout,
            });
            let (event_sender, events) = mpsc::channel();
            let reader_client = client.clone();
            std::thread::spawn(move || {
                read_websocket_app_server(reader_socket, reader_client, event_sender)
            });
            let session = Self {
                client,
                events,
                thread_id: String::new(),
            };
            session.request(
                "initialize",
                json!({
                    "clientInfo": {
                        "name": "aurapilot",
                        "title": "AuraPilot",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }),
            )?;
            session.notify("initialized", json!({}))?;
            Ok(session)
        }
    }

    pub fn start_turn(&mut self, prompt: &str) -> Result<StartedTurn, String> {
        let result = self.request(
            "turn/start",
            json!({
                "threadId": self.thread_id,
                "input": [{ "type": "text", "text": prompt }]
            }),
        )?;
        let turn_id = result
            .pointer("/turn/id")
            .and_then(Value::as_str)
            .ok_or_else(|| "Codex turn/start response did not contain result.turn.id".to_owned())?
            .to_owned();
        Ok(StartedTurn {
            turn_id,
            process_id: None,
        })
    }

    pub fn wait_for_turn(&mut self, turn_id: &str) -> Result<(), String> {
        loop {
            let message = self
                .events
                .recv()
                .map_err(|_| "Codex App Server event stream closed unexpectedly".to_owned())?;
            if message.get("method").and_then(Value::as_str) == Some("turn/completed") {
                let completed_id = message.pointer("/params/turn/id").and_then(Value::as_str);
                if completed_id == Some(turn_id) {
                    return Ok(());
                }
            }
        }
    }

    pub fn live_handle(&self) -> CodexLiveHandle {
        CodexLiveHandle {
            client: self.client.clone(),
            thread_id: self.thread_id.clone(),
        }
    }

    pub fn wait_for_pending_requests(&self) {
        self.client.wait_for_pending_requests();
    }

    fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        self.client.request(method, params)
    }

    fn notify(&self, method: &str, params: Value) -> Result<(), String> {
        self.client
            .write(&json!({ "method": method, "params": params }))
    }
}

impl CodexLiveHandle {
    pub fn same_connection(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.client, &other.client)
    }

    pub fn steer_turn(&self, turn_id: &str, prompt: &str) -> Result<String, String> {
        let result = self.client.request(
            "turn/steer",
            json!({
                "threadId": self.thread_id,
                "expectedTurnId": turn_id,
                "input": [{ "type": "text", "text": prompt }]
            }),
        )?;
        result
            .get("turnId")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| "Codex turn/steer response did not contain result.turnId".to_owned())
    }

    pub fn interrupt_turn(&self, turn_id: &str) -> Result<(), String> {
        self.client.request(
            "turn/interrupt",
            json!({ "threadId": self.thread_id, "turnId": turn_id }),
        )?;
        Ok(())
    }
}

impl CodexClient {
    fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = mpsc::sync_channel(1);
        self.pending
            .lock()
            .map_err(|error| format!("Codex pending request lock poisoned: {error}"))?
            .insert(
                id,
                PendingRequest {
                    method: method.to_owned(),
                    response: sender,
                },
            );
        if let Err(error) = self.write(&json!({ "method": method, "id": id, "params": params })) {
            if let Ok(mut pending) = self.pending.lock() {
                pending.remove(&id);
            }
            return Err(error);
        }
        match receiver.recv_timeout(self.timeout) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Ok(mut pending) = self.pending.lock() {
                    pending.remove(&id);
                    self.pending_changed.notify_all();
                }
                Err(format!(
                    "Codex {method} timed out after {}ms; retry is not automatically safe",
                    self.timeout.as_millis()
                ))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(format!("Codex {method} response channel closed"))
            }
        }
    }

    fn write(&self, message: &Value) -> Result<(), String> {
        self.sink
            .lock()
            .map_err(|error| format!("Codex message sink lock poisoned: {error}"))?
            .send(message)
    }

    fn resolve(&self, message: &Value) -> Result<bool, String> {
        if message.get("id").is_some() && message.get("method").is_some() {
            let mut sink = self
                .sink
                .lock()
                .map_err(|error| format!("Codex message sink lock poisoned: {error}"))?;
            reject_server_request(&mut **sink, message)?;
            return Ok(true);
        }
        let Some(id) = message.get("id").and_then(Value::as_u64) else {
            return Ok(false);
        };
        let pending = self
            .pending
            .lock()
            .map_err(|error| format!("Codex pending request lock poisoned: {error}"))?
            .remove(&id);
        self.pending_changed.notify_all();
        if let Some(pending) = pending {
            let result = if let Some(error) = message.get("error") {
                Err(format!("Codex {} failed: {error}", pending.method))
            } else {
                message.get("result").cloned().ok_or_else(|| {
                    format!("Codex {} returned neither result nor error", pending.method)
                })
            };
            let _ = pending.response.send(result);
        }
        Ok(true)
    }

    fn fail_pending(&self, error: &str) {
        if let Ok(mut pending) = self.pending.lock() {
            for (_, request) in pending.drain() {
                let _ = request.response.send(Err(format!(
                    "Codex {} failed because the App Server connection closed: {error}",
                    request.method
                )));
            }
            self.pending_changed.notify_all();
        }
    }

    fn wait_for_pending_requests(&self) {
        if let Ok(pending) = self.pending.lock() {
            let _ = self
                .pending_changed
                .wait_timeout_while(pending, self.timeout, |requests| !requests.is_empty());
        }
    }
}

#[cfg(unix)]
fn connect_managed_socket() -> Result<(WebSocket<UnixStream>, WebSocket<UnixStream>), String> {
    let path = managed_paths()?.socket;
    let stream = UnixStream::connect(&path).map_err(|error| {
        format!(
            "failed to connect to managed Codex socket {}: {error}",
            path.display()
        )
    })?;
    let handshake_timeout = MANAGED_DAEMON_POLL_INTERVAL * MANAGED_DAEMON_START_ATTEMPTS as u32;
    stream
        .set_read_timeout(Some(handshake_timeout))
        .and_then(|()| stream.set_write_timeout(Some(handshake_timeout)))
        .map_err(|error| format!("failed to configure managed Codex socket: {error}"))?;
    let reader_stream = stream
        .try_clone()
        .map_err(|error| format!("failed to clone managed Codex socket: {error}"))?;
    let (writer_socket, _) = client("ws://localhost/", stream)
        .map_err(|error| format!("managed Codex WebSocket handshake failed: {error}"))?;
    reader_stream
        .set_read_timeout(Some(MANAGED_DAEMON_POLL_INTERVAL))
        .map_err(|error| format!("failed to configure managed Codex event timeout: {error}"))?;
    Ok((
        writer_socket,
        WebSocket::from_raw_socket(reader_stream, Role::Client, None),
    ))
}

#[cfg(unix)]
fn read_websocket_app_server(
    mut socket: WebSocket<UnixStream>,
    client: Arc<CodexClient>,
    events: mpsc::Sender<Value>,
) {
    loop {
        let frame = socket.read();
        let encoded = match frame {
            Ok(Message::Text(value)) => value.to_string(),
            Ok(Message::Binary(value)) => match String::from_utf8(value.to_vec()) {
                Ok(value) => value,
                Err(error) => {
                    client.fail_pending(&format!("invalid Codex WebSocket UTF-8: {error}"));
                    return;
                }
            },
            Ok(Message::Close(_)) => {
                client.fail_pending("Codex App Server closed the WebSocket");
                return;
            }
            Ok(_) => continue,
            Err(tungstenite::Error::Io(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                continue;
            }
            Err(error) => {
                client.fail_pending(&error.to_string());
                return;
            }
        };
        let message = match serde_json::from_str::<Value>(&encoded) {
            Ok(message) => message,
            Err(error) => {
                client.fail_pending(&format!("invalid JSON: {error}; message={encoded:?}"));
                return;
            }
        };
        match client.resolve(&message) {
            Ok(true) => {}
            Ok(false) => {
                if events.send(message).is_err() {
                    return;
                }
            }
            Err(error) => {
                client.fail_pending(&error);
                return;
            }
        }
    }
}

fn reject_server_request(sink: &mut dyn CodexMessageSink, message: &Value) -> Result<(), String> {
    sink.send(&server_request_rejection(message)?)
}

#[cfg(test)]
fn request_io(
    reader: &mut dyn BufRead,
    writer: &mut dyn Write,
    next_request_id: &mut u64,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    let id = *next_request_id;
    *next_request_id += 1;
    write_message_io(
        writer,
        &json!({ "method": method, "id": id, "params": params }),
    )?;
    loop {
        let message = read_message_io(reader)?;
        if message.get("id").and_then(Value::as_u64) == Some(id) {
            if let Some(error) = message.get("error") {
                return Err(format!("Codex {method} failed: {error}"));
            }
            return message
                .get("result")
                .cloned()
                .ok_or_else(|| format!("Codex {method} returned neither result nor error"));
        }
        if message.get("id").is_some() && message.get("method").is_some() {
            reject_server_request_io(writer, &message)?;
        }
    }
}

#[cfg(test)]
fn reject_server_request_io(writer: &mut dyn Write, message: &Value) -> Result<(), String> {
    write_message_io(writer, &server_request_rejection(message)?)
}

fn server_request_rejection(message: &Value) -> Result<Value, String> {
    let id = message
        .get("id")
        .cloned()
        .ok_or_else(|| "Codex server request did not contain an id".to_owned())?;
    let method = message
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    Ok(json!({
        "id": id,
        "error": {
            "code": -32001,
            "message": format!(
                "AuraPilot cannot answer Codex server request {method} yet; use a Profile whose policy does not require interactive approval"
            )
        }
    }))
}

#[cfg(test)]
fn write_message_io(writer: &mut dyn Write, message: &Value) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, message)
        .map_err(|error| format!("failed to encode Codex App Server request: {error}"))?;
    writer
        .write_all(b"\n")
        .and_then(|()| writer.flush())
        .map_err(|error| format!("failed to write Codex App Server request: {error}"))
}

#[cfg(test)]
fn read_message_io(reader: &mut dyn BufRead) -> Result<Value, String> {
    let mut line = String::new();
    let read = reader
        .read_line(&mut line)
        .map_err(|error| format!("failed to read Codex App Server response: {error}"))?;
    if read == 0 {
        return Err("Codex App Server closed its output unexpectedly".into());
    }
    serde_json::from_str(&line)
        .map_err(|error| format!("invalid Codex App Server JSON: {error}; line={line:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Cursor};

    #[derive(Clone)]
    struct SharedWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn mock_client() -> (Arc<CodexClient>, Arc<Mutex<Vec<u8>>>) {
        let output = Arc::new(Mutex::new(Vec::new()));
        (
            Arc::new(CodexClient {
                sink: Mutex::new(Box::new(WriterSink(SharedWriter(output.clone())))),
                pending: Mutex::new(HashMap::new()),
                pending_changed: Condvar::new(),
                next_request_id: AtomicU64::new(1),
                timeout: Duration::from_secs(1),
            }),
            output,
        )
    }

    fn wait_for_pending(client: &CodexClient) {
        let deadline = std::time::Instant::now() + client.timeout;
        while std::time::Instant::now() < deadline {
            if !client.pending.lock().unwrap().is_empty() {
                return;
            }
            std::thread::yield_now();
        }
        panic!("mock Codex request was not registered before timeout");
    }

    #[test]
    fn mock_app_server_resumes_the_same_thread_then_starts_a_turn() {
        let responses = concat!(
            "{\"id\":91,\"method\":\"item/commandExecution/requestApproval\",\"params\":{}}\n",
            "{\"id\":1,\"result\":{\"thread\":{\"id\":\"thr_existing\"}}}\n",
            "{\"id\":2,\"result\":{\"turn\":{\"id\":\"turn_next\"}}}\n"
        );
        let mut reader = Cursor::new(responses.as_bytes());
        let mut writer = Vec::new();
        let mut next_id = 1;

        let resumed = request_io(
            &mut reader,
            &mut writer,
            &mut next_id,
            "thread/resume",
            json!({ "threadId": "thr_existing" }),
        )
        .unwrap();
        assert_eq!(resumed.pointer("/thread/id").unwrap(), "thr_existing");
        let turn = request_io(
            &mut reader,
            &mut writer,
            &mut next_id,
            "turn/start",
            json!({
                "threadId": "thr_existing",
                "input": [{ "type": "text", "text": "pointer" }]
            }),
        )
        .unwrap();
        assert_eq!(turn.pointer("/turn/id").unwrap(), "turn_next");

        let sent = String::from_utf8(writer).unwrap();
        let messages = sent
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0]["method"], "thread/resume");
        assert_eq!(messages[0]["params"]["threadId"], "thr_existing");
        assert_eq!(messages[1]["id"], 91);
        assert_eq!(messages[1]["error"]["code"], -32001);
        assert_eq!(messages[2]["method"], "turn/start");
        assert_eq!(messages[2]["params"]["threadId"], "thr_existing");
        assert_eq!(messages[2]["params"]["input"][0]["text"], "pointer");
    }

    #[test]
    fn mock_app_server_errors_remain_visible() {
        let mut reader =
            Cursor::new(b"{\"id\":1,\"error\":{\"code\":-32000,\"message\":\"thread missing\"}}\n");
        let mut writer = Vec::new();
        let mut next_id = 1;
        let error = request_io(
            &mut reader,
            &mut writer,
            &mut next_id,
            "thread/resume",
            json!({ "threadId": "thr_missing" }),
        )
        .unwrap_err();
        assert!(error.contains("thread missing"));
    }

    #[test]
    fn mock_app_server_fork_uses_the_source_thread_and_returns_a_new_id() {
        let mut reader = Cursor::new(
            b"{\"id\":1,\"result\":{\"thread\":{\"id\":\"thr_forked\",\"forkedFromId\":\"thr_source\"}}}\n",
        );
        let mut writer = Vec::new();
        let mut next_id = 1;
        let result = request_io(
            &mut reader,
            &mut writer,
            &mut next_id,
            "thread/fork",
            json!({ "threadId": "thr_source" }),
        )
        .unwrap();

        assert_eq!(result.pointer("/thread/id").unwrap(), "thr_forked");
        let sent: Value = serde_json::from_slice(&writer).unwrap();
        assert_eq!(sent["method"], "thread/fork");
        assert_eq!(sent["params"]["threadId"], "thr_source");
        assert!(sent["params"].get("lastTurnId").is_none());
    }

    #[test]
    fn live_handle_dispatches_steer_and_interrupt_responses_without_owning_event_reader() {
        let (client, output) = mock_client();
        let handle = CodexLiveHandle {
            client: client.clone(),
            thread_id: "thr_live".into(),
        };
        let steer = std::thread::spawn(move || handle.steer_turn("turn_active", "追加要求"));
        wait_for_pending(&client);
        client
            .resolve(&json!({ "id": 1, "result": { "turnId": "turn_active" } }))
            .unwrap();
        assert_eq!(steer.join().unwrap().unwrap(), "turn_active");

        let handle = CodexLiveHandle {
            client: client.clone(),
            thread_id: "thr_live".into(),
        };
        let interrupt = std::thread::spawn(move || handle.interrupt_turn("turn_active"));
        wait_for_pending(&client);
        client.resolve(&json!({ "id": 2, "result": {} })).unwrap();
        interrupt.join().unwrap().unwrap();

        let sent = String::from_utf8(output.lock().unwrap().clone()).unwrap();
        let messages = sent
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(messages[0]["method"], "turn/steer");
        assert_eq!(messages[0]["params"]["expectedTurnId"], "turn_active");
        assert_eq!(messages[1]["method"], "turn/interrupt");
        assert_eq!(messages[1]["params"]["turnId"], "turn_active");
        assert!(
            messages
                .iter()
                .all(|message| message["params"]["threadId"] == "thr_live")
        );
    }

    #[test]
    #[ignore = "requires a real Codex login and AURAPILOT_CODEX_THREAD_ID"]
    fn verifies_a_real_thread_through_the_managed_websocket() {
        let thread_id = std::env::var("AURAPILOT_CODEX_THREAD_ID")
            .expect("AURAPILOT_CODEX_THREAD_ID is required");
        ensure_managed_daemon().unwrap();
        CodexAppSession::verify_thread(&thread_id, Duration::from_secs(15)).unwrap();
        assert!(managed_daemon_status().unwrap().running);
    }
}
