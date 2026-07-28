use crate::state::AppState;
use crate::{PUSH_ATTEMPT_EVENT, platform};
use aurapilot_core::agent_profile::{
    AgentLaunchProfile, LaunchMode, PromptTransport, is_builtin_profile,
};
use aurapilot_core::pointer_prompt::{PointerPrompt, build_pointer_prompt};
use aurapilot_core::project_registry::RegisteredProject;
use aurapilot_core::project_scanner::{
    ProjectSnapshot, scan_project as scan_one, scan_projects as scan_all,
};
use aurapilot_core::push_attempt::{PushAttempt, PushAttemptStatus, PushDelivery};
use aurapilot_core::validation::SeverityProfile;
use aurapilot_core::watcher::WatchError;
use aurapilot_core::{
    model::LocatedTask,
    task_store::{
        CreateTaskInput, TransitionTaskInput, UpdateTaskInput, create_task as create_one,
        delete_task as delete_one, transition_task as transition_one, update_task as update_one,
    },
};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State};
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

#[derive(Clone, Debug, Serialize)]
pub struct AgentProfileEntry {
    pub profile: AgentLaunchProfile,
    pub built_in: bool,
    pub availability: platform::ExecutableAvailability,
}

#[derive(Clone, Debug, Serialize)]
pub struct PushOutcome {
    pub attempt: PushAttempt,
    pub pointer_prompt: PointerPrompt,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProfileTestOutcome {
    pub profile_id: String,
    pub process_id: Option<u32>,
    pub copied_to_clipboard: bool,
    pub message: String,
}

#[tauri::command]
pub fn list_agent_profiles(state: State<'_, AppState>) -> Result<Vec<AgentProfileEntry>, String> {
    let profiles = state.profiles.lock().map_err(|error| error.to_string())?;
    Ok(profiles
        .all_profiles()
        .into_iter()
        .map(|profile| AgentProfileEntry {
            built_in: is_builtin_profile(&profile.id),
            availability: profile_availability(&profile),
            profile,
        })
        .collect())
}

#[tauri::command]
pub fn save_agent_profile(
    profile: AgentLaunchProfile,
    state: State<'_, AppState>,
) -> Result<AgentLaunchProfile, String> {
    state
        .profiles
        .lock()
        .map_err(|error| error.to_string())?
        .save(profile)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_agent_profile(
    id: String,
    state: State<'_, AppState>,
) -> Result<AgentLaunchProfile, String> {
    let removed = state
        .profiles
        .lock()
        .map_err(|error| error.to_string())?
        .delete(&id)
        .map_err(|error| error.to_string())?;
    let project_ids = state
        .registry
        .lock()
        .map_err(|error| error.to_string())?
        .projects()
        .iter()
        .filter(|project| project.last_profile_id.as_deref() == Some(&id))
        .map(|project| project.id)
        .collect::<Vec<_>>();
    let mut registry = state.registry.lock().map_err(|error| error.to_string())?;
    for project_id in project_ids {
        registry
            .set_last_profile(project_id, None)
            .map_err(|error| error.to_string())?;
    }
    Ok(removed)
}

#[tauri::command]
pub fn preview_pointer_prompt(
    project_id: String,
    task_id: String,
    state: State<'_, AppState>,
) -> Result<PointerPrompt, String> {
    let project = registered_project(&project_id, &state)?;
    let task = find_task(&project, &task_id, &state)?;
    build_pointer_prompt(&project.path, &task).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_push_attempts(state: State<'_, AppState>) -> Result<Vec<PushAttempt>, String> {
    Ok(state
        .push_attempts
        .lock()
        .map_err(|error| error.to_string())?
        .attempts()
        .to_vec())
}

#[tauri::command]
pub fn push_task(
    project_id: String,
    task_id: String,
    profile_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<PushOutcome, String> {
    let project = registered_project(&project_id, &state)?;
    let task = find_task(&project, &task_id, &state)?;
    let pointer_prompt =
        build_pointer_prompt(&project.path, &task).map_err(|error| error.to_string())?;
    let profile = state
        .profiles
        .lock()
        .map_err(|error| error.to_string())?
        .find(&profile_id)
        .ok_or_else(|| format!("agent profile not found: {profile_id}"))?;
    let prepared = profile
        .prepare(&pointer_prompt, &state.config)
        .map_err(|error| error.to_string())?;
    state
        .registry
        .lock()
        .map_err(|error| error.to_string())?
        .set_last_profile(project.id, Some(profile_id.clone()))
        .map_err(|error| error.to_string())?;
    let attempt = state
        .push_attempts
        .lock()
        .map_err(|error| error.to_string())?
        .requested(project.id, &task_id, &profile_id)
        .map_err(|error| error.to_string())?;
    let _ = app.emit(PUSH_ATTEMPT_EVENT, &attempt);

    if prepared.launch_mode == LaunchMode::ClipboardOnly {
        return finish_clipboard_push(&state, &app, attempt, pointer_prompt);
    }

    match platform::launch(&prepared) {
        Ok(mut child) => {
            let process_id = child.id();
            let started = update_attempt(
                &state,
                attempt.id,
                PushAttemptStatus::Started,
                Some(process_id),
                None,
                PushDelivery::Process,
            )?;
            let _ = app.emit(PUSH_ATTEMPT_EVENT, &started);
            let attempts = state.push_attempts.clone();
            let app_handle = app.clone();
            std::thread::spawn(move || {
                let wait_result = child.wait();
                let error = wait_result.err().map(|error| error.to_string());
                if let Ok(mut store) = attempts.lock()
                    && let Ok(exited) = store.update(
                        started.id,
                        PushAttemptStatus::Exited,
                        Some(process_id),
                        error,
                        PushDelivery::Process,
                    )
                {
                    let _ = app_handle.emit(PUSH_ATTEMPT_EVENT, exited);
                }
            });
            Ok(PushOutcome {
                attempt: started,
                pointer_prompt,
                message: format!("{} 已启动", profile.display_name),
            })
        }
        Err(launch_error) => {
            let launch_message = launch_error.to_string();
            let (delivery, message) = match platform::copy_text(&prepared.prompt) {
                Ok(()) => (
                    PushDelivery::ClipboardFallback,
                    format!("启动失败，Pointer Prompt 已复制：{launch_message}"),
                ),
                Err(copy_error) => (
                    PushDelivery::Process,
                    format!("启动失败：{launch_message}；剪贴板兜底也失败：{copy_error}"),
                ),
            };
            let failed = update_attempt(
                &state,
                attempt.id,
                PushAttemptStatus::FailedToStart,
                None,
                Some(launch_message),
                delivery,
            )?;
            let _ = app.emit(PUSH_ATTEMPT_EVENT, &failed);
            Ok(PushOutcome {
                attempt: failed,
                pointer_prompt,
                message,
            })
        }
    }
}

#[tauri::command]
pub fn test_agent_profile(
    project_id: String,
    profile_id: String,
    state: State<'_, AppState>,
) -> Result<ProfileTestOutcome, String> {
    let project = registered_project(&project_id, &state)?;
    let profile = state
        .profiles
        .lock()
        .map_err(|error| error.to_string())?
        .find(&profile_id)
        .ok_or_else(|| format!("agent profile not found: {profile_id}"))?;
    let prompt = PointerPrompt {
        task_id: "PROFILE-TEST".into(),
        protocol_file: ".aurapilot/AGENTS.md".into(),
        task_file: ".aurapilot/AGENTS.md".into(),
        repository: project.path,
        text: "这是 AuraPilot Agent Profile 只读连接测试。请仅确认已在当前仓库启动，不要修改任何文件、任务状态或 Git 历史。".into(),
    };
    let prepared = profile
        .prepare(&prompt, &state.config)
        .map_err(|error| error.to_string())?;
    if prepared.launch_mode == LaunchMode::ClipboardOnly {
        platform::copy_text(&prepared.prompt).map_err(|error| error.to_string())?;
        return Ok(ProfileTestOutcome {
            profile_id,
            process_id: None,
            copied_to_clipboard: true,
            message: "只读测试 Prompt 已复制".into(),
        });
    }
    let child = platform::launch(&prepared).map_err(|error| error.to_string())?;
    Ok(ProfileTestOutcome {
        profile_id,
        process_id: Some(child.id()),
        copied_to_clipboard: prepared.prompt_transport == PromptTransport::Clipboard,
        message: "只读测试已启动".into(),
    })
}

fn finish_clipboard_push(
    state: &State<'_, AppState>,
    app: &AppHandle,
    attempt: PushAttempt,
    pointer_prompt: PointerPrompt,
) -> Result<PushOutcome, String> {
    match platform::copy_text(&pointer_prompt.text) {
        Ok(()) => {
            let started = update_attempt(
                state,
                attempt.id,
                PushAttemptStatus::Started,
                None,
                None,
                PushDelivery::Clipboard,
            )?;
            let _ = app.emit(PUSH_ATTEMPT_EVENT, &started);
            Ok(PushOutcome {
                attempt: started,
                pointer_prompt,
                message: "Pointer Prompt 已复制到剪贴板".into(),
            })
        }
        Err(error) => {
            let message = error.to_string();
            let failed = update_attempt(
                state,
                attempt.id,
                PushAttemptStatus::FailedToStart,
                None,
                Some(message.clone()),
                PushDelivery::Clipboard,
            )?;
            let _ = app.emit(PUSH_ATTEMPT_EVENT, &failed);
            Ok(PushOutcome {
                attempt: failed,
                pointer_prompt,
                message: format!("复制 Pointer Prompt 失败：{message}"),
            })
        }
    }
}

fn update_attempt(
    state: &State<'_, AppState>,
    id: Uuid,
    status: PushAttemptStatus,
    process_id: Option<u32>,
    error: Option<String>,
    delivery: PushDelivery,
) -> Result<PushAttempt, String> {
    state
        .push_attempts
        .lock()
        .map_err(|error| error.to_string())?
        .update(id, status, process_id, error, delivery)
        .map_err(|error| error.to_string())
}

fn profile_availability(profile: &AgentLaunchProfile) -> platform::ExecutableAvailability {
    if profile.launch_mode == LaunchMode::ClipboardOnly {
        return platform::ExecutableAvailability {
            available: true,
            resolved_path: None,
            detail: "clipboard fallback".into(),
        };
    }
    std::iter::once(profile.executable.as_str())
        .chain(profile.detect_commands.iter().map(String::as_str))
        .map(platform::detect_command)
        .find(|availability| availability.available)
        .unwrap_or_else(|| platform::detect_command(&profile.executable))
}

fn find_task(
    project: &RegisteredProject,
    task_id: &str,
    state: &State<'_, AppState>,
) -> Result<LocatedTask, String> {
    let matching = scan_one(project, &state.config, SeverityProfile::lenient())
        .tasks
        .into_iter()
        .filter(|task| task.document.id.as_deref() == Some(task_id))
        .collect::<Vec<_>>();
    match matching.as_slice() {
        [task] => Ok(task.clone()),
        [] => Err(format!("task not found: {task_id}")),
        _ => Err(format!("task id exists more than once: {task_id}")),
    }
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
