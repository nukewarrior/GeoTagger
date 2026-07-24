mod commands;
mod coordinate;
mod domain;
mod error;
mod exiftool;
mod fs_utils;
mod gpx;
mod matching;
mod photo;
mod project;
mod report;
mod state;
mod task;
mod write;

pub use domain::*;
pub use error::{AppError, AppResult, ErrorCode};

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let resource_dir = app
                .path()
                .resource_dir()
                .unwrap_or_else(|_| {
                    std::env::current_exe()
                        .ok()
                        .and_then(|path| path.parent().map(std::path::Path::to_path_buf))
                        .unwrap_or_else(|| std::path::PathBuf::from("."))
                        .join("__missing_resources__")
                });
            app.manage(AppState::new(resource_dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_project,
            commands::open_project,
            commands::get_project_snapshot,
            commands::get_project_summary,
            commands::is_project_dirty,
            commands::save_project,
            commands::import_tracks,
            commands::preview_coordinate_conversion,
            commands::scan_photos,
            commands::read_photo_metadata,
            commands::calculate_matches,
            commands::build_write_plan,
            commands::execute_write_plan,
            commands::cancel_task,
            commands::get_task,
            commands::list_tasks,
            commands::export_report,
            commands::get_exiftool_status,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run GeoTagger");
}
