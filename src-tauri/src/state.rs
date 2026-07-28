use aurapilot_core::app_paths::{profile_path, push_attempt_path, registry_path};
use aurapilot_core::config::CoreConfig;
use aurapilot_core::profile_registry::AgentProfileRegistry;
use aurapilot_core::project_registry::ProjectRegistry;
use aurapilot_core::push_attempt::PushAttemptStore;
use aurapilot_core::watcher::{ProjectChange, ProjectWatchService};
use std::sync::{Arc, Mutex};

pub struct AppState {
    pub config: CoreConfig,
    pub registry: Mutex<ProjectRegistry>,
    pub profiles: Mutex<AgentProfileRegistry>,
    pub push_attempts: Arc<Mutex<PushAttemptStore>>,
    pub watchers: Mutex<ProjectWatchService>,
}

impl AppState {
    pub fn load<F>(on_change: F) -> Result<Self, Box<dyn std::error::Error>>
    where
        F: Fn(ProjectChange) + Send + Sync + 'static,
    {
        let config = CoreConfig::default();
        let registry = ProjectRegistry::load(registry_path()?, config.clone())?;
        let profiles = AgentProfileRegistry::load(profile_path()?, config.clone())?;
        let push_attempts = PushAttemptStore::load(push_attempt_path()?, config.clone())?;
        let mut watchers = ProjectWatchService::new(&config, on_change)?;
        for project in registry.projects() {
            if project.path.join(".aurapilot").is_dir() {
                let _ = watchers.watch_project(project);
            }
        }
        Ok(Self {
            config,
            registry: Mutex::new(registry),
            profiles: Mutex::new(profiles),
            push_attempts: Arc::new(Mutex::new(push_attempts)),
            watchers: Mutex::new(watchers),
        })
    }
}
