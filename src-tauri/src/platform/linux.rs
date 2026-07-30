use super::*;
use std::env;

pub fn resolve_on_path(command: &str) -> Option<PathBuf> {
    let path_directories = env::var_os("PATH")
        .map(|value| env::split_paths(&value).collect::<Vec<_>>())
        .unwrap_or_default();
    resolve_in_directories(command, path_directories).or_else(|| {
        dirs::home_dir()
            .and_then(|home| resolve_in_directories(command, user_executable_directories(&home)))
    })
}

pub(super) fn user_executable_directories(home: &Path) -> Vec<PathBuf> {
    [
        ".local/bin",
        "bin",
        ".cargo/bin",
        ".npm-global/bin",
        ".opencode/bin",
    ]
    .into_iter()
    .map(|relative| home.join(relative))
    .collect()
}

pub fn copy_text(text: &str) -> io::Result<()> {
    let candidates: [(&str, &[&str]); 3] = [
        ("wl-copy", &[]),
        ("xclip", &["-selection", "clipboard"]),
        ("xsel", &["--clipboard", "--input"]),
    ];
    for (program, args) in candidates {
        let Some(executable) = resolve_on_path(program) else {
            continue;
        };
        let mut child = Command::new(executable)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;
        child
            .stdin
            .take()
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::BrokenPipe, "clipboard stdin unavailable")
            })?
            .write_all(text.as_bytes())?;
        let output = child.wait_with_output()?;
        if output.status.success() {
            return Ok(());
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "no working clipboard provider found (wl-copy, xclip or xsel)",
    ))
}

pub fn open_folder(path: &Path) -> io::Result<Child> {
    let opener = resolve_on_path("xdg-open")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "xdg-open is unavailable"))?;
    Command::new(opener)
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
}

pub fn launch_terminal(prepared: &PreparedLaunch, executable: &Path) -> io::Result<Child> {
    if prepared.prompt_transport == PromptTransport::Stdin {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "stdin prompt transport is unsupported for interactive terminals",
        ));
    }
    let cwd = &prepared.working_directory;
    let mut command = if let Some(terminal) = resolve_on_path("gnome-terminal") {
        let mut command = Command::new(terminal);
        command
            .arg(format!("--working-directory={}", cwd.display()))
            .arg("--")
            .arg(executable)
            .args(&prepared.args);
        command
    } else if let Some(terminal) = resolve_on_path("konsole") {
        let mut command = Command::new(terminal);
        command
            .arg("--workdir")
            .arg(cwd)
            .arg("-e")
            .arg(executable)
            .args(&prepared.args);
        command
    } else if let Some(terminal) = resolve_on_path("kitty") {
        let mut command = Command::new(terminal);
        command
            .arg("--directory")
            .arg(cwd)
            .arg(executable)
            .args(&prepared.args);
        command
    } else if let Some(terminal) = resolve_on_path("alacritty") {
        let mut command = Command::new(terminal);
        command
            .arg("--working-directory")
            .arg(cwd)
            .arg("-e")
            .arg(executable)
            .args(&prepared.args);
        command
    } else if let Some(terminal) = resolve_on_path("x-terminal-emulator") {
        let mut command = Command::new(terminal);
        command
            .arg("-e")
            .arg(executable)
            .args(&prepared.args)
            .current_dir(cwd);
        command
    } else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no supported terminal emulator found",
        ));
    };
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
}
