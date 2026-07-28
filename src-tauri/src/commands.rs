use crate::state::AppState;
use aurapilot_core::project_registry::RegisteredProject;
use aurapilot_core::project_scanner::{
    ProjectSnapshot, scan_project as scan_one, scan_projects as scan_all,
};
use aurapilot_core::validation::SeverityProfile;
use aurapilot_core::watcher::WatchError;
use aurapilot_core::{
    model::LocatedTask,
    task_store::{
        CreateTaskInput, TransitionTaskInput, UpdateTaskInput, create_task as create_one,
        delete_task as delete_one, transition_task as transition_one, update_task as update_one,
    },
};
use std::path::PathBuf;
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub fn list_projects(state: State<'_, AppState>) -> Result<Vec<RegisteredProject>, String> {
    let registry = state.registry.lock().map_err(|error| error.to_string())?;
    Ok(registry.projects().to_vec())
}

#[tauri::command]
pub fn add_project(path: PathBuf, state: State<'_, AppState>) -> Result<RegisteredProject, String> {
    let project = {
        let mut registry = state.registry.lock().map_err(|error| error.to_string())?;
        registry.add(&path).map_err(|error| error.to_string())?
    };
    let watch_result = state
        .watchers
        .lock()
        .map_err(|error| error.to_string())?
        .watch_project(&project);
    if let Err(error) = watch_result {
        let mut registry = state.registry.lock().map_err(|error| error.to_string())?;
        let _ = registry.remove(project.id);
        return Err(error.to_string());
    }
    Ok(project)
}

#[tauri::command]
pub fn remove_project(id: String, state: State<'_, AppState>) -> Result<RegisteredProject, String> {
    let id = Uuid::parse_str(&id).map_err(|error| error.to_string())?;
    let project = {
        let registry = state.registry.lock().map_err(|error| error.to_string())?;
        registry
            .projects()
            .iter()
            .find(|project| project.id == id)
            .cloned()
            .ok_or_else(|| format!("registered project not found: {id}"))?
    };
    let unwatch_result = state
        .watchers
        .lock()
        .map_err(|error| error.to_string())?
        .unwatch_project(id);
    match unwatch_result {
        Ok(()) | Err(WatchError::NotFound(_)) => {}
        Err(error) => return Err(error.to_string()),
    }
    let remove_result = state
        .registry
        .lock()
        .map_err(|error| error.to_string())?
        .remove(id);
    match remove_result {
        Ok(removed) => Ok(removed),
        Err(error) => {
            let _ = state
                .watchers
                .lock()
                .map_err(|error| error.to_string())?
                .watch_project(&project);
            Err(error.to_string())
        }
    }
}

#[tauri::command]
pub fn scan_projects(state: State<'_, AppState>) -> Result<Vec<ProjectSnapshot>, String> {
    let projects = state
        .registry
        .lock()
        .map_err(|error| error.to_string())?
        .projects()
        .to_vec();
    Ok(scan_all(
        &projects,
        &state.config,
        SeverityProfile::lenient(),
    ))
}

#[tauri::command]
pub fn scan_project(id: String, state: State<'_, AppState>) -> Result<ProjectSnapshot, String> {
    let id = Uuid::parse_str(&id).map_err(|error| error.to_string())?;
    let project = state
        .registry
        .lock()
        .map_err(|error| error.to_string())?
        .projects()
        .iter()
        .find(|project| project.id == id)
        .cloned()
        .ok_or_else(|| format!("registered project not found: {id}"))?;
    Ok(scan_one(
        &project,
        &state.config,
        SeverityProfile::lenient(),
    ))
}

#[tauri::command]
pub fn create_task(
    project_id: String,
    input: CreateTaskInput,
    state: State<'_, AppState>,
) -> Result<LocatedTask, String> {
    let project = registered_project(&project_id, &state)?;
    create_one(&project.path, &state.config, input).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_task(
    project_id: String,
    task_id: String,
    input: UpdateTaskInput,
    state: State<'_, AppState>,
) -> Result<LocatedTask, String> {
    let project = registered_project(&project_id, &state)?;
    update_one(&project.path, &task_id, input).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn transition_task(
    project_id: String,
    task_id: String,
    input: TransitionTaskInput,
    state: State<'_, AppState>,
) -> Result<LocatedTask, String> {
    let project = registered_project(&project_id, &state)?;
    transition_one(&project.path, &task_id, input).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_task(
    project_id: String,
    task_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let project = registered_project(&project_id, &state)?;
    delete_one(&project.path, &task_id)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn registered_project(id: &str, state: &State<'_, AppState>) -> Result<RegisteredProject, String> {
    let id = Uuid::parse_str(id).map_err(|error| error.to_string())?;
    state
        .registry
        .lock()
        .map_err(|error| error.to_string())?
        .projects()
        .iter()
        .find(|project| project.id == id)
        .cloned()
        .ok_or_else(|| format!("registered project not found: {id}"))
}
