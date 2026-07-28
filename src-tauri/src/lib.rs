mod commands;
mod state;

use commands::{
    add_project, create_task, delete_task, list_projects, remove_project, scan_project,
    scan_projects, transition_task, update_task,
};
use state::AppState;
use tauri::{Emitter, Manager};

pub const PROJECT_CHANGED_EVENT: &str = "aurapilot://project-changed";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();
            let state = AppState::load(move |change| {
                let _ = handle.emit(PROJECT_CHANGED_EVENT, change);
            })?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_projects,
            add_project,
            remove_project,
            scan_projects,
            scan_project,
            create_task,
            update_task,
            transition_task,
            delete_task
        ])
        .run(tauri::generate_context!())
        .expect("error while running AuraPilot");
}
