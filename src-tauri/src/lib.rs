mod commands;
mod platform;
mod providers;
mod state;

use commands::{
    add_project, bind_agent_session, create_task, delete_agent_profile, delete_task,
    export_aura_tasks, fork_task_session, get_git_workspace_status, import_aura_tasks,
    initialize_project, interrupt_task_session, list_agent_profiles, list_agent_sessions,
    list_projects, list_push_attempts, preview_aura_import, preview_pointer_prompt, push_task,
    push_task_to_session, remove_project, save_agent_profile, scan_project, scan_projects,
    steer_task_session, test_agent_profile, transition_task, update_task,
};
use commands::{recover_claude_inboxes, recover_codex_inboxes, recover_opencode_inboxes};
use state::AppState;
use tauri::{Emitter, Manager};

pub const PROJECT_CHANGED_EVENT: &str = "aurapilot://project-changed";
pub const PUSH_ATTEMPT_EVENT: &str = "aurapilot://push-attempt";

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
            scan_projects,
            scan_project,
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
            list_agent_sessions,
            bind_agent_session,
            push_task_to_session,
            steer_task_session,
            interrupt_task_session,
            fork_task_session,
            test_agent_profile
        ])
        .run(tauri::generate_context!())
        .expect("error while running AuraPilot");
}
