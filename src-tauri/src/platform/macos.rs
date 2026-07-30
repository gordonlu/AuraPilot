use super::*;
use std::env;

pub fn resolve_on_path(command: &str) -> Option<PathBuf> {
    let path_directories = env::var_os("PATH")
        .map(|value| env::split_paths(&value).collect::<Vec<_>>())
        .unwrap_or_default();
    resolve_in_directories(command, path_directories).or_else(|| {
        let mut directories = vec![
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
        ];
        if let Some(home) = dirs::home_dir() {
            directories.extend(
                [
                    ".local/bin",
                    "bin",
                    ".cargo/bin",
                    ".npm-global/bin",
                    ".opencode/bin",
                ]
                .into_iter()
                .map(|relative| home.join(relative)),
            );
        }
        resolve_in_directories(command, directories)
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

pub fn open_folder(path: &Path) -> io::Result<Child> {
    Command::new("/usr/bin/open")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
}

pub fn launch_terminal(_prepared: &PreparedLaunch, _executable: &Path) -> io::Result<Child> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "safe interactive terminal launch is unavailable; Pointer Prompt can be copied instead",
    ))
}
