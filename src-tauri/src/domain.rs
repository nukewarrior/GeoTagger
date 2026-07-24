use crate::error::AppError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

pub const PROJECT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoordinateSystem {
    #[serde(rename = "WGS84")]
    Wgs84,
    #[serde(rename = "GCJ02")]
    Gcj02,
    #[serde(rename = "BD09")]
    Bd09,
    #[serde(rename = "UNKNOWN")]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeoPoint {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OriginalPoint {
    pub lat: f64,
    pub lon: f64,
    pub crs: CoordinateSystem,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackPoint {
    pub time_utc: DateTime<Utc>,
    pub original: OriginalPoint,
    pub normalized: GeoPoint,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elevation: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hdop: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackSegment {
    pub id: Uuid,
    pub source_index: usize,
    pub points: Vec<TrackPoint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeoBounds {
    pub min_lat: f64,
    pub min_lon: f64,
    pub max_lat: f64,
    pub max_lon: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackStatistics {
    pub distance_meters: f64,
    pub duration_seconds: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_elevation: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_elevation: Option<f64>,
    pub segment_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackWarning {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segment_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub point_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: Uuid,
    pub name: String,
    pub source_path: String,
    pub relative_path: String,
    pub hash_sha256: String,
    pub source_crs: CoordinateSystem,
    pub segments: Vec<TrackSegment>,
    pub start_utc: DateTime<Utc>,
    pub end_utc: DateTime<Utc>,
    pub point_count: usize,
    pub bounds: GeoBounds,
    pub statistics: TrackStatistics,
    pub warnings: Vec<TrackWarning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_cache: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileFingerprint {
    pub sha256: String,
    pub size_bytes: u64,
    pub modified_unix_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TimezoneSource {
    MetadataOffset,
    ProjectDefault,
    UserOverride,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExistingGps {
    pub lat: f64,
    pub lon: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub altitude: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PhotoMetadataStatus {
    Pending,
    Ready,
    AmbiguousTime,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Photo {
    pub id: Uuid,
    pub path: String,
    pub relative_path: String,
    pub file_name: String,
    pub extension: String,
    pub fingerprint: FileFingerprint,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_local: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_utc: Option<DateTime<Utc>>,
    pub timezone_source: TimezoneSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub existing_gps: Option<ExistingGps>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    pub metadata_status: PhotoMetadataStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPoint {
    pub capture_local: String,
    pub actual_utc: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Calibration {
    pub timezone: String,
    pub fixed_offset_ms: i64,
    #[serde(default)]
    pub sync_points: Vec<SyncPoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drift_model: Option<String>,
}

impl Default for Calibration {
    fn default() -> Self {
        Self {
            timezone: "Asia/Shanghai".to_owned(),
            fixed_offset_ms: 0,
            sync_points: Vec::new(),
            drift_model: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MatchStatus {
    MatchedHigh,
    MatchedMedium,
    MatchedLow,
    OutOfRange,
    NoCaptureTime,
    SegmentGap,
    AlreadyHasGps,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoMatch {
    pub photo_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segment_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lat: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lon: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elevation: Option<f64>,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    pub status: MatchStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality_status: Option<MatchStatus>,
    pub reason: String,
    pub existing_gps_conflict: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_time_utc: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_point_time_utc: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_point_time_utc: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_error_meters: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MapProvider {
    #[serde(rename = "maplibre")]
    Maplibre,
    #[serde(rename = "amap")]
    Amap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OutputMode {
    CopyToDirectory,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSettings {
    pub photo_timezone: String,
    pub fixed_offset_ms: i64,
    pub map_provider: MapProvider,
    pub output_mode: OutputMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_output_directory: Option<String>,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            photo_timezone: "Asia/Shanghai".to_owned(),
            fixed_offset_ms: 0,
            map_provider: MapProvider::Maplibre,
            output_mode: OutputMode::CopyToDirectory,
            default_output_directory: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSnapshot {
    pub schema_version: u32,
    pub project: ProjectInfo,
    pub settings: ProjectSettings,
    #[serde(default)]
    pub calibration: Calibration,
    #[serde(default)]
    pub tracks: Vec<Track>,
    #[serde(default)]
    pub photos: Vec<Photo>,
    #[serde(default)]
    pub matches: Vec<PhotoMatch>,
    #[serde(default)]
    pub write_history: Vec<WriteJob>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExistingGpsPolicy {
    Skip,
    Overwrite,
    Preserve,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteOptions {
    pub existing_gps_policy: ExistingGpsPolicy,
    pub include_altitude: bool,
    pub preserve_relative_paths: bool,
    pub overwrite_output: bool,
}

impl Default for WriteOptions {
    fn default() -> Self {
        Self {
            existing_gps_policy: ExistingGpsPolicy::Skip,
            include_altitude: true,
            preserve_relative_paths: true,
            overwrite_output: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WritePlanItemAction {
    WriteGps,
    SkipExistingGps,
    PreserveExistingGps,
    SkipNoMatch,
    SkipUnsupportedFormat,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WritePlanItem {
    pub photo_id: Uuid,
    pub source_path: String,
    pub output_path: String,
    pub source_fingerprint: FileFingerprint,
    pub action: WritePlanItemAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_gps: Option<ExistingGps>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_gps: Option<ExistingGps>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WritePlan {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub output_directory: String,
    pub options: WriteOptions,
    pub items: Vec<WritePlanItem>,
    pub writable_count: usize,
    pub skipped_count: usize,
    pub conflict_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WriteItemStatus {
    WrittenVerified,
    Skipped,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteItemResult {
    pub photo_id: Uuid,
    pub output_path: String,
    pub status: WriteItemStatus,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<AppError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WriteJobStatus {
    Running,
    Completed,
    CompletedWithErrors,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteJob {
    pub id: Uuid,
    pub write_plan_id: Uuid,
    pub selected_photo_ids: Vec<Uuid>,
    pub output_dir: String,
    pub options: WriteOptions,
    pub status: WriteJobStatus,
    pub started_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    pub results: Vec<WriteItemResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskKind {
    PhotoScan,
    MatchCalculation,
    WriteExif,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecord {
    pub id: Uuid,
    pub kind: TaskKind,
    pub status: TaskStatus,
    pub stage: String,
    pub completed: u64,
    pub total: u64,
    pub message: String,
    pub started_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<AppError>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskAccepted {
    pub task_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgressEvent {
    pub task_id: Uuid,
    pub stage: String,
    pub completed: u64,
    pub total: u64,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskFinishedEvent {
    pub task_id: Uuid,
    pub summary: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskWarningEvent {
    pub task_id: Uuid,
    pub code: String,
    pub file: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskFailedEvent {
    pub task_id: Uuid,
    pub error: AppError,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDirtyEvent {
    pub changed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub project_path: String,
    pub project: ProjectInfo,
    pub track_count: usize,
    pub photo_count: usize,
    pub match_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub project_path: String,
    pub saved_at: DateTime<Utc>,
    pub schema_version: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoMetadata {
    pub photo_id: Uuid,
    pub source_file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_time_original: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_sec_date_time_original: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset_time_original: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_utc: Option<DateTime<Utc>>,
    pub timezone_source: TimezoneSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub existing_gps: Option<ExistingGps>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExifToolStatus {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<AppError>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionPreviewPoint {
    pub original: GeoPoint,
    pub normalized: GeoPoint,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertedPreview {
    pub track_id: Uuid,
    pub source_crs: CoordinateSystem,
    pub points: Vec<ConversionPreviewPoint>,
    pub bounds: GeoBounds,
    pub sample_count: usize,
    pub total_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportRecord {
    pub photo_id: Uuid,
    pub relative_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_utc: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_status: Option<MatchStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub longitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub altitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_write_status: Option<WriteItemStatus>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticSummary {
    pub schema_version: u32,
    pub track_count: usize,
    pub photo_count: usize,
    pub match_status_counts: BTreeMap<String, usize>,
    pub write_job_count: usize,
}
