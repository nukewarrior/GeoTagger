use crate::domain::{DiagnosticSummary, MatchStatus, ProjectSnapshot, ReportRecord};
use crate::error::{AppError, AppResult};
use crate::fs_utils::{normalize_absolute, write_atomic};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReportFormat {
    Csv,
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReportRequest {
    pub format: ReportFormat,
    pub target_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub format: ReportFormat,
    pub target_path: String,
    pub record_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonReport<'a> {
    schema_version: u32,
    project_id: uuid::Uuid,
    project_name: &'a str,
    generated_at: chrono::DateTime<Utc>,
    summary: DiagnosticSummary,
    records: &'a [ReportRecord],
    write_history: &'a [crate::domain::WriteJob],
}

pub fn export_report(
    snapshot: &ProjectSnapshot,
    request: &ExportReportRequest,
) -> AppResult<ExportResult> {
    let expected_extension = match request.format {
        ReportFormat::Csv => "csv",
        ReportFormat::Json => "json",
    };
    let actual_extension = Path::new(&request.target_path)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase);
    if actual_extension.as_deref() != Some(expected_extension) {
        return Err(AppError::invalid(format!(
            "报告文件扩展名必须为 .{expected_extension}。"
        )));
    }
    if request.target_path.trim().is_empty() || !Path::new(&request.target_path).is_absolute() {
        return Err(AppError::invalid("报告目标必须是非空绝对路径。"));
    }
    let records = build_records(snapshot);
    let bytes = match request.format {
        ReportFormat::Json => {
            let report = JsonReport {
                schema_version: snapshot.schema_version,
                project_id: snapshot.project.id,
                project_name: &snapshot.project.name,
                generated_at: Utc::now(),
                summary: diagnostic_summary(snapshot),
                records: &records,
                write_history: &snapshot.write_history,
            };
            serde_json::to_vec_pretty(&report)
                .map_err(|error| AppError::internal(format!("JSON 报告序列化失败：{error}")))?
        }
        ReportFormat::Csv => {
            let mut writer = csv::WriterBuilder::new()
                .has_headers(false)
                .from_writer(Vec::new());
            writer
                .write_record([
                    "photoId",
                    "relativePath",
                    "captureUtc",
                    "matchStatus",
                    "latitude",
                    "longitude",
                    "altitude",
                    "confidence",
                    "reason",
                    "latestWriteStatus",
                ])
                .map_err(|error| AppError::internal(format!("CSV 报告表头生成失败：{error}")))?;
            for record in &records {
                writer
                    .serialize(record)
                    .map_err(|error| AppError::internal(format!("CSV 报告序列化失败：{error}")))?;
            }
            writer.into_inner().map_err(|error| {
                AppError::internal(format!("CSV 报告生成失败：{}", error.error()))
            })?
        }
    };
    let target = normalize_absolute(Path::new(&request.target_path))?;
    write_atomic(&target, &bytes)?;
    Ok(ExportResult {
        format: request.format,
        target_path: target.to_string_lossy().into_owned(),
        record_count: records.len(),
    })
}

pub fn build_records(snapshot: &ProjectSnapshot) -> Vec<ReportRecord> {
    let matches = snapshot
        .matches
        .iter()
        .map(|photo_match| (photo_match.photo_id, photo_match))
        .collect::<BTreeMap<_, _>>();
    let mut latest_write = BTreeMap::new();
    for job in &snapshot.write_history {
        for result in &job.results {
            latest_write.insert(result.photo_id, result.status);
        }
    }
    let mut records = snapshot
        .photos
        .iter()
        .map(|photo| {
            let photo_match = matches.get(&photo.id).copied();
            ReportRecord {
                photo_id: photo.id,
                relative_path: photo.relative_path.clone(),
                capture_utc: photo.capture_utc,
                match_status: photo_match.map(|value| value.status),
                latitude: photo_match.and_then(|value| value.lat),
                longitude: photo_match.and_then(|value| value.lon),
                altitude: photo_match.and_then(|value| value.elevation),
                confidence: photo_match.and_then(|value| value.confidence),
                reason: photo_match.map(|value| value.reason.clone()),
                latest_write_status: latest_write.get(&photo.id).copied(),
            }
        })
        .collect::<Vec<_>>();
    records.sort_by_key(|record| record.photo_id);
    records
}

pub fn diagnostic_summary(snapshot: &ProjectSnapshot) -> DiagnosticSummary {
    let mut match_status_counts = BTreeMap::<String, usize>::new();
    for photo_match in &snapshot.matches {
        let key = status_name(photo_match.status);
        *match_status_counts.entry(key).or_default() += 1;
    }
    DiagnosticSummary {
        schema_version: snapshot.schema_version,
        track_count: snapshot.tracks.len(),
        photo_count: snapshot.photos.len(),
        match_status_counts,
        write_job_count: snapshot.write_history.len(),
    }
}

fn status_name(status: MatchStatus) -> String {
    serde_json::to_value(status)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "UNKNOWN".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::create_snapshot;

    #[test]
    fn json_report_does_not_require_photo_paths_in_summary() {
        let snapshot = create_snapshot("Report".to_owned(), None);
        let summary = diagnostic_summary(&snapshot);
        assert_eq!(summary.photo_count, 0);
        assert!(summary.match_status_counts.is_empty());
    }

    #[test]
    fn writes_empty_csv_report() {
        let snapshot = create_snapshot("Report".to_owned(), None);
        let directory = tempfile::tempdir().expect("temp dir");
        let target = directory.path().join("report.csv");
        let result = export_report(
            &snapshot,
            &ExportReportRequest {
                format: ReportFormat::Csv,
                target_path: target.to_string_lossy().into_owned(),
            },
        )
        .expect("export");
        assert_eq!(result.record_count, 0);
        assert!(target.exists());
    }
}
