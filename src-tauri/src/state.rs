use crate::providers::codex::CodexLiveHandle;
use aurapilot_core::app_paths::{
    profile_path, push_attempt_path, registry_path, runtime_database_path,
};
use aurapilot_core::config::CoreConfig;
use aurapilot_core::profile_registry::AgentProfileRegistry;
use aurapilot_core::project_registry::ProjectRegistry;
use aurapilot_core::push_attempt::PushAttemptStore;
use aurapilot_core::runtime_store::RuntimeStore;
use aurapilot_core::watcher::{ProjectChange, ProjectWatchService};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub struct AppState {
    pub config: CoreConfig,
    pub registry: Mutex<ProjectRegistry>,
    pub profiles: Mutex<AgentProfileRegistry>,
    pub push_attempts: Arc<Mutex<PushAttemptStore>>,
    pub runtime: Arc<Mutex<RuntimeStore>>,
    pub codex_sessions: Arc<Mutex<HashMap<Uuid, CodexLiveHandle>>>,
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
        let mut runtime = RuntimeStore::open(runtime_database_path()?, &config)?;
        let recovered = runtime.recover_interrupted_deliveries()?;
        if recovered > 0 {
            eprintln!("recovered {recovered} interrupted push deliveries as delivery_unknown");
        }
        let unloaded = runtime.recover_loaded_sessions()?;
        if unloaded > 0 {
            eprintln!("marked {unloaded} previously loaded sessions as not_loaded");
        }
        let mut watchers = ProjectWatchService::new(&config, on_change)?;
        for project in registry.projects() {
            if project.path.join(".aurapilot").is_dir()
                && let Err(error) = watchers.watch_project(project)
            {
                eprintln!(
                    "failed to watch registered project {}: {error}",
                    project.path.display()
                );
            }
        }
        Ok(Self {
            config,
            registry: Mutex::new(registry),
            profiles: Mutex::new(profiles),
            push_attempts: Arc::new(Mutex::new(push_attempts)),
            runtime: Arc::new(Mutex::new(runtime)),
            codex_sessions: Arc::new(Mutex::new(HashMap::new())),
            watchers: Mutex::new(watchers),
        })
    }
}
