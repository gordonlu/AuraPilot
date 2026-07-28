mod commands;
mod platform;
mod state;

use commands::{
    add_project, create_task, delete_agent_profile, delete_task, initialize_project,
    list_agent_profiles, list_projects, list_push_attempts, preview_pointer_prompt, push_task,
    remove_project, save_agent_profile, scan_project, scan_projects, test_agent_profile,
    transition_task, update_task,
};
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
            list_agent_profiles,
            save_agent_profile,
            delete_agent_profile,
            preview_pointer_prompt,
            push_task,
            list_push_attempts,
            test_agent_profile
        ])
        .run(tauri::generate_context!())
        .expect("error while running AuraPilot");
}
