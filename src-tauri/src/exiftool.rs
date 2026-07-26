use crate::domain::{ExifToolStatus, ExistingGps, Photo, PhotoMetadata, TimezoneSource};
use crate::error::{AppError, AppResult, ErrorCode};
use chrono::{DateTime, LocalResult, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use uuid::Uuid;

const METADATA_BATCH_SIZE: usize = if cfg!(windows) { 16 } else { 128 };

#[derive(Debug, Clone)]
pub struct ExifTool {
    executable: PathBuf,
}

impl ExifTool {
    pub fn discover(resource_dir: &Path) -> AppResult<Self> {
        let executable_name = if cfg!(windows) {
            "exiftool.exe"
        } else {
            "exiftool"
        };
        let mut candidates = vec![
            resource_dir
                .join("resources")
                .join("exiftool")
                .join(executable_name),
            resource_dir.join("exiftool").join(executable_name),
            resource_dir.join(executable_name),
        ];
        if let Ok(current_executable) = std::env::current_exe() {
            if let Some(directory) = current_executable.parent() {
                candidates.push(
                    directory
                        .join("resources")
                        .join("exiftool")
                        .join(executable_name),
                );
                candidates.push(directory.join("exiftool").join(executable_name));
            }
        }

        for candidate in candidates {
            if candidate.is_file() {
                let canonical = candidate.canonicalize().map_err(|error| {
                    AppError::new(
                        ErrorCode::ExiftoolNotAvailable,
                        format!("无法解析 ExifTool 资源路径：{error}"),
                        "请重新安装由官方发布流程生成的应用包。",
                        true,
                    )
                })?;
                return Ok(Self {
                    executable: canonical,
                });
            }
        }
        Err(missing_exiftool())
    }

    pub fn status(resource_dir: &Path) -> ExifToolStatus {
        match Self::discover(resource_dir).and_then(|service| {
            service
                .version()
                .map(|version| (service.executable, version))
        }) {
            Ok((executable, version)) => ExifToolStatus {
                available: true,
                version: Some(version),
                executable_path: Some(executable.to_string_lossy().into_owned()),
                error: None,
            },
            Err(error) => ExifToolStatus {
                available: false,
                version: None,
                executable_path: None,
                error: Some(error),
            },
        }
    }

    pub fn version(&self) -> AppResult<String> {
        let output = Command::new(&self.executable)
            .arg("-ver")
            .stdin(Stdio::null())
            .output()
            .map_err(unavailable_to_start)?;
        if !output.status.success() {
            return Err(AppError::new(
                ErrorCode::ExiftoolNotAvailable,
                format!("ExifTool 版本检查失败：{}", redacted_stderr(&output.stderr)),
                "请重新安装由官方发布流程生成的应用包。",
                true,
            ));
        }
        let version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if version.is_empty() {
            return Err(AppError::new(
                ErrorCode::ExiftoolNotAvailable,
                "ExifTool 没有返回版本号。",
                "请重新安装由官方发布流程生成的应用包。",
                true,
            ));
        }
        Ok(version)
    }

    pub fn read_metadata(&self, photos: &[Photo], timezone: &str) -> AppResult<Vec<PhotoMetadata>> {
        let parsed_timezone = timezone.parse::<Tz>().map_err(|_| {
            AppError::new(
                ErrorCode::PhotoTimeAmbiguous,
                format!("无效时区：{timezone}"),
                "请选择 IANA 时区，例如 Asia/Shanghai。",
                true,
            )
        })?;
        let mut metadata = Vec::with_capacity(photos.len());
        for batch in photos.chunks(METADATA_BATCH_SIZE) {
            metadata.extend(self.read_metadata_batch(batch, parsed_timezone)?);
        }
        Ok(metadata)
    }

    fn read_metadata_batch(&self, photos: &[Photo], timezone: Tz) -> AppResult<Vec<PhotoMetadata>> {
        let mut command = Command::new(&self.executable);
        command
            .arg("-json")
            .arg("-n")
            .arg("-charset")
            .arg("filename=UTF8")
            .arg("-DateTimeOriginal")
            .arg("-SubSecDateTimeOriginal")
            .arg("-OffsetTimeOriginal")
            .arg("-CreateDate")
            .arg("-GPSLatitude")
            .arg("-GPSLongitude")
            .arg("-GPSAltitude")
            .arg("-GPSAltitudeRef");
        for photo in photos {
            command.arg(&photo.path);
        }
        let output = command
            .stdin(Stdio::null())
            .output()
            .map_err(unavailable_to_start)?;
        if output.stdout.is_empty() {
            return Err(AppError::new(
                ErrorCode::PhotoMetadataFailed,
                format!("ExifTool 未返回 JSON：{}", redacted_stderr(&output.stderr)),
                "请确认照片格式受支持且文件可读。",
                true,
            ));
        }
        let values: Vec<Value> = serde_json::from_slice(&output.stdout).map_err(|error| {
            AppError::new(
                ErrorCode::PhotoMetadataFailed,
                format!("ExifTool JSON 解析失败：{error}"),
                "请确认应用内置的 ExifTool 版本与当前平台匹配。",
                true,
            )
        })?;

        let mut photo_by_path = BTreeMap::new();
        for photo in photos {
            photo_by_path.insert(normalize_path_key(&photo.path), photo);
        }
        let mut results = Vec::with_capacity(photos.len());
        let mut seen = BTreeMap::<Uuid, ()>::new();
        for (index, value) in values.iter().enumerate() {
            let object = value.as_object().ok_or_else(|| {
                AppError::new(
                    ErrorCode::PhotoMetadataFailed,
                    "ExifTool JSON 条目不是对象。",
                    "请重新读取元数据；若持续失败请导出诊断信息。",
                    true,
                )
            })?;
            let source_file = string_value(object, "SourceFile").unwrap_or_default();
            let photo = photo_by_path
                .get(&normalize_path_key(&source_file))
                .copied()
                .or_else(|| photos.get(index))
                .ok_or_else(|| {
                    AppError::new(
                        ErrorCode::PhotoMetadataFailed,
                        "ExifTool 返回了无法对应到照片的条目。",
                        "请重新扫描照片目录后重试。",
                        true,
                    )
                })?;
            seen.insert(photo.id, ());
            results.push(metadata_from_object(photo, object, timezone));
        }

        for photo in photos {
            if !seen.contains_key(&photo.id) {
                results.push(PhotoMetadata {
                    photo_id: photo.id,
                    source_file: photo.path.clone(),
                    date_time_original: None,
                    sub_sec_date_time_original: None,
                    offset_time_original: None,
                    create_date: None,
                    capture_utc: None,
                    timezone_source: TimezoneSource::Unknown,
                    existing_gps: None,
                    error: Some("ExifTool 没有返回此文件的元数据。".to_owned()),
                });
            }
        }
        results.sort_by_key(|item| item.photo_id);
        Ok(results)
    }

    pub fn write_gps(
        &self,
        path: &Path,
        gps: &ExistingGps,
        include_altitude: bool,
    ) -> AppResult<()> {
        let mut command = Command::new(&self.executable);
        command
            .arg("-overwrite_original")
            .arg("-n")
            .arg("-charset")
            .arg("filename=UTF8")
            .arg(format!("-GPSLatitude={:.10}", gps.lat.abs()))
            .arg(format!(
                "-GPSLatitudeRef={}",
                if gps.lat < 0.0 { "S" } else { "N" }
            ))
            .arg(format!("-GPSLongitude={:.10}", gps.lon.abs()))
            .arg(format!(
                "-GPSLongitudeRef={}",
                if gps.lon < 0.0 { "W" } else { "E" }
            ));
        if include_altitude {
            if let Some(altitude) = gps.altitude {
                command
                    .arg(format!("-GPSAltitude={:.4}", altitude.abs()))
                    .arg(format!(
                        "-GPSAltitudeRef={}",
                        if altitude < 0.0 { 1 } else { 0 }
                    ));
            }
        }
        let output = command
            .arg(path)
            .stdin(Stdio::null())
            .output()
            .map_err(unavailable_to_start)?;
        if !output.status.success() {
            return Err(AppError::new(
                ErrorCode::WritePermissionDenied,
                format!("ExifTool 写入失败：{}", redacted_stderr(&output.stderr)),
                "请确认输出文件可写、格式受支持，并重新生成写入计划。",
                true,
            ));
        }
        Ok(())
    }
}

fn metadata_from_object(photo: &Photo, object: &Map<String, Value>, timezone: Tz) -> PhotoMetadata {
    let date_time_original = string_value(object, "DateTimeOriginal");
    let sub_sec_date_time_original = string_value(object, "SubSecDateTimeOriginal");
    let offset_time_original = string_value(object, "OffsetTimeOriginal");
    let create_date = string_value(object, "CreateDate");
    let capture_local = sub_sec_date_time_original
        .as_deref()
        .or(date_time_original.as_deref())
        .or(create_date.as_deref());

    let (capture_utc, timezone_source, time_error) = match capture_local {
        Some(local) => match parse_capture_time(local, offset_time_original.as_deref(), timezone) {
            Ok((capture, source)) => (Some(capture), source, None),
            Err(message) => (None, TimezoneSource::Unknown, Some(message)),
        },
        None => (None, TimezoneSource::Unknown, None),
    };
    let latitude = number_value(object, "GPSLatitude");
    let longitude = number_value(object, "GPSLongitude");
    let altitude = number_value(object, "GPSAltitude").map(|value| {
        if number_value(object, "GPSAltitudeRef")
            .map(|reference| reference.round() as i32 == 1)
            .unwrap_or(false)
        {
            -value.abs()
        } else {
            value
        }
    });
    let existing_gps = latitude
        .zip(longitude)
        .map(|(lat, lon)| ExistingGps { lat, lon, altitude });
    let exiftool_error = string_value(object, "Error");
    let error = exiftool_error.or(time_error);

    PhotoMetadata {
        photo_id: photo.id,
        source_file: photo.path.clone(),
        date_time_original,
        sub_sec_date_time_original,
        offset_time_original,
        create_date,
        capture_utc,
        timezone_source,
        existing_gps,
        error,
    }
}

fn parse_capture_time(
    value: &str,
    separate_offset: Option<&str>,
    timezone: Tz,
) -> Result<(DateTime<Utc>, TimezoneSource), String> {
    for format in [
        "%Y:%m:%d %H:%M:%S%.f%:z",
        "%Y:%m:%d %H:%M:%S%.f%z",
        "%Y-%m-%dT%H:%M:%S%.f%:z",
        "%Y-%m-%dT%H:%M:%S%.f%z",
    ] {
        if let Ok(parsed) = DateTime::parse_from_str(value, format) {
            return Ok((parsed.with_timezone(&Utc), TimezoneSource::MetadataOffset));
        }
    }
    if let Some(offset) = separate_offset {
        let combined = format!("{value}{offset}");
        for format in ["%Y:%m:%d %H:%M:%S%.f%:z", "%Y:%m:%d %H:%M:%S%.f%z"] {
            if let Ok(parsed) = DateTime::parse_from_str(&combined, format) {
                return Ok((parsed.with_timezone(&Utc), TimezoneSource::MetadataOffset));
            }
        }
    }

    let naive = [
        "%Y:%m:%d %H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S%.f",
    ]
    .iter()
    .find_map(|format| NaiveDateTime::parse_from_str(value, format).ok())
    .ok_or_else(|| format!("无法解析拍摄时间：{value}"))?;
    match timezone.from_local_datetime(&naive) {
        LocalResult::Single(datetime) => {
            Ok((datetime.with_timezone(&Utc), TimezoneSource::ProjectDefault))
        }
        LocalResult::Ambiguous(_, _) => Err(format!(
            "拍摄时间 {value} 在时区 {timezone} 的夏令时切换中有两个可能值。"
        )),
        LocalResult::None => Err(format!(
            "拍摄时间 {value} 在时区 {timezone} 中不存在（夏令时跳变）。"
        )),
    }
}

fn string_value(object: &Map<String, Value>, key: &str) -> Option<String> {
    object.get(key).and_then(|value| match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    })
}

