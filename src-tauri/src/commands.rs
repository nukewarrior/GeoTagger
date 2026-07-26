use crate::coordinate::convert;
use crate::domain::{
    Calibration, ConversionPreviewPoint, ConvertedPreview, CoordinateSystem, ExifToolStatus,
    GeoBounds, GeoPoint, PhotoMetadata, PhotoMetadataStatus, ProjectDirtyEvent, ProjectSnapshot,
    ProjectSummary, SaveResult, TaskAccepted, TaskFailedEvent, TaskFinishedEvent, TaskKind,
    TaskProgressEvent, TaskRecord, TaskWarningEvent, Track, WritePlan,
};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::exiftool::ExifTool;
use crate::gpx::parse_gpx_file;
use crate::matching::calculate_matches as calculate_matches_core;
use crate::photo::{scan_photo_directory, PhotoScanResult, ScanPhotosRequest};
use crate::project::{
    create_snapshot, open_project as open_project_file, project_path_from_directory,
    save_project as save_project_file, summary, validate_snapshot,
};
use crate::report::{export_report as export_report_file, ExportReportRequest, ExportResult};
use crate::state::AppState;
use crate::write::{
    build_write_plan as build_write_plan_core, execute_write_plan as execute_write_plan_core,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectRequest {
    pub name: String,
    #[serde(alias = "directory")]
    pub project_directory: String,
    #[serde(default)]
    pub default_output_directory: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProjectRequest {
    #[serde(default)]
    pub project_path: Option<String>,
    #[serde(default)]
    pub snapshot: Option<ProjectSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportTracksRequest {
    pub paths: Vec<String>,
    pub source_crs: CoordinateSystem,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackImportResult {
    pub tracks: Vec<Track>,
    pub warning_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadPhotoMetadataRequest {
    pub photo_ids: Vec<Uuid>,
    #[serde(default)]
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalculateMatchesRequest {
    #[serde(default)]
    pub track_ids: Option<Vec<Uuid>>,
    #[serde(default)]
    pub photo_ids: Option<Vec<Uuid>>,
    pub calibration: Calibration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewCoordinateConversionRequest {
    pub track_id: Uuid,
    pub source_crs: CoordinateSystem,
    #[serde(default = "default_preview_limit")]
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildWritePlanRequest {
    pub photo_ids: Vec<Uuid>,
    pub output_directory: String,
    #[serde(default)]
    pub options: crate::domain::WriteOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteWritePlanRequest {
    pub write_plan_id: Uuid,
}

#[tauri::command]
pub fn create_project(
    app: AppHandle,
    state: State<'_, AppState>,
    request: CreateProjectRequest,
) -> AppResult<ProjectSummary> {
    let name = request.name.trim();
    if name.is_empty() {
        return Err(AppError::invalid("项目名称不能为空。"));
    }
    require_absolute_path(&request.project_directory, "项目目录")?;
    if let Some(output_directory) = request.default_output_directory.as_deref() {
        require_absolute_path(output_directory, "默认输出目录")?;
    }
    let directory = crate::fs_utils::normalize_absolute(Path::new(&request.project_directory))?;
    fs::create_dir_all(&directory).map_err(|error| {
        AppError::new(
            ErrorCode::WritePermissionDenied,
            format!("无法创建项目目录：{error}"),
            "请选择当前用户有写权限的目录。",
            true,
        )
    })?;
    let project_path = project_path_from_directory(&directory)?;
    if project_path.exists() {
        return Err(AppError::new(
            ErrorCode::OutputConflict,
            "所选目录已经包含 project.json。",
            "请打开现有项目或选择新的项目目录。",
            true,
        ));
    }
    let mut snapshot = create_snapshot(name.to_owned(), request.default_output_directory);
    save_project_file(&project_path, &mut snapshot)?;
    state.replace_project(project_path.clone(), snapshot.clone(), false);
    emit_dirty(&app, false);
    Ok(summary(&project_path, &snapshot))
}

#[tauri::command]
pub fn open_project(
    app: AppHandle,
    state: State<'_, AppState>,
    project_path: String,
) -> AppResult<ProjectSnapshot> {
    require_absolute_path(&project_path, "项目文件")?;
    let (canonical, snapshot) = open_project_file(Path::new(&project_path))?;
    state.replace_project(canonical, snapshot.clone(), false);
    emit_dirty(&app, false);
    Ok(snapshot)
}

#[tauri::command]
pub fn get_project_snapshot(state: State<'_, AppState>) -> AppResult<ProjectSnapshot> {
    state.snapshot()
}

#[tauri::command]
pub fn get_project_summary(state: State<'_, AppState>) -> AppResult<ProjectSummary> {
    let snapshot = state.snapshot()?;
    let path = state.project_path()?;
    Ok(summary(&path, &snapshot))
}

#[tauri::command]
pub fn is_project_dirty(state: State<'_, AppState>) -> bool {
    state.is_dirty()
}

#[tauri::command]
pub fn save_project(
    app: AppHandle,
    state: State<'_, AppState>,
    request: SaveProjectRequest,
) -> AppResult<SaveResult> {
    if let Some(incoming) = request.snapshot {
        validate_snapshot(&incoming)?;
        let (current, token) = state.snapshot_with_token()?;
        if current.project.id != incoming.project.id {
            return Err(AppError::project_invalid("保存快照与当前项目 ID 不一致。"));
        }
        if current.project.updated_at != incoming.project.updated_at {
            return Err(AppError::new(
                ErrorCode::OutputConflict,
                "当前项目已被后台任务更新，拒绝用旧快照覆盖。",
                "请刷新当前项目状态后重新应用设置并保存。",
                true,
            ));
        }
        state.mutate_project_if_current(token, |project| {
            *project = incoming;
            Ok(())
        })?;
    }
    let (mut snapshot, token) = state.snapshot_with_token()?;
    validate_snapshot(&snapshot)?;
    let path = match request.project_path {
        Some(path) => {
            require_absolute_path(&path, "项目文件")?;
            crate::fs_utils::normalize_absolute(Path::new(&path))?
        }
        None => state.project_path()?,
    };
    if path.file_name().and_then(|value| value.to_str()) != Some(crate::project::PROJECT_FILE_NAME)
    {
        return Err(AppError::invalid(
            "项目文件名必须为 project.json，以避免覆盖其他文件。",
        ));
    }
    let result = save_project_file(&path, &mut snapshot)?;
    state.commit_saved_if_current(token, path, snapshot)?;
    emit_dirty(&app, false);
    Ok(result)
}

#[tauri::command]
pub async fn import_tracks(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ImportTracksRequest,
) -> AppResult<TrackImportResult> {
    if request.paths.is_empty() {
        return Err(AppError::invalid("至少选择一个 GPX 文件。"));
    }
    for path in &request.paths {
        require_absolute_path(path, "GPX 文件")?;
    }
    if request.source_crs == CoordinateSystem::Unknown {
        return Err(AppError::new(
            ErrorCode::CrsUnconfirmed,
            "轨迹坐标系尚未确认。",
            "请选择原始坐标系并检查转换预览。",
            true,
        ));
    }
    let token = state.project_token()?;
    let project_root = state
        .project_path()?
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| AppError::project_invalid("项目路径缺少父目录。"))?;
    if state.project_token()? != token {
        return Err(AppError::project_invalid(
            "读取项目目录时项目已发生切换，请重试。",
        ));
    }
    let paths = request.paths;
    let source_crs = request.source_crs;
    let imported = tauri::async_runtime::spawn_blocking(move || {
        let mut tracks = Vec::with_capacity(paths.len());
        for path in paths {
            tracks.push(parse_gpx_file(
                Path::new(&path),
                source_crs,
                Some(&project_root),
            )?);
        }
        Ok::<_, AppError>(tracks)
    })
    .await
    .map_err(|error| AppError::internal(format!("轨迹任务异常终止：{error}")))??;

    let warning_count = imported.iter().map(|track| track.warnings.len()).sum();
    let returned = imported.clone();
    state.mutate_project_if_current(token, |project| {
        let imported_ids: BTreeSet<Uuid> = imported.iter().map(|track| track.id).collect();
        project
            .tracks
            .retain(|track| !imported_ids.contains(&track.id));
        project.tracks.extend(imported);
        project.tracks.sort_by_key(|track| track.id);
        project.project.updated_at = Utc::now();
        Ok(())
    })?;
    emit_dirty(&app, true);
    Ok(TrackImportResult {
        tracks: returned,
        warning_count,
    })
}

#[tauri::command]
pub fn preview_coordinate_conversion(
    state: State<'_, AppState>,
    request: PreviewCoordinateConversionRequest,
) -> AppResult<ConvertedPreview> {
    let snapshot = state.snapshot()?;
    let track = snapshot
        .tracks
        .iter()
        .find(|track| track.id == request.track_id)
        .ok_or_else(|| AppError::invalid("找不到指定轨迹。"))?;
    let raw_points = track
        .segments
        .iter()
        .flat_map(|segment| segment.points.iter())
        .collect::<Vec<_>>();
    if raw_points.is_empty() {
        return Err(AppError::new(
            ErrorCode::TrackParseFailed,
            "轨迹没有可预览的点。",
            "请重新导入有效 GPX。",
            true,
        ));
    }
    let limit = request.limit.clamp(2, 5000).min(raw_points.len());
    let step = (raw_points.len() as f64 / limit as f64).max(1.0);
    let mut points = Vec::with_capacity(limit);
    for sample_index in 0..limit {
        let index = ((sample_index as f64 * step).floor() as usize).min(raw_points.len() - 1);
        let point = raw_points[index];
        let original = GeoPoint {
            lat: point.original.lat,
            lon: point.original.lon,
        };
        points.push(ConversionPreviewPoint {
            original,
            normalized: convert(original, request.source_crs, CoordinateSystem::Wgs84)?,
        });
    }
    let bounds = bounds_for_preview(&points)?;
    Ok(ConvertedPreview {
        track_id: track.id,
        source_crs: request.source_crs,
        sample_count: points.len(),
        total_count: raw_points.len(),
        points,
        bounds,
    })
}

#[tauri::command]
pub fn scan_photos(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ScanPhotosRequest,
) -> AppResult<TaskAccepted> {
    let token = state.project_token()?;
    let (task_id, cancelled) = state.tasks.start(TaskKind::PhotoScan, "scan");
    let task_app = app.clone();
    tauri::async_runtime::spawn(async move {
        let progress_app = task_app.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            scan_photo_directory(&request, &cancelled, |completed, total, message| {
                if let Some(state) = progress_app.try_state::<AppState>() {
                    state
                        .tasks
                        .update(task_id, "scan", completed, total, message);
                }
                let _ = progress_app.emit(
                    "task://progress",
                    TaskProgressEvent {
                        task_id,
                        stage: "scan".to_owned(),
                        completed,
                        total,
                        message: message.to_owned(),
                    },
                );
            })
        })
        .await
        .map_err(|error| AppError::internal(format!("照片扫描任务异常终止：{error}")))
        .and_then(|result| result);
        finish_photo_scan(&task_app, task_id, token, result);
    });
    Ok(TaskAccepted { task_id })
}

fn finish_photo_scan(
    app: &AppHandle,
    task_id: Uuid,
    token: crate::state::ProjectToken,
    result: AppResult<PhotoScanResult>,
) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    match result {
        Ok(result) => {
            let photo_count = result.photos.len();
            let warning_count = result.warnings.len();
            for warning in &result.warnings {
                let _ = app.emit(
                    "task://warning",
                    TaskWarningEvent {
                        task_id,
                        code: "PHOTO_SCAN_WARNING".to_owned(),
                        file: warning.path.clone(),
                        message: warning.message.clone(),
                    },
                );
            }
            let mutation = state.mutate_project_if_current(token, |project| {
                let mut existing_by_path = project
                    .photos
                    .drain(..)
                    .map(|photo| (photo.path.clone(), photo))
                    .collect::<BTreeMap<_, _>>();
                for mut scanned in result.photos {
                    if let Some(existing) = existing_by_path.remove(&scanned.path) {
                        if existing.id == scanned.id {
                            scanned.capture_local = existing.capture_local;
                            scanned.capture_utc = existing.capture_utc;
                            scanned.timezone_source = existing.timezone_source;
                            scanned.existing_gps = existing.existing_gps;
                            scanned.metadata_status = existing.metadata_status;
                            scanned.metadata_error = existing.metadata_error;
                        }
                    }
                    existing_by_path.insert(scanned.path.clone(), scanned);
                }
                project.photos = existing_by_path.into_values().collect();
                project.photos.sort_by_key(|photo| photo.id);
                let photo_ids: BTreeSet<Uuid> =
                    project.photos.iter().map(|photo| photo.id).collect();
                project
                    .matches
                    .retain(|photo_match| photo_ids.contains(&photo_match.photo_id));
                project.project.updated_at = Utc::now();
                Ok(())
            });
            if let Err(error) = mutation {
                state.tasks.fail(task_id, error.clone());
                let _ = app.emit("task://failed", TaskFailedEvent { task_id, error });
                return;
            }
            state.tasks.finish(task_id, "照片扫描完成。");
            let summary = json!({
                "photoCount": photo_count,
                "warningCount": warning_count,
                "rootDirectory": result.root_directory,
            });
            let _ = app.emit("task://finished", TaskFinishedEvent { task_id, summary });
            emit_dirty(app, true);
        }
        Err(error) => {
            state.tasks.fail(task_id, error.clone());
            let _ = app.emit("task://failed", TaskFailedEvent { task_id, error });
        }
    }
}

#[tauri::command]
pub async fn read_photo_metadata(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ReadPhotoMetadataRequest,
) -> AppResult<Vec<PhotoMetadata>> {
    if request.photo_ids.is_empty() {
        return Ok(Vec::new());
    }
    let (snapshot, token) = state.snapshot_with_token()?;
    let timezone = request
        .timezone
        .unwrap_or_else(|| snapshot.settings.photo_timezone.clone());
    let selected_ids: BTreeSet<Uuid> = request.photo_ids.iter().copied().collect();
    let photos = snapshot
        .photos
        .iter()
        .filter(|photo| selected_ids.contains(&photo.id))
        .cloned()
        .collect::<Vec<_>>();
    if photos.len() != selected_ids.len() {
        return Err(AppError::invalid("元数据请求包含未知照片 ID。"));
    }
    let exiftool = state
        .exiftool_resource_dir()
        .and_then(|resource_dir| ExifTool::discover(&resource_dir))?;
    let metadata = tauri::async_runtime::spawn_blocking(move || {
        exiftool.version()?;
        exiftool.read_metadata(&photos, &timezone)
    })
    .await
    .map_err(|error| AppError::internal(format!("元数据任务异常终止：{error}")))??;
    let by_id = metadata
        .iter()
        .map(|item| (item.photo_id, item))
        .collect::<BTreeMap<_, _>>();
    state.mutate_project_if_current(token, |project| {
        for photo in &mut project.photos {
            let Some(item) = by_id.get(&photo.id).copied() else {
                continue;
            };
            photo.capture_local = item
                .sub_sec_date_time_original
                .clone()
                .or(item.date_time_original.clone())
                .or(item.create_date.clone());
            photo.capture_utc = item.capture_utc;
            photo.timezone_source = item.timezone_source;
            photo.existing_gps = item.existing_gps.clone();
            photo.metadata_error = item.error.clone();
            photo.metadata_status = if item.error.is_some() {
                if item.capture_utc.is_none() && photo.capture_local.is_some() {
                    PhotoMetadataStatus::AmbiguousTime
                } else {
                    PhotoMetadataStatus::Failed
                }
            } else {
                PhotoMetadataStatus::Ready
            };
        }
        project.project.updated_at = Utc::now();
        Ok(())
    })?;
    emit_dirty(&app, true);
    Ok(metadata)
}

#[tauri::command]
pub fn calculate_matches(
    app: AppHandle,
    state: State<'_, AppState>,
    request: CalculateMatchesRequest,
) -> AppResult<TaskAccepted> {
    request
        .calibration
        .timezone
        .parse::<chrono_tz::Tz>()
        .map_err(|_| {
            AppError::new(
                ErrorCode::PhotoTimeAmbiguous,
                format!("无效时区：{}", request.calibration.timezone),
                "请选择 IANA 时区，例如 Asia/Shanghai。",
                true,
            )
        })?;
    if !request.calibration.sync_points.is_empty() || request.calibration.drift_model.is_some() {
        return Err(AppError::invalid(
            "MVP-1 仅支持固定时间偏差；同步点和漂移模型将在后续版本启用。",
        ));
    }
    let (snapshot, token) = state.snapshot_with_token()?;
    let (task_id, cancelled) = state.tasks.start(TaskKind::MatchCalculation, "matching");
    let task_app = app.clone();
    tauri::async_runtime::spawn(async move {
        let progress_app = task_app.clone();
        let request_for_result = request.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            calculate_matches_core(
                &snapshot.photos,
                &snapshot.tracks,
                request.photo_ids.as_deref(),
                request.track_ids.as_deref(),
                &request.calibration,
                &cancelled,
                |completed, total, message| {
                    if let Some(state) = progress_app.try_state::<AppState>() {
                        state
                            .tasks
                            .update(task_id, "matching", completed, total, message);
                    }
                    let _ = progress_app.emit(
                        "task://progress",
                        TaskProgressEvent {
                            task_id,
                            stage: "matching".to_owned(),
                            completed,
                            total,
                            message: message.to_owned(),
                        },
                    );
                },
            )
        })
        .await
        .map_err(|error| AppError::internal(format!("匹配任务异常终止：{error}")))
        .and_then(|result| result);
        finish_matching(
            &task_app,
            task_id,
            token,
            request_for_result.calibration,
            result,
        );
    });
    Ok(TaskAccepted { task_id })
}

fn finish_matching(
    app: &AppHandle,
    task_id: Uuid,
    token: crate::state::ProjectToken,
    calibration: Calibration,
    result: AppResult<Vec<crate::domain::PhotoMatch>>,
) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    match result {
        Ok(matches) => {
            let count = matches.len();
            let high = matches
                .iter()
                .filter(|item| {
                    item.quality_status.unwrap_or(item.status)
                        == crate::domain::MatchStatus::MatchedHigh
                })
                .count();
            let changed_ids: BTreeSet<Uuid> = matches.iter().map(|item| item.photo_id).collect();
            let mutation = state.mutate_project_if_current(token, |project| {
                project
                    .matches
                    .retain(|item| !changed_ids.contains(&item.photo_id));
                project.matches.extend(matches);
                project.matches.sort_by_key(|item| item.photo_id);
                project.settings.fixed_offset_ms = calibration.fixed_offset_ms;
                project.settings.photo_timezone = calibration.timezone.clone();
                project.calibration = calibration;
                project.project.updated_at = Utc::now();
                Ok(())
            });
            if let Err(error) = mutation {
                state.tasks.fail(task_id, error.clone());
                let _ = app.emit("task://failed", TaskFailedEvent { task_id, error });
                return;
            }
            state.tasks.finish(task_id, "照片匹配完成。");
            let _ = app.emit(
                "task://finished",
                TaskFinishedEvent {
                    task_id,
                    summary: json!({"matchCount": count, "highConfidenceCount": high}),
                },
            );
            emit_dirty(app, true);
        }
        Err(error) => {
            state.tasks.fail(task_id, error.clone());
            let _ = app.emit("task://failed", TaskFailedEvent { task_id, error });
        }
    }
}

#[tauri::command]
pub fn build_write_plan(
    state: State<'_, AppState>,
    request: BuildWritePlanRequest,
) -> AppResult<WritePlan> {
    let (snapshot, token) = state.snapshot_with_token()?;
    let plan = build_write_plan_core(
        &snapshot.photos,
        &snapshot.matches,
        &request.photo_ids,
        Path::new(&request.output_directory),
        request.options,
    )?;
    state.insert_write_plan_if_current(token, plan.clone())?;
    Ok(plan)
}

#[tauri::command]
pub fn execute_write_plan(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ExecuteWritePlanRequest,
) -> AppResult<TaskAccepted> {
    let (plan, token) = state.write_plan_with_token(request.write_plan_id)?;
    let exiftool = state
        .exiftool_resource_dir()
        .and_then(|resource_dir| ExifTool::discover(&resource_dir))?;
    exiftool.version()?;
    let (task_id, cancelled) = state.tasks.start(TaskKind::WriteExif, "write");
    let task_app = app.clone();
    tauri::async_runtime::spawn(async move {
        let progress_app = task_app.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            execute_write_plan_core(&plan, &exiftool, &cancelled, |completed, total, message| {
                if let Some(state) = progress_app.try_state::<AppState>() {
                    state
                        .tasks
                        .update(task_id, "write", completed, total, message);
                }
                let _ = progress_app.emit(
                    "task://progress",
                    TaskProgressEvent {
                        task_id,
                        stage: "write".to_owned(),
                        completed,
                        total,
                        message: message.to_owned(),
                    },
                );
            })
        })
        .await
        .map_err(|error| AppError::internal(format!("写入任务异常终止：{error}")));
        finish_write(&task_app, task_id, token, result);
    });
    Ok(TaskAccepted { task_id })
}

