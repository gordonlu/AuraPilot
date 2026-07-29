use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::Duration;

pub struct ClaudeProcess {
    child: Child,
    messages: Receiver<Result<Value, String>>,
    session_id: Option<String>,
    timeout: Duration,
}

impl ClaudeProcess {
    pub fn start(repository: &Path, prompt: &str, timeout: Duration) -> Result<Self, String> {
        Self::spawn(repository, None, prompt, timeout)
    }

    pub fn resume(
        repository: &Path,
        session_id: &str,
        prompt: &str,
        timeout: Duration,
    ) -> Result<Self, String> {
        Self::spawn(repository, Some(session_id), prompt, timeout)
    }

    fn spawn(
        repository: &Path,
        session_id: Option<&str>,
        prompt: &str,
        timeout: Duration,
    ) -> Result<Self, String> {
        let mut command = Command::new("claude");
        command.args(["-p", "--output-format", "stream-json", "--verbose"]);
        if let Some(session_id) = session_id {
            command.args(["--resume", session_id]);
        }
        let mut child = command
            .arg(prompt)
            .current_dir(repository)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("failed to start Claude Code: {error}"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Claude Code stdout is unavailable".to_owned())?;
        let (sender, messages) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let message = line
                    .map_err(|error| format!("failed to read Claude stream-json: {error}"))
                    .and_then(|line| {
                        serde_json::from_str(&line).map_err(|error| {
                            format!("invalid Claude stream-json: {error}; line={line:?}")
                        })
                    });
                if sender.send(message).is_err() {
                    break;
                }
            }
        });
        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    eprintln!("claude: {line}");
                }
            });
        }
        Ok(Self {
            child,
            messages,
            session_id: None,
            timeout,
        })
    }

    pub fn identify_session(&mut self, expected: Option<&str>) -> Result<String, String> {
        let session_id = loop {
            let message = self.next_message("Session identification")?;
            if message.get("type").and_then(Value::as_str) == Some("system")
                && message.get("subtype").and_then(Value::as_str) == Some("init")
            {
                break message
                    .get("session_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .ok_or_else(|| "Claude system/init did not contain session_id".to_owned())?;
            }
        };
        if let Some(expected) = expected
            && session_id != expected
        {
            return Err(format!(
                "Claude Code resumed unexpected Session {session_id}; expected {expected}"
            ));
        }
        self.session_id = Some(session_id.clone());
        Ok(session_id)
    }

    pub fn wait_for_completion(mut self) -> Result<(), String> {
        let session_id = self
            .session_id
            .as_deref()
            .ok_or_else(|| "Claude Session was not identified before monitoring".to_owned())?;
        let result = loop {
            let message = self.next_message_blocking("turn completion")?;
            if message.get("type").and_then(Value::as_str) == Some("result") {
                break validate_result(&message, session_id);
            }
        };
        let status = self
            .child
            .wait()
            .map_err(|error| format!("failed to wait for Claude Code: {error}"))?;
        result?;
        if !status.success() {
            return Err(format!("Claude Code exited with status {status}"));
        }
        Ok(())
    }

    pub fn process_id(&self) -> u32 {
        self.child.id()
    }

    fn next_message(&self, phase: &str) -> Result<Value, String> {
        receive_message(&self.messages, self.timeout, phase)
    }

    fn next_message_blocking(&self, phase: &str) -> Result<Value, String> {
        self.messages
            .recv()
            .map_err(|_| format!("Claude Code closed stdout during {phase}"))?
    }
}

fn receive_message(
    messages: &Receiver<Result<Value, String>>,
    timeout: Duration,
    phase: &str,
) -> Result<Value, String> {
    match messages.recv_timeout(timeout) {
        Ok(message) => message,
        Err(RecvTimeoutError::Timeout) => Err(format!(
            "Claude Code {phase} timed out after {} ms",
            timeout.as_millis()
        )),
        Err(RecvTimeoutError::Disconnected) => {
            Err(format!("Claude Code closed stdout during {phase}"))
        }
    }
}

#[cfg(test)]
fn identify_session_from(reader: &mut dyn BufRead) -> Result<String, String> {
    loop {
        let message = read_message(reader)?;
        if message.get("type").and_then(Value::as_str) == Some("system")
            && message.get("subtype").and_then(Value::as_str) == Some("init")
        {
            return message
                .get("session_id")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| "Claude system/init did not contain session_id".to_owned());
        }
    }
}

#[cfg(test)]
fn wait_for_result_from(reader: &mut dyn BufRead, expected_session_id: &str) -> Result<(), String> {
    loop {
        let message = read_message(reader)?;
        if message.get("type").and_then(Value::as_str) != Some("result") {
            continue;
        }
        return validate_result(&message, expected_session_id);
    }
}

fn validate_result(message: &Value, expected_session_id: &str) -> Result<(), String> {
    let session_id = message
        .get("session_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "Claude result did not contain session_id".to_owned())?;
    if session_id != expected_session_id {
        return Err(format!(
            "Claude result Session {session_id} did not match {expected_session_id}"
        ));
    }
    if message
        .get("is_error")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(format!(
            "Claude turn failed: {}",
            message
                .get("subtype")
                .and_then(Value::as_str)
                .unwrap_or("unknown error")
        ));
    }
    Ok(())
}

#[cfg(test)]
fn read_message(reader: &mut dyn BufRead) -> Result<Value, String> {
    let mut line = String::new();
    let read = reader
        .read_line(&mut line)
        .map_err(|error| format!("failed to read Claude stream-json: {error}"))?;
    if read == 0 {
        return Err("Claude Code closed stdout before emitting the expected event".into());
    }
    serde_json::from_str(&line)
        .map_err(|error| format!("invalid Claude stream-json: {error}; line={line:?}"))
}

impl Drop for ClaudeProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn captures_init_session_before_the_result_and_validates_completion() {
        let stream = concat!(
            "{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"session-1\"}\n",
            "{\"type\":\"assistant\",\"session_id\":\"session-1\",\"message\":{}}\n",
            "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"session_id\":\"session-1\"}\n"
        );
        let mut reader = Cursor::new(stream.as_bytes());
        let session_id = identify_session_from(&mut reader).unwrap();
        assert_eq!(session_id, "session-1");
        wait_for_result_from(&mut reader, &session_id).unwrap();
    }

    #[test]
    fn result_errors_and_session_mismatches_are_visible() {
        let mut failed = Cursor::new(
            b"{\"type\":\"result\",\"subtype\":\"error_during_execution\",\"is_error\":true,\"session_id\":\"session-1\"}\n",
        );
        assert!(
            wait_for_result_from(&mut failed, "session-1")
                .unwrap_err()
                .contains("error_during_execution")
        );
        let mut mismatch = Cursor::new(
            b"{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"session_id\":\"other\"}\n",
        );
        assert!(
            wait_for_result_from(&mut mismatch, "session-1")
                .unwrap_err()
                .contains("did not match")
        );
    }

    #[test]
    fn stalled_session_identification_times_out_visibly() {
        let (_sender, messages) = mpsc::channel();
        let error = receive_message(
            &messages,
            Duration::from_millis(1),
            "Session identification",
        )
        .unwrap_err();
        assert_eq!(
            error,
            "Claude Code Session identification timed out after 1 ms"
        );
    }
}
