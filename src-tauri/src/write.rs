use crate::domain::{
    ExistingGps, ExistingGpsPolicy, MatchStatus, Photo, PhotoMatch, WriteItemResult,
    WriteItemStatus, WriteJob, WriteJobStatus, WriteOptions, WritePlan, WritePlanItem,
    WritePlanItemAction,
};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::exiftool::ExifTool;
use crate::fs_utils::{
    atomic_replace, fingerprint, fingerprint_matches, normalize_absolute, resolved_identity,
};
use chrono::Utc;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use uuid::Uuid;

pub fn build_write_plan(
    photos: &[Photo],
    matches: &[PhotoMatch],
    selected_photo_ids: &[Uuid],
    output_directory: &Path,
    options: WriteOptions,
) -> AppResult<WritePlan> {
    if selected_photo_ids.is_empty() {
        return Err(AppError::invalid("至少选择一张照片后才能生成写入计划。"));
    }
    if output_directory.as_os_str().is_empty() || !output_directory.is_absolute() {
        return Err(AppError::invalid("输出目录必须是非空绝对路径。"));
    }
    let output_directory = normalize_absolute(output_directory)?;
    if output_directory.exists() && !output_directory.is_dir() {
        return Err(AppError::invalid("所选输出路径不是目录。"));
    }
    let resolved_output_root = resolved_identity(&output_directory)?;
    let selected: BTreeSet<Uuid> = selected_photo_ids.iter().copied().collect();
    let photo_by_id: BTreeMap<Uuid, &Photo> = photos
        .iter()
        .filter(|photo| selected.contains(&photo.id))
        .map(|photo| (photo.id, photo))
        .collect();
    if photo_by_id.len() != selected.len() {
        return Err(AppError::invalid("写入选择中包含未在当前项目找到的照片。"));
    }
    let match_by_photo: BTreeMap<Uuid, &PhotoMatch> = matches
        .iter()
        .map(|photo_match| (photo_match.photo_id, photo_match))
        .collect();

    let mut proposed_paths = BTreeMap::<PathBuf, usize>::new();
    let mut output_paths = BTreeMap::<Uuid, PathBuf>::new();
    for photo in photo_by_id.values() {
        let relative = if options.preserve_relative_paths {
            safe_relative_path(&photo.relative_path)?
        } else {
            safe_relative_path(&photo.file_name)?
        };
        let output = output_directory.join(relative);
        if !resolved_identity(&output)?.starts_with(&resolved_output_root) {
            return Err(AppError::new(
                ErrorCode::PathOutsideScope,
                "照片输出路径离开了所选输出目录。",
                "请重新扫描照片并重新生成写入计划。",
                true,
            ));
        }
        *proposed_paths.entry(output.clone()).or_default() += 1;
        output_paths.insert(photo.id, output);
    }

    let mut items = Vec::with_capacity(photo_by_id.len());
    for (photo_id, photo) in photo_by_id {
        let output = output_paths
            .get(&photo_id)
            .expect("output path built for every selected photo");
        let mut warnings = Vec::new();
        let photo_match = match_by_photo.get(&photo_id).copied();
        let matched_gps = photo_match.and_then(gps_from_match);
        if let Some(gps) = &matched_gps {
            crate::coordinate::validate_point(crate::domain::GeoPoint {
                lat: gps.lat,
                lon: gps.lon,
            })?;
            if gps.altitude.is_some_and(|value| !value.is_finite()) {
                return Err(AppError::invalid("匹配结果包含无效海拔。"));
            }
        }
        let duplicate_output = proposed_paths.get(output).copied().unwrap_or_default() > 1;
        let same_as_source =
            resolved_identity(Path::new(&photo.path))? == resolved_identity(output)?;

        let action = if duplicate_output {
            warnings.push("多个源文件映射到同一输出路径。".to_owned());
            WritePlanItemAction::Conflict
        } else if same_as_source {
            warnings.push("输出路径与源照片相同，禁止修改原件。".to_owned());
            WritePlanItemAction::Conflict
        } else if output.exists() && !options.overwrite_output {
            warnings.push("输出文件已存在且未启用明确覆盖。".to_owned());
            WritePlanItemAction::Conflict
        } else if matched_gps.is_none() {
            warnings.push("照片没有可写入的匹配坐标。".to_owned());
            WritePlanItemAction::SkipNoMatch
        } else if photo.existing_gps.is_some() {
            match options.existing_gps_policy {
                ExistingGpsPolicy::Skip => WritePlanItemAction::SkipExistingGps,
                ExistingGpsPolicy::Preserve => WritePlanItemAction::PreserveExistingGps,
                ExistingGpsPolicy::Overwrite if is_mvp_writable_format(&photo.extension) => {
                    WritePlanItemAction::WriteGps
                }
                ExistingGpsPolicy::Overwrite => {
                    warnings.push(format!(
                        "MVP-1 不写入 .{}；HEIC 需平台回归，RAW 将使用后续 XMP 流程。",
                        photo.extension
                    ));
                    WritePlanItemAction::SkipUnsupportedFormat
                }
            }
        } else if !is_mvp_writable_format(&photo.extension) {
            warnings.push(format!(
                "MVP-1 不写入 .{}；仅 JPEG/TIFF 支持复制后写入。",
                photo.extension
            ));
            WritePlanItemAction::SkipUnsupportedFormat
        } else {
            WritePlanItemAction::WriteGps
        };

        items.push(WritePlanItem {
            photo_id,
            source_path: photo.path.clone(),
            output_path: output.to_string_lossy().into_owned(),
            source_fingerprint: photo.fingerprint.clone(),
            action,
            old_gps: photo.existing_gps.clone(),
            new_gps: matched_gps,
            warnings,
        });
    }
    items.sort_by_key(|item| item.photo_id);
    let writable_count = items
        .iter()
        .filter(|item| {
            matches!(
                item.action,
                WritePlanItemAction::WriteGps | WritePlanItemAction::PreserveExistingGps
            )
        })
        .count();
    let conflict_count = items
        .iter()
        .filter(|item| item.action == WritePlanItemAction::Conflict)
        .count();
    let skipped_count = items.len() - writable_count - conflict_count;

    Ok(WritePlan {
        id: Uuid::new_v4(),
        created_at: Utc::now(),
        output_directory: output_directory.to_string_lossy().into_owned(),
        options,
        items,
        writable_count,
        skipped_count,
        conflict_count,
    })
}

