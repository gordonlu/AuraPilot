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
            detail: format!("executable found: {}", path.display()),
            resolved_path: Some(path),
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

pub fn open_folder(path: &Path) -> io::Result<Child> {
    imp::open_folder(path)
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

pub(crate) fn resolve_command(command: &str) -> Option<PathBuf> {
    let candidate = Path::new(command);
    if candidate.components().count() > 1 {
        return is_executable_file(candidate).then(|| candidate.to_path_buf());
    }
    imp::resolve_on_path(command)
}

fn resolve_in_directories(
    command: &str,
    directories: impl IntoIterator<Item = PathBuf>,
) -> Option<PathBuf> {
    directories.into_iter().find_map(|directory| {
        let candidate = directory.join(command);
        is_executable_file(&candidate).then_some(candidate)
    })
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
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

    pub fn open_folder(_path: &Path) -> io::Result<Child> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "opening folders is unsupported on this platform",
        ))
    }

    pub fn launch_terminal(_prepared: &PreparedLaunch, _executable: &Path) -> io::Result<Child> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "interactive terminal launch is unsupported on this platform",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[cfg(unix)]
    #[test]
    fn fallback_resolution_requires_an_executable_file() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempdir().unwrap();
        let candidate = temp.path().join("agent");
        fs::write(&candidate, "binary").unwrap();
        assert!(resolve_in_directories("agent", [temp.path().to_path_buf()]).is_none());
        let mut permissions = fs::metadata(&candidate).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&candidate, permissions).unwrap();
        assert_eq!(
            resolve_in_directories("agent", [temp.path().to_path_buf()]),
            Some(candidate)
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_fallbacks_include_the_opencode_installer_directory() {
        let home = Path::new("/home/example");
        assert!(imp::user_executable_directories(home).contains(&home.join(".opencode/bin")));
    }

    #[test]
    #[ignore = "requires AURAPILOT_EXPECTED_EXECUTABLE and a locally installed executable"]
    fn manually_verifies_a_real_fallback_executable() {
        let expected = std::env::var_os("AURAPILOT_EXPECTED_EXECUTABLE")
            .map(PathBuf::from)
            .expect("AURAPILOT_EXPECTED_EXECUTABLE is required");
        let command = expected
            .file_name()
            .and_then(|value| value.to_str())
            .expect("expected executable filename must be UTF-8");
        assert_eq!(resolve_command(command), Some(expected));
    }
}
