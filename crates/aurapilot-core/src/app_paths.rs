use std::io;
use std::path::PathBuf;

pub fn config_directory() -> io::Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "home directory unavailable"))?;
    Ok(home.join(".aurapilot"))
}

pub fn registry_path() -> io::Result<PathBuf> {
    Ok(config_directory()?.join("config.json"))
}

pub fn profile_path() -> io::Result<PathBuf> {
    Ok(config_directory()?.join("agent-profiles.json"))
}

pub fn push_attempt_path() -> io::Result<PathBuf> {
    Ok(config_directory()?.join("push-attempts.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_paths_do_not_depend_on_the_tauri_identifier() {
        for path in [registry_path(), profile_path(), push_attempt_path()] {
            let path = path.unwrap();
            assert!(path.to_string_lossy().contains(".aurapilot/"));
            assert!(!path.to_string_lossy().contains("dev.aurapilot.desktop"));
        }
    }
}