pub fn execute_write_plan<F>(
    plan: &WritePlan,
    exiftool: &ExifTool,
    cancelled: &AtomicBool,
    mut progress: F,
) -> WriteJob
where
    F: FnMut(u64, u64, &str),
{
    let started_at = Utc::now();
    let job_id = Uuid::new_v4();
    let total = plan.items.len() as u64;
    let mut results = Vec::with_capacity(plan.items.len());
    let mut was_cancelled = false;

    for (index, item) in plan.items.iter().enumerate() {
        if cancelled.load(Ordering::Relaxed) {
            was_cancelled = true;
            for remaining in &plan.items[index..] {
                results.push(WriteItemResult {
                    photo_id: remaining.photo_id,
                    output_path: remaining.output_path.clone(),
                    status: WriteItemStatus::Cancelled,
                    message: "任务取消，未处理此文件。".to_owned(),
                    error: Some(AppError::cancelled()),
                });
            }
            break;
        }

        let result = execute_item(
            item,
            &plan.options,
            Path::new(&plan.output_directory),
            exiftool,
        );
        results.push(result);
        progress(
            index as u64 + 1,
            total,
            &format!(
                "已处理 {}",
                Path::new(&item.source_path)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("照片")
            ),
        );
    }

    let has_failures = results
        .iter()
        .any(|result| result.status == WriteItemStatus::Failed);
    let status = if was_cancelled {
        WriteJobStatus::Cancelled
    } else if has_failures {
        WriteJobStatus::CompletedWithErrors
    } else {
        WriteJobStatus::Completed
    };
    WriteJob {
        id: job_id,
        write_plan_id: plan.id,
        selected_photo_ids: plan.items.iter().map(|item| item.photo_id).collect(),
        output_dir: plan.output_directory.clone(),
        options: plan.options.clone(),
        status,
        started_at,
        finished_at: Some(Utc::now()),
        results,
    }
}

