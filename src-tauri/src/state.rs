use aurapilot_core::config::CoreConfig;
use aurapilot_core::project_registry::ProjectRegistry;
use aurapilot_core::watcher::{ProjectChange, ProjectWatchService};
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct AppState {
    pub config: CoreConfig,
    pub registry: Mutex<ProjectRegistry>,
    pub watchers: Mutex<ProjectWatchService>,
}

impl AppState {
    pub fn load<F>(on_change: F) -> Result<Self, Box<dyn std::error::Error>>
    where
        F: Fn(ProjectChange) + Send + Sync + 'static,
    {
        let config = CoreConfig::default();
        let registry = ProjectRegistry::load(registry_path()?, config.clone())?;
        let mut watchers = ProjectWatchService::new(&config, on_change)?;
        for project in registry.projects() {
            if project.path.join(".aurapilot").is_dir() {
                let _ = watchers.watch_project(project);
            }
        }
        Ok(Self {
            config,
            registry: Mutex::new(registry),
            watchers: Mutex::new(watchers),
        })
    }
}

pub fn registry_path() -> io::Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "home directory unavailable"))?;
    Ok(home.join(".aurapilot/config.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_path_is_independent_of_tauri_identifier() {
        let path = registry_path().unwrap();
        assert!(path.ends_with(".aurapilot/config.json"));
        assert!(!path.to_string_lossy().contains("dev.aurapilot.desktop"));
    }
}
