use serde_json::{Value, json};
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

pub struct CodexAppSession {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_request_id: u64,
    pub thread_id: String,
}

#[derive(Clone, Debug)]
pub struct StartedTurn {
    pub turn_id: String,
    pub process_id: u32,
}

impl CodexAppSession {
    pub fn create(repository: &Path) -> Result<Self, String> {
        let mut session = Self::connect()?;
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

    pub fn resume(thread_id: &str) -> Result<Self, String> {
        let mut session = Self::connect()?;
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

    fn connect() -> Result<Self, String> {
        let mut child = Command::new("codex")
            .args(["app-server", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("failed to start Codex App Server: {error}"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Codex App Server stdin is unavailable".to_owned())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Codex App Server stdout is unavailable".to_owned())?;
        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    eprintln!("codex app-server: {line}");
                }
            });
        }
        let mut session = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_request_id: 1,
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
            process_id: self.child.id(),
        })
    }

    pub fn wait_for_turn(&mut self, turn_id: &str) -> Result<(), String> {
        loop {
            let message = self.read_message()?;
            if message.get("method").and_then(Value::as_str) == Some("turn/completed") {
                let completed_id = message.pointer("/params/turn/id").and_then(Value::as_str);
                if completed_id == Some(turn_id) {
                    return Ok(());
                }
            }
            if message.get("id").is_some() && message.get("method").is_some() {
                self.reject_server_request(&message)?;
            }
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        request_io(
            &mut self.stdout,
            &mut self.stdin,
            &mut self.next_request_id,
            method,
            params,
        )
    }

    fn notify(&mut self, method: &str, params: Value) -> Result<(), String> {
        self.write_message(&json!({ "method": method, "params": params }))
    }

    fn reject_server_request(&mut self, message: &Value) -> Result<(), String> {
        reject_server_request_io(&mut self.stdin, message)
    }

    fn write_message(&mut self, message: &Value) -> Result<(), String> {
        write_message_io(&mut self.stdin, message)
    }

    fn read_message(&mut self) -> Result<Value, String> {
        read_message_io(&mut self.stdout)
    }
}

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

fn reject_server_request_io(writer: &mut dyn Write, message: &Value) -> Result<(), String> {
    let id = message
        .get("id")
        .cloned()
        .ok_or_else(|| "Codex server request did not contain an id".to_owned())?;
    let method = message
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    write_message_io(
        writer,
        &json!({
            "id": id,
            "error": {
                "code": -32001,
                "message": format!(
                    "AuraPilot cannot answer Codex server request {method} yet; use a Profile whose policy does not require interactive approval"
                )
            }
        }),
    )
}

fn write_message_io(writer: &mut dyn Write, message: &Value) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, message)
        .map_err(|error| format!("failed to encode Codex App Server request: {error}"))?;
    writer
        .write_all(b"\n")
        .and_then(|()| writer.flush())
        .map_err(|error| format!("failed to write Codex App Server request: {error}"))
}

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

impl Drop for CodexAppSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn _assert_io_error_is_send(_: io::Error) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

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
}
