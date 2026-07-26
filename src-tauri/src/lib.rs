mod commands;
mod coordinate;
mod domain;
mod error;
mod exiftool;
mod fs_utils;
mod gpx;
mod matching;
mod photo;
#[cfg(windows)]
mod portable_exiftool;
mod project;
mod report;
mod state;
mod task;
mod write;
#[cfg(windows)]
mod webview2;

pub use domain::*;
pub use error::{AppError, AppResult, ErrorCode};

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let exiftool_resource_dir = exiftool_resource_dir(app);
            app.manage(AppState::new(exiftool_resource_dir));
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

#[cfg(windows)]
fn exiftool_resource_dir(app: &tauri::App<tauri::Wry>) -> AppResult<std::path::PathBuf> {
    app.path()
        .app_local_data_dir()
        .map_err(|error| {
            AppError::new(
                ErrorCode::ExiftoolNotAvailable,
                format!("无法定位本地应用数据目录：{error}"),
                "请确认当前用户可以访问本地应用数据目录后重试。",
                true,
            )
        })
        .and_then(|directory| portable_exiftool::prepare(&directory))
}

#[cfg(not(windows))]
fn exiftool_resource_dir(app: &tauri::App<tauri::Wry>) -> AppResult<std::path::PathBuf> {
    Ok(app.path().resource_dir().unwrap_or_else(|_| {
        std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("__missing_resources__")
    }))
}

#[cfg(windows)]
pub fn portable_smoke() -> i32 {
    let result = std::env::var_os("GEOTAGGER_PORTABLE_DATA_DIR")
        .map(std::path::PathBuf::from)
        .ok_or_else(|| {
            AppError::new(
                ErrorCode::ExiftoolNotAvailable,
                "便携版自检没有设置数据目录。",
                "请由 GitHub Actions 运行便携版自检。",
                false,
            )
        })
        .and_then(|directory| portable_exiftool::smoke_test(&directory));
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("GeoTagger portable smoke failed: {error}");
            1
        }
    }
}

#[cfg(windows)]
pub fn webview2_available() -> bool {
    webview2::ensure_available()
}