fn execute_item(
    item: &WritePlanItem,
    options: &WriteOptions,
    output_root: &Path,
    exiftool: &ExifTool,
) -> WriteItemResult {
    let skipped_message = match item.action {
        WritePlanItemAction::SkipExistingGps => Some("按策略跳过已有 GPS 的照片。"),
        WritePlanItemAction::SkipNoMatch => Some("照片没有可写入的匹配结果。"),
        WritePlanItemAction::SkipUnsupportedFormat => Some("MVP-1 不写入此照片格式。"),
        WritePlanItemAction::Conflict => Some("写入计划存在输出冲突。"),
        _ => None,
    };
    if let Some(message) = skipped_message {
        return WriteItemResult {
            photo_id: item.photo_id,
            output_path: item.output_path.clone(),
            status: if item.action == WritePlanItemAction::Conflict {
                WriteItemStatus::Failed
            } else {
                WriteItemStatus::Skipped
            },
            message: message.to_owned(),
            error: (item.action == WritePlanItemAction::Conflict).then(|| {
                AppError::new(
                    ErrorCode::OutputConflict,
                    "写入计划中的输出冲突尚未解决。",
                    "返回确认页更改输出目录或覆盖策略。",
                    true,
                )
            }),
        };
    }

    match copy_write_verify(item, options, output_root, exiftool) {
        Ok(message) => WriteItemResult {
            photo_id: item.photo_id,
            output_path: item.output_path.clone(),
            status: WriteItemStatus::WrittenVerified,
            message,
            error: None,
        },
        Err(error) => WriteItemResult {
            photo_id: item.photo_id,
            output_path: item.output_path.clone(),
            status: WriteItemStatus::Failed,
            message: error.message.clone(),
            error: Some(error),
        },
    }
}

fn copy_write_verify(
    item: &WritePlanItem,
    options: &WriteOptions,
    output_root: &Path,
    exiftool: &ExifTool,
) -> AppResult<String> {
    let source = Path::new(&item.source_path);
    let output = Path::new(&item.output_path);
    let resolved_root = resolved_identity(output_root)?;
    let resolved_output = resolved_identity(output)?;
    if !resolved_output.starts_with(&resolved_root) {
        return Err(AppError::new(
            ErrorCode::PathOutsideScope,
            "执行时检测到输出路径已经离开所选输出目录。",
            "请检查输出目录中的符号链接，重新生成写入计划后重试。",
            true,
        ));
    }
    if resolved_identity(source)? == resolved_identity(output)? {
        return Err(AppError::new(
            ErrorCode::OutputConflict,
            "输出路径与源文件相同。",
            "请选择独立输出目录；应用不会修改源照片。",
            true,
        ));
    }
    if !fingerprint_matches(source, &item.source_fingerprint)? {
        return Err(AppError::new(
            ErrorCode::OutputConflict,
            "源照片在生成写入计划后发生变化。",
            "请重新扫描照片并重新生成写入计划。",
            true,
        ));
    }
    let parent = output
        .parent()
        .ok_or_else(|| AppError::invalid("输出文件缺少父目录。"))?;
    fs::create_dir_all(parent).map_err(|error| {
        AppError::new(
            ErrorCode::WritePermissionDenied,
            format!("无法创建输出目录：{error}"),
            "请选择有写权限的独立输出目录。",
            true,
        )
    })?;
    let file_name = output
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("photo.jpg");
    let temporary = parent.join(format!(".geotagger-{}-{file_name}", Uuid::new_v4()));
    let operation = (|| -> AppResult<String> {
        fs::copy(source, &temporary)
            .map_err(|error| AppError::io("复制照片到临时输出失败", error))?;

        if item.action == WritePlanItemAction::WriteGps {
            let gps = item.new_gps.as_ref().ok_or_else(|| {
                AppError::new(
                    ErrorCode::MatchNotFound,
                    "写入项缺少目标 GPS。",
                    "请重新执行匹配并生成写入计划。",
                    true,
                )
            })?;
            exiftool.write_gps(&temporary, gps, options.include_altitude)?;
            verify_written_gps(
                exiftool,
                item.photo_id,
                &temporary,
                gps,
                options.include_altitude,
            )?;
        } else if item.action == WritePlanItemAction::PreserveExistingGps {
            let source_fingerprint = fingerprint(source)?;
            let copied_fingerprint = fingerprint(&temporary)?;
            if source_fingerprint.sha256 != copied_fingerprint.sha256 {
                return Err(AppError::new(
                    ErrorCode::WriteVerifyFailed,
                    "保留 GPS 的输出副本与源文件内容不一致。",
                    "请删除该输出文件并单独重试。",
                    true,
                ));
            }
        }

        atomic_replace(&temporary, output, options.overwrite_output)?;
        Ok(if item.action == WritePlanItemAction::WriteGps {
            "GPS 已写入副本并重新读取验证。".to_owned()
        } else {
            "已复制照片并保留原有 GPS。".to_owned()
        })
    })();
    if operation.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    operation
}