fn finish_write(
    app: &AppHandle,
    task_id: Uuid,
    token: crate::state::ProjectToken,
    result: AppResult<crate::domain::WriteJob>,
) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    match result {
        Ok(job) => {
            for item in job
                .results
                .iter()
                .filter(|item| item.status == crate::domain::WriteItemStatus::Failed)
            {
                let _ = app.emit(
                    "task://warning",
                    TaskWarningEvent {
                        task_id,
                        code: item
                            .error
                            .as_ref()
                            .map(|error| {
                                serde_json::to_value(error.code)
                                    .ok()
                                    .and_then(|value| value.as_str().map(str::to_owned))
                                    .unwrap_or_else(|| "WRITE_FAILED".to_owned())
                            })
                            .unwrap_or_else(|| "WRITE_FAILED".to_owned()),
                        file: item.output_path.clone(),
                        message: item.message.clone(),
                    },
                );
            }
            let failed = job
                .results
                .iter()
                .filter(|item| item.status == crate::domain::WriteItemStatus::Failed)
                .count();
            let written = job
                .results
                .iter()
                .filter(|item| item.status == crate::domain::WriteItemStatus::WrittenVerified)
                .count();
            let job_status = job.status;
            let mutation = state.mutate_project_if_current(token, |project| {
                project.write_history.push(job);
                project.project.updated_at = Utc::now();
                Ok(())
            });
            if let Err(error) = mutation {
                state.tasks.fail(task_id, error.clone());
                let _ = app.emit("task://failed", TaskFailedEvent { task_id, error });
                return;
            }
            if job_status == crate::domain::WriteJobStatus::Cancelled {
                state.tasks.fail(task_id, AppError::cancelled());
            } else {
                state.tasks.finish(task_id, "写入任务完成。");
            }
            let _ = app.emit(
                "task://finished",
                TaskFinishedEvent {
                    task_id,
                    summary: json!({"writtenCount": written, "failedCount": failed}),
                },
            );
            emit_dirty(app, true);
        }
        Err(error) => {
            state.tasks.fail(task_id, error.clone());
            let _ = app.emit("task://failed", TaskFailedEvent { task_id, error });
        }
    }
}

