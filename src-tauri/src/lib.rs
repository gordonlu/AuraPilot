mod commands;
mod platform;
mod providers;
mod state;

use commands::{
    add_project, apply_task_repair, bind_agent_session, create_task, delete_agent_profile,
    delete_task, export_aura_tasks, fork_task_session, get_git_workspace_status, import_aura_tasks,
    initialize_project, interrupt_task_session, list_agent_profiles, list_agent_sessions,
    list_approval_requests, list_execution_events, list_pending_items, list_projects,
    list_push_attempts, open_project_folder, preview_aura_import, preview_pointer_prompt,
    preview_task_repairs, push_task, push_task_to_session, remove_project,
    respond_approval_request, save_agent_profile, scan_project, scan_projects, steer_task_session,
    test_agent_profile, transition_task, update_agent_session, update_task,
};
use commands::{recover_claude_inboxes, recover_codex_inboxes, recover_opencode_inboxes};
use state::AppState;
use tauri::{Emitter, Manager};

pub const PROJECT_CHANGED_EVENT: &str = "aurapilot://project-changed";
pub const PUSH_ATTEMPT_EVENT: &str = "aurapilot://push-attempt";
pub const EXECUTION_EVENT: &str = "aurapilot://execution-event";
pub const APPROVAL_EVENT: &str = "aurapilot://approval-request";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let state = AppState::load(move |change| {
                if let Err(error) = handle.emit(PROJECT_CHANGED_EVENT, change) {
                    eprintln!("failed to emit project change: {error}");
                }
            })?;
            app.manage(state);
            let recovered =
                recover_codex_inboxes(app.handle().clone(), app.state::<AppState>().inner())
                    .map_err(std::io::Error::other)?;
            if recovered > 0 {
                eprintln!("started recovery for {recovered} Codex Session inboxes");
            }
            let recovered =
                recover_claude_inboxes(app.handle().clone(), app.state::<AppState>().inner())
                    .map_err(std::io::Error::other)?;
            if recovered > 0 {
                eprintln!("started recovery for {recovered} Claude Session inboxes");
            }
            let recovered =
                recover_opencode_inboxes(app.handle().clone(), app.state::<AppState>().inner())
                    .map_err(std::io::Error::other)?;
            if recovered > 0 {
                eprintln!("started recovery for {recovered} OpenCode Session inboxes");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_projects,
            add_project,
            initialize_project,
            remove_project,
            open_project_folder,
            scan_projects,
            list_pending_items,
            scan_project,
            preview_task_repairs,
            apply_task_repair,
            create_task,
            update_task,
            transition_task,
            delete_task,
            get_git_workspace_status,
            export_aura_tasks,
            preview_aura_import,
            import_aura_tasks,
            list_agent_profiles,
            save_agent_profile,
            delete_agent_profile,
            preview_pointer_prompt,
            push_task,
            list_push_attempts,
            list_execution_events,
            list_approval_requests,
            respond_approval_request,
            list_agent_sessions,
            bind_agent_session,
            update_agent_session,
            push_task_to_session,
            steer_task_session,
            interrupt_task_session,
            fork_task_session,
            test_agent_profile
        ])
        .run(tauri::generate_context!())
        .expect("error while running AuraPilot");
}