fn verify_written_gps(
    exiftool: &ExifTool,
    photo_id: Uuid,
    path: &Path,
    expected: &ExistingGps,
    include_altitude: bool,
) -> AppResult<()> {
    let metadata = fs::metadata(path).map_err(|error| AppError::io("读取临时输出失败", error))?;
    let photo = Photo {
        id: photo_id,
        path: path.to_string_lossy().into_owned(),
        relative_path: path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("photo")
            .to_owned(),
        file_name: path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("photo")
            .to_owned(),
        extension: path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase(),
        fingerprint: crate::domain::FileFingerprint {
            sha256: String::new(),
            size_bytes: metadata.len(),
            modified_unix_ms: 0,
        },
        capture_local: None,
        capture_utc: None,
        timezone_source: crate::domain::TimezoneSource::Unknown,
        existing_gps: None,
        thumbnail: None,
        metadata_status: crate::domain::PhotoMetadataStatus::Pending,
        metadata_error: None,
    };
    let actual = exiftool
        .read_metadata(&[photo], "UTC")?
        .into_iter()
        .next()
        .and_then(|metadata| metadata.existing_gps)
        .ok_or_else(|| {
            AppError::new(
                ErrorCode::WriteVerifyFailed,
                "写入后未读取到 GPS。",
                "请保留任务日志并单独重试该文件。",
                true,
            )
        })?;

    let coordinate_matches = (actual.lat - expected.lat).abs() <= 0.000_001
        && (actual.lon - expected.lon).abs() <= 0.000_001;
    let altitude_matches = !include_altitude
        || expected.altitude.is_none()
        || match (actual.altitude, expected.altitude) {
            (Some(actual), Some(expected)) => (actual - expected).abs() <= 0.2,
            (None, None) => true,
            _ => false,
        };
    if !coordinate_matches || !altitude_matches {
        return Err(AppError::new(
            ErrorCode::WriteVerifyFailed,
            format!(
                "GPS 校验不一致：计划 ({:.7}, {:.7})，读取 ({:.7}, {:.7})。",
                expected.lat, expected.lon, actual.lat, actual.lon
            ),
            "请保留任务日志并单独重试该文件。",
            true,
        ));
    }
    Ok(())
}

fn gps_from_match(photo_match: &PhotoMatch) -> Option<ExistingGps> {
    let effective_status = photo_match.quality_status.unwrap_or(photo_match.status);
    if !matches!(
        effective_status,
        MatchStatus::MatchedHigh | MatchStatus::MatchedMedium | MatchStatus::MatchedLow
    ) {
        return None;
    }
    Some(ExistingGps {
        lat: photo_match.lat?,
        lon: photo_match.lon?,
        altitude: photo_match.elevation,
    })
}

fn safe_relative_path(value: &str) -> AppResult<PathBuf> {
    let path = Path::new(value);
    let mut safe = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => safe.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(AppError::new(
                    ErrorCode::PathOutsideScope,
                    "照片相对路径试图离开输出目录。",
                    "请重新扫描照片目录并生成写入计划。",
                    true,
                ))
            }
        }
    }
    if safe.as_os_str().is_empty() {
        return Err(AppError::invalid("照片相对路径为空。"));
    }
    Ok(safe)
}

