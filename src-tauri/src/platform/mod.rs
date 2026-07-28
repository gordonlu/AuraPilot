use aurapilot_core::agent_profile::{LaunchMode, PreparedLaunch, PromptTransport};
use serde::Serialize;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

#[derive(Clone, Debug, Serialize)]
pub struct ExecutableAvailability {
    pub available: bool,
    pub resolved_path: Option<PathBuf>,
    pub detail: String,
}

pub fn detect_command(command: &str) -> ExecutableAvailability {
    match resolve_command(command) {
        Some(path) => ExecutableAvailability {
            available: true,
            resolved_path: Some(path),
            detail: "executable found".into(),
        },
        None => ExecutableAvailability {
            available: false,
            resolved_path: None,
            detail: format!("executable not found: {command}"),
        },
    }
}

pub fn copy_text(text: &str) -> io::Result<()> {
    imp::copy_text(text)
}

pub fn launch(prepared: &PreparedLaunch) -> io::Result<Child> {
    if prepared.launch_mode == LaunchMode::ClipboardOnly {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "clipboard-only profiles do not launch a process",
        ));
    }
    let executable = resolve_command(&prepared.executable).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("executable not found: {}", prepared.executable),
        )
    })?;
    match prepared.launch_mode {
        LaunchMode::HeadlessProcess => launch_headless(prepared, &executable),
        LaunchMode::InteractiveTerminal => imp::launch_terminal(prepared, &executable),
        LaunchMode::ClipboardOnly => unreachable!(),
    }
}

fn launch_headless(prepared: &PreparedLaunch, executable: &Path) -> io::Result<Child> {
    let mut command = Command::new(executable);
    command
        .args(&prepared.args)
        .current_dir(&prepared.working_directory)
        .stdin(if prepared.prompt_transport == PromptTransport::Stdin {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = command.spawn()?;
    if prepared.prompt_transport == PromptTransport::Stdin {
        let mut stdin = child.stdin.take().ok_or_else(|| {
            io::Error::new(io::ErrorKind::BrokenPipe, "agent stdin is unavailable")
        })?;
        stdin.write_all(prepared.prompt.as_bytes())?;
        stdin.write_all(b"\n")?;
    }
    Ok(child)
}

fn resolve_command(command: &str) -> Option<PathBuf> {
    let candidate = Path::new(command);
    if candidate.components().count() > 1 {
        return candidate.is_file().then(|| candidate.to_path_buf());
    }
    imp::resolve_on_path(command)
}

#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod imp;
#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod imp;
#[cfg(target_os = "windows")]
#[path = "windows.rs"]
mod imp;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
mod imp {
    use super::*;

    pub fn resolve_on_path(_command: &str) -> Option<PathBuf> {
        None
    }

    pub fn copy_text(_text: &str) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "clipboard is unsupported on this platform",
        ))
    }

    pub fn launch_terminal(_prepared: &PreparedLaunch, _executable: &Path) -> io::Result<Child> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "interactive terminal launch is unsupported on this platform",
        ))
    }
}
