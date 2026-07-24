use crate::domain::{
    Calibration, ProjectInfo, ProjectSettings, ProjectSnapshot, ProjectSummary, SaveResult,
    PROJECT_SCHEMA_VERSION,
};
use crate::error::{AppError, AppResult};
use crate::fs_utils::{canonical_file, normalize_absolute, write_atomic};
use chrono::Utc;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const PROJECT_FILE_NAME: &str = "project.json";

pub fn create_snapshot(name: String, default_output_directory: Option<String>) -> ProjectSnapshot {
    let now = Utc::now();
    let settings = ProjectSettings {
        default_output_directory,
        ..ProjectSettings::default()
    };
    ProjectSnapshot {
        schema_version: PROJECT_SCHEMA_VERSION,
        project: ProjectInfo {
            id: Uuid::new_v4(),
            name,
            created_at: now,
            updated_at: now,
        },
        calibration: Calibration {
            timezone: settings.photo_timezone.clone(),
            fixed_offset_ms: settings.fixed_offset_ms,
            ..Calibration::default()
        },
        settings,
        tracks: Vec::new(),
        photos: Vec::new(),
        matches: Vec::new(),
        write_history: Vec::new(),
    }
}

pub fn project_path_from_directory(directory: &Path) -> AppResult<PathBuf> {
    Ok(normalize_absolute(directory)?.join(PROJECT_FILE_NAME))
}

pub fn save_project(path: &Path, snapshot: &mut ProjectSnapshot) -> AppResult<SaveResult> {
    snapshot.schema_version = PROJECT_SCHEMA_VERSION;
    snapshot.project.updated_at = Utc::now();
    let bytes = serde_json::to_vec_pretty(snapshot)
        .map_err(|error| AppError::project_invalid(format!("项目序列化失败：{error}")))?;
    write_atomic(path, &bytes)?;
    let normalized = normalize_absolute(path)?;
    Ok(SaveResult {
        project_path: normalized.to_string_lossy().into_owned(),
        saved_at: snapshot.project.updated_at,
        schema_version: snapshot.schema_version,
    })
}

pub fn open_project(path: &Path) -> AppResult<(PathBuf, ProjectSnapshot)> {
    let canonical = canonical_file(path)?;
    let bytes =
        fs::read(&canonical).map_err(|error| AppError::io("读取项目文件失败", error))?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| AppError::project_invalid(format!("项目 JSON 损坏：{error}")))?;
    let migrated = migrate_to_current(value)?;
    let snapshot: ProjectSnapshot = serde_json::from_value(migrated)
        .map_err(|error| AppError::project_invalid(format!("项目字段不完整：{error}")))?;
    validate_snapshot(&snapshot)?;
    Ok((canonical, snapshot))
}

pub fn validate_snapshot(snapshot: &ProjectSnapshot) -> AppResult<()> {
    if snapshot.schema_version != PROJECT_SCHEMA_VERSION {
        return Err(AppError::project_invalid(format!(
            "不支持的项目版本：{}",
            snapshot.schema_version
        )));
    }
    if snapshot.project.name.trim().is_empty() {
        return Err(AppError::project_invalid("项目名称不能为空。"));
    }
    Ok(())
}

fn migrate_to_current(mut value: Value) -> AppResult<Value> {
    let version = value
        .get("schemaVersion")
        .or_else(|| value.get("schema_version"))
        .and_then(Value::as_u64)
        .ok_or_else(|| AppError::project_invalid("项目缺少 schemaVersion。"))?;
    if version > PROJECT_SCHEMA_VERSION as u64 {
        return Err(AppError::project_invalid(format!(
            "项目版本 {version} 高于当前支持版本 {PROJECT_SCHEMA_VERSION}。"
        )));
    }

    // Schema v1 is the first public schema. The loop is intentionally kept as
    // the single migration entry point so future versions cannot bypass
    // ordered migration.
    while version_of(&value)? < PROJECT_SCHEMA_VERSION as u64 {
        value = match version_of(&value)? {
            unsupported => {
                return Err(AppError::project_invalid(format!(
                    "无法从项目版本 {unsupported} 迁移。"
                )))
            }
        };
    }
    Ok(value)
}

fn version_of(value: &Value) -> AppResult<u64> {
    value
        .get("schemaVersion")
        .or_else(|| value.get("schema_version"))
        .and_then(Value::as_u64)
        .ok_or_else(|| AppError::project_invalid("项目缺少 schemaVersion。"))
}

pub fn summary(path: &Path, snapshot: &ProjectSnapshot) -> ProjectSummary {
    ProjectSummary {
        project_path: path.to_string_lossy().into_owned(),
        project: snapshot.project.clone(),
        track_count: snapshot.tracks.len(),
        photo_count: snapshot.photos.len(),
        match_count: snapshot.matches.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_v1_round_trip() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join(PROJECT_FILE_NAME);
        let mut snapshot = create_snapshot("Test".to_owned(), None);
        save_project(&path, &mut snapshot).expect("save");
        let (_, reopened) = open_project(&path).expect("open");
        assert_eq!(reopened.schema_version, PROJECT_SCHEMA_VERSION);
        assert_eq!(reopened.project.id, snapshot.project.id);
    }

    #[test]
    fn rejects_future_schema() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join(PROJECT_FILE_NAME);
        std::fs::write(
            &path,
            br#"{"schemaVersion":999,"project":{"name":"future"}}"#,
        )
        .expect("fixture");
        assert!(open_project(&path).is_err());
    }
}