fn number_value(object: &Map<String, Value>, key: &str) -> Option<f64> {
    object.get(key).and_then(|value| match value {
        Value::Number(number) => number.as_f64(),
        Value::String(value) => value.parse::<f64>().ok(),
        _ => None,
    })
}

fn normalize_path_key(value: &str) -> String {
    if cfg!(windows) {
        value.replace('\\', "/").to_ascii_lowercase()
    } else {
        value.to_owned()
    }
}

fn missing_exiftool() -> AppError {
    AppError::new(
        ErrorCode::ExiftoolNotAvailable,
        "应用资源目录中没有可用的 ExifTool。",
        "请使用 GitHub Actions 发布的完整安装包；应用不会在本机自动安装 ExifTool。",
        true,
    )
}

fn unavailable_to_start(error: std::io::Error) -> AppError {
    AppError::new(
        ErrorCode::ExiftoolNotAvailable,
        format!("无法启动内置 ExifTool：{error}"),
        "请使用 GitHub Actions 发布的完整安装包，或重新安装应用。",
        true,
    )
}

fn redacted_stderr(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        "未提供错误详情".to_owned()
    } else {
        format!(
            "ExifTool 返回了 {} 字节错误信息（完整路径和坐标已隐藏）",
            bytes.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_offset_time_before_project_timezone() {
        let timezone = "Asia/Shanghai".parse::<Tz>().expect("timezone");
        let (parsed, source) =
            parse_capture_time("2024:10:03 09:12:45.123", Some("+08:00"), timezone).expect("parse");
        assert_eq!(parsed.to_rfc3339(), "2024-10-03T01:12:45.123+00:00");
        assert_eq!(source, TimezoneSource::MetadataOffset);
    }

    #[test]
    fn parses_local_time_with_project_timezone() {
        let timezone = "Asia/Shanghai".parse::<Tz>().expect("timezone");
        let (parsed, source) =
            parse_capture_time("2024:10:03 09:12:45", None, timezone).expect("parse");
        assert_eq!(parsed.to_rfc3339(), "2024-10-03T01:12:45+00:00");
        assert_eq!(source, TimezoneSource::ProjectDefault);
    }

    #[test]
    fn missing_resource_returns_business_error() {
        let directory = tempfile::tempdir().expect("temp dir");
        let error = ExifTool::discover(directory.path()).expect_err("missing");
        assert_eq!(error.code, ErrorCode::ExiftoolNotAvailable);
    }
}