#[tauri::command]
pub fn cancel_task(state: State<'_, AppState>, task_id: Uuid) -> bool {
    state.tasks.cancel(task_id)
}

#[tauri::command]
pub fn get_task(state: State<'_, AppState>, task_id: Uuid) -> Option<TaskRecord> {
    state.tasks.get(task_id)
}

#[tauri::command]
pub fn list_tasks(state: State<'_, AppState>) -> Vec<TaskRecord> {
    state.tasks.list()
}

#[tauri::command]
pub async fn export_report(
    state: State<'_, AppState>,
    request: ExportReportRequest,
) -> AppResult<ExportResult> {
    let snapshot = state.snapshot()?;
    tauri::async_runtime::spawn_blocking(move || export_report_file(&snapshot, &request))
        .await
        .map_err(|error| AppError::internal(format!("报告任务异常终止：{error}")))?
}

#[tauri::command]
pub fn get_exiftool_status(state: State<'_, AppState>) -> ExifToolStatus {
    match state.exiftool_resource_dir() {
        Ok(resource_dir) => ExifTool::status(&resource_dir),
        Err(error) => ExifTool::unavailable_status(error),
    }
}

fn bounds_for_preview(points: &[ConversionPreviewPoint]) -> AppResult<GeoBounds> {
    let first = points
        .first()
        .ok_or_else(|| AppError::invalid("转换预览没有采样点。"))?;
    let mut bounds = GeoBounds {
        min_lat: first.normalized.lat,
        min_lon: first.normalized.lon,
        max_lat: first.normalized.lat,
        max_lon: first.normalized.lon,
    };
    for point in &points[1..] {
        bounds.min_lat = bounds.min_lat.min(point.normalized.lat);
        bounds.min_lon = bounds.min_lon.min(point.normalized.lon);
        bounds.max_lat = bounds.max_lat.max(point.normalized.lat);
        bounds.max_lon = bounds.max_lon.max(point.normalized.lon);
    }
    Ok(bounds)
}

fn emit_dirty(app: &AppHandle, changed: bool) {
    let _ = app.emit("project://dirty", ProjectDirtyEvent { changed });
}

fn require_absolute_path(value: &str, label: &str) -> AppResult<()> {
    if value.trim().is_empty() || !Path::new(value).is_absolute() {
        return Err(AppError::invalid(format!("{label}必须是非空绝对路径。")));
    }
    Ok(())
}

const fn default_preview_limit() -> usize {
    1000
}
