use super::*;
use std::env;

pub fn resolve_on_path(command: &str) -> Option<PathBuf> {
    let extensions = env::var_os("PATHEXT")
        .map(|value| {
            value
                .to_string_lossy()
                .split(';')
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![".COM".into(), ".EXE".into(), ".BAT".into(), ".CMD".into()]);
    env::split_paths(&env::var_os("PATH")?).find_map(|directory| {
        let direct = directory.join(command);
        if direct.is_file() {
            return Some(direct);
        }
        extensions.iter().find_map(|extension| {
            let candidate = directory.join(format!("{command}{extension}"));
            candidate.is_file().then_some(candidate)
        })
    })
}

pub fn copy_text(text: &str) -> io::Result<()> {
    let powershell = resolve_on_path("powershell")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "PowerShell is unavailable"))?;
    let mut child = Command::new(powershell)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "$input | Set-Clipboard",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "clipboard stdin unavailable"))?
        .write_all(text.as_bytes())?;
    let status = child.wait()?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| io::Error::other("Set-Clipboard failed"))
}

pub fn launch_terminal(prepared: &PreparedLaunch, executable: &Path) -> io::Result<Child> {
    if prepared.prompt_transport == PromptTransport::Stdin {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "stdin prompt transport is unsupported for interactive terminals",
        ));
    }
    let terminal = resolve_on_path("wt").ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "Windows Terminal is unavailable")
    })?;
    Command::new(terminal)
        .arg("-d")
        .arg(&prepared.working_directory)
        .arg(executable)
        .args(&prepared.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
}
