fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(
            tauri_build::AppManifest::new().commands(&[
                "create_project",
                "open_project",
                "get_project_snapshot",
                "get_project_summary",
                "is_project_dirty",
                "save_project",
                "import_tracks",
                "preview_coordinate_conversion",
                "scan_photos",
                "read_photo_metadata",
                "calculate_matches",
                "build_write_plan",
                "execute_write_plan",
                "cancel_task",
                "get_task",
                "list_tasks",
                "export_report",
                "get_exiftool_status",
            ]),
        ),
    )
    .expect("failed to generate Tauri build context")
}
