use super::*;
use std::env;

pub fn resolve_on_path(command: &str) -> Option<PathBuf> {
    env::split_paths(&env::var_os("PATH")?).find_map(|directory| {
        let candidate = directory.join(command);
        candidate.is_file().then_some(candidate)
    })
}

pub fn copy_text(text: &str) -> io::Result<()> {
    let mut child = Command::new("/usr/bin/pbcopy")
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
        .ok_or_else(|| io::Error::other("pbcopy failed"))
}

pub fn launch_terminal(_prepared: &PreparedLaunch, _executable: &Path) -> io::Result<Child> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "safe interactive terminal launch is unavailable; Pointer Prompt can be copied instead",
    ))
}