fn is_mvp_writable_format(extension: &str) -> bool {
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "jpg" | "jpeg" | "tif" | "tiff"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_relative_path_escape() {
        assert!(safe_relative_path("../outside.jpg").is_err());
        assert!(safe_relative_path("nested/photo.jpg").is_ok());
    }

    #[test]
    fn plan_never_targets_source_file() {
        let directory = tempfile::tempdir().expect("temp dir");
        let source = directory.path().join("photo.jpg");
        std::fs::write(&source, b"fixture").expect("fixture");
        let fingerprint = crate::fs_utils::fingerprint(&source).expect("fingerprint");
        let photo_id = Uuid::new_v4();
        let photo = Photo {
            id: photo_id,
            path: source.to_string_lossy().into_owned(),
            relative_path: "photo.jpg".to_owned(),
            file_name: "photo.jpg".to_owned(),
            extension: "jpg".to_owned(),
            fingerprint,
            capture_local: None,
            capture_utc: None,
            timezone_source: crate::domain::TimezoneSource::Unknown,
            existing_gps: None,
            thumbnail: None,
            metadata_status: crate::domain::PhotoMetadataStatus::Ready,
            metadata_error: None,
        };
        let matched = PhotoMatch {
            photo_id,
            track_id: Some(Uuid::new_v4()),
            segment_id: Some(Uuid::new_v4()),
            lat: Some(1.0),
            lon: Some(2.0),
            elevation: None,
            method: "test".to_owned(),
            confidence: Some(1.0),
            status: MatchStatus::MatchedHigh,
            quality_status: Some(MatchStatus::MatchedHigh),
            reason: "test".to_owned(),
            existing_gps_conflict: false,
            matched_time_utc: None,
            previous_point_time_utc: None,
            next_point_time_utc: None,
            interval_seconds: Some(0.0),
            estimated_error_meters: Some(1.0),
        };
        let plan = build_write_plan(
            &[photo],
            &[matched],
            &[photo_id],
            directory.path(),
            WriteOptions::default(),
        )
        .expect("plan");
        assert_eq!(plan.items[0].action, WritePlanItemAction::Conflict);
    }

    #[test]
    fn rejects_untrusted_file_name_traversal_without_relative_paths() {
        let source_directory = tempfile::tempdir().expect("source temp dir");
        let output_directory = tempfile::tempdir().expect("output temp dir");
        let source = source_directory.path().join("photo.jpg");
        std::fs::write(&source, b"fixture").expect("fixture");
        let fingerprint = crate::fs_utils::fingerprint(&source).expect("fingerprint");
        let photo_id = Uuid::new_v4();
        let photo = Photo {
            id: photo_id,
            path: source.to_string_lossy().into_owned(),
            relative_path: "photo.jpg".to_owned(),
            file_name: "../escape.jpg".to_owned(),
            extension: "jpg".to_owned(),
            fingerprint,
            capture_local: None,
            capture_utc: None,
            timezone_source: crate::domain::TimezoneSource::Unknown,
            existing_gps: None,
            thumbnail: None,
            metadata_status: crate::domain::PhotoMetadataStatus::Ready,
            metadata_error: None,
        };
        let matched = matched_fixture(photo_id);
        let options = WriteOptions {
            preserve_relative_paths: false,
            ..WriteOptions::default()
        };
        let result = build_write_plan(
            &[photo],
            &[matched],
            &[photo_id],
            output_directory.path(),
            options,
        );
        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape_from_output_root() {
        use std::os::unix::fs::symlink;

        let source_directory = tempfile::tempdir().expect("source temp dir");
        let output_directory = tempfile::tempdir().expect("output temp dir");
        let outside_directory = tempfile::tempdir().expect("outside temp dir");
        symlink(
            outside_directory.path(),
            output_directory.path().join("escape"),
        )
        .expect("symlink");
        let source = source_directory.path().join("photo.jpg");
        std::fs::write(&source, b"fixture").expect("fixture");
        let fingerprint = crate::fs_utils::fingerprint(&source).expect("fingerprint");
        let photo_id = Uuid::new_v4();
        let photo = Photo {
            id: photo_id,
            path: source.to_string_lossy().into_owned(),
            relative_path: "escape/photo.jpg".to_owned(),
            file_name: "photo.jpg".to_owned(),
            extension: "jpg".to_owned(),
            fingerprint,
            capture_local: None,
            capture_utc: None,
            timezone_source: crate::domain::TimezoneSource::Unknown,
            existing_gps: None,
            thumbnail: None,
            metadata_status: crate::domain::PhotoMetadataStatus::Ready,
            metadata_error: None,
        };
        let matched = matched_fixture(photo_id);
        let result = build_write_plan(
            &[photo],
            &[matched],
            &[photo_id],
            output_directory.path(),
            WriteOptions::default(),
        );
        assert!(result.is_err());
    }

    fn matched_fixture(photo_id: Uuid) -> PhotoMatch {
        PhotoMatch {
            photo_id,
            track_id: Some(Uuid::new_v4()),
            segment_id: Some(Uuid::new_v4()),
            lat: Some(1.0),
            lon: Some(2.0),
            elevation: None,
            method: "test".to_owned(),
            confidence: Some(1.0),
            status: MatchStatus::MatchedHigh,
            quality_status: Some(MatchStatus::MatchedHigh),
            reason: "test".to_owned(),
            existing_gps_conflict: false,
            matched_time_utc: None,
            previous_point_time_utc: None,
            next_point_time_utc: None,
            interval_seconds: Some(0.0),
            estimated_error_meters: Some(1.0),
        }
    }
}
