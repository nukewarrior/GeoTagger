use crate::coordinate::{convert, haversine_meters};
use crate::domain::{
    CoordinateSystem, GeoBounds, GeoPoint, Track, TrackPoint, TrackSegment, TrackStatistics,
    TrackWarning,
};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::fs_utils::{canonical_file, sha256_file};
use chrono::{DateTime, Utc};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::cmp::Ordering;
use std::fs;
use std::path::Path;
use uuid::Uuid;

#[derive(Default)]
struct PointBuilder {
    lat: Option<f64>,
    lon: Option<f64>,
    time: Option<DateTime<Utc>>,
    elevation: Option<f64>,
    hdop: Option<f64>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TextField {
    TrackName,
    Time,
    Elevation,
    Hdop,
}

pub fn parse_gpx_file(
    path: &Path,
    source_crs: CoordinateSystem,
    project_root: Option<&Path>,
) -> AppResult<Track> {
    if source_crs == CoordinateSystem::Unknown {
        return Err(AppError::new(
            ErrorCode::CrsUnconfirmed,
            "无法导入未确认坐标系的轨迹。",
            "请先选择 WGS84、GCJ-02 或 BD-09。",
            true,
        ));
    }
    let canonical = canonical_file(path)?;
    let bytes = fs::read(&canonical).map_err(|error| AppError::io("读取 GPX 失败", error))?;
    let hash = sha256_file(&canonical)?;
    let relative_path = project_root
        .and_then(|root| canonical.strip_prefix(root).ok())
        .unwrap_or_else(|| {
            canonical
                .file_name()
                .map(Path::new)
                .unwrap_or(canonical.as_path())
        })
        .to_string_lossy()
        .into_owned();
    parse_gpx_bytes(
        &bytes,
        canonical.to_string_lossy().into_owned(),
        relative_path,
        hash,
        source_crs,
    )
}

pub fn parse_gpx_bytes(
    bytes: &[u8],
    source_path: String,
    relative_path: String,
    hash_sha256: String,
    source_crs: CoordinateSystem,
) -> AppResult<Track> {
    if source_crs == CoordinateSystem::Unknown {
        return Err(AppError::new(
            ErrorCode::CrsUnconfirmed,
            "轨迹坐标系尚未确认。",
            "请确认轨迹原始坐标系后再导入。",
            true,
        ));
    }

    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut track_name: Option<String> = None;
    let mut inside_track = false;
    let mut current_segment: Option<Vec<PointBuilder>> = None;
    let mut segments: Vec<Vec<PointBuilder>> = Vec::new();
    let mut current_point: Option<PointBuilder> = None;
    let mut active_field: Option<TextField> = None;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(element)) => match element.local_name().as_ref() {
                b"trk" => inside_track = true,
                b"trkseg" => {
                    if current_segment.is_some() {
                        return Err(track_parse_error("GPX 包含嵌套 trkseg。"));
                    }
                    current_segment = Some(Vec::new());
                }
                b"trkpt" => {
                    let mut point = PointBuilder::default();
                    for attribute in element.attributes().with_checks(false) {
                        let attribute = attribute.map_err(|error| {
                            track_parse_error(format!("轨迹点属性无效：{error}"))
                        })?;
                        let value = attribute
                            .decode_and_unescape_value(reader.decoder())
                            .map_err(|error| {
                                track_parse_error(format!("轨迹点属性编码无效：{error}"))
                            })?;
                        match attribute.key.local_name().as_ref() {
                            b"lat" => point.lat = value.parse::<f64>().ok(),
                            b"lon" => point.lon = value.parse::<f64>().ok(),
                            _ => {}
                        }
                    }
                    current_point = Some(point);
                }
                b"name" if inside_track && current_point.is_none() && track_name.is_none() => {
                    active_field = Some(TextField::TrackName)
                }
                b"time" if current_point.is_some() => active_field = Some(TextField::Time),
                b"ele" if current_point.is_some() => active_field = Some(TextField::Elevation),
                b"hdop" if current_point.is_some() => active_field = Some(TextField::Hdop),
                _ => {}
            },
            Ok(Event::Text(text)) => {
                let value = text
                    .unescape()
                    .map_err(|error| track_parse_error(format!("GPX 文本编码无效：{error}")))?
                    .trim()
                    .to_owned();
                match active_field {
                    Some(TextField::TrackName) if !value.is_empty() => track_name = Some(value),
                    Some(TextField::Time) => {
                        if let Some(point) = current_point.as_mut() {
                            point.time = DateTime::parse_from_rfc3339(&value)
                                .ok()
                                .map(|time| time.with_timezone(&Utc));
                        }
                    }
                    Some(TextField::Elevation) => {
                        if let Some(point) = current_point.as_mut() {
                            point.elevation = value.parse::<f64>().ok();
                        }
                    }
                    Some(TextField::Hdop) => {
                        if let Some(point) = current_point.as_mut() {
                            point.hdop = value.parse::<f64>().ok();
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(element)) => match element.local_name().as_ref() {
                b"trk" => inside_track = false,
                b"name" | b"time" | b"ele" | b"hdop" => active_field = None,
                b"trkpt" => {
                    let point = current_point
                        .take()
                        .ok_or_else(|| track_parse_error("GPX 轨迹点结束标签没有对应开始标签。"))?;
                    let segment = current_segment
                        .as_mut()
                        .ok_or_else(|| track_parse_error("GPX trkpt 必须位于 trkseg 内。"))?;
                    segment.push(point);
                }
                b"trkseg" => {
                    let segment = current_segment
                        .take()
                        .ok_or_else(|| track_parse_error("GPX 分段结束标签没有对应开始标签。"))?;
                    if !segment.is_empty() {
                        segments.push(segment);
                    }
                }
                _ => {}
            },
            Ok(Event::Empty(element)) if element.local_name().as_ref() == b"trkpt" => {
                return Err(AppError::new(
                    ErrorCode::TrackNoTime,
                    "GPX 包含没有 time 的空轨迹点。",
                    "请选择包含 ISO 8601 time 的 GPX，或先修复轨迹时间。",
                    true,
                ));
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => {
                return Err(track_parse_error(format!(
                    "GPX XML 解析失败（字节 {}）：{error}",
                    reader.buffer_position()
                )))
            }
        }
        buffer.clear();
    }

    if current_point.is_some() || current_segment.is_some() {
        return Err(track_parse_error("GPX 在轨迹或分段结束前终止。"));
    }
    if segments.is_empty() {
        return Err(track_parse_error("GPX 未包含有效 trkseg/trkpt。"));
    }

    let stable_key = format!("{source_path}|{hash_sha256}|{source_crs:?}");
    let track_id = Uuid::new_v5(&Uuid::NAMESPACE_URL, stable_key.as_bytes());
    let mut normalized_segments = Vec::with_capacity(segments.len());
    let mut warnings = Vec::new();

    for (segment_index, raw_segment) in segments.into_iter().enumerate() {
        let mut points = Vec::with_capacity(raw_segment.len());
        for (point_index, raw_point) in raw_segment.into_iter().enumerate() {
            let lat = raw_point.lat.ok_or_else(|| {
                track_parse_error(format!(
                    "分段 {} 的轨迹点 {} 缺少纬度。",
                    segment_index + 1,
                    point_index + 1
                ))
            })?;
            let lon = raw_point.lon.ok_or_else(|| {
                track_parse_error(format!(
                    "分段 {} 的轨迹点 {} 缺少经度。",
                    segment_index + 1,
                    point_index + 1
                ))
            })?;
            let time_utc = raw_point.time.ok_or_else(|| {
                AppError::new(
                    ErrorCode::TrackNoTime,
                    format!(
                        "分段 {} 的轨迹点 {} 缺少有效时间。",
                        segment_index + 1,
                        point_index + 1
                    ),
                    "请选择包含 ISO 8601 time 的 GPX，或先修复轨迹时间。",
                    true,
                )
            })?;
            let original = GeoPoint { lat, lon };
            let normalized = convert(original, source_crs, CoordinateSystem::Wgs84)?;
            points.push(TrackPoint {
                time_utc,
                original: crate::domain::OriginalPoint {
                    lat,
                    lon,
                    crs: source_crs,
                },
                normalized,
                elevation: raw_point.elevation,
                hdop: raw_point.hdop,
            });
        }

        if points
            .windows(2)
            .any(|pair| pair[0].time_utc > pair[1].time_utc)
        {
            warnings.push(TrackWarning {
                code: "TRACK_TIME_NOT_SORTED".to_owned(),
                message: "轨迹点时间不是递增顺序，已在该分段内稳定排序。".to_owned(),
                segment_index: Some(segment_index),
                point_index: None,
            });
        }
        points.sort_by(|left, right| {
            left.time_utc
                .cmp(&right.time_utc)
                .then_with(|| {
                    left.normalized
                        .lat
                        .partial_cmp(&right.normalized.lat)
                        .unwrap_or(Ordering::Equal)
                })
                .then_with(|| {
                    left.normalized
                        .lon
                        .partial_cmp(&right.normalized.lon)
                        .unwrap_or(Ordering::Equal)
                })
        });

        for (point_index, pair) in points.windows(2).enumerate() {
            let delta_ms = (pair[1].time_utc - pair[0].time_utc).num_milliseconds();
            if delta_ms == 0 {
                warnings.push(TrackWarning {
                    code: "TRACK_DUPLICATE_TIME".to_owned(),
                    message: "同一分段包含重复时间点；精确匹配将选择排序后的第一个点。".to_owned(),
                    segment_index: Some(segment_index),
                    point_index: Some(point_index + 1),
                });
                continue;
            }
            let distance = haversine_meters(pair[0].normalized, pair[1].normalized);
            let speed = distance / (delta_ms as f64 / 1000.0);
            if speed > 120.0 {
                warnings.push(TrackWarning {
                    code: "TRACK_SUSPICIOUS_JUMP".to_owned(),
                    message: format!("检测到约 {speed:.1} m/s 的异常跳点，匹配置信度将降级。"),
                    segment_index: Some(segment_index),
                    point_index: Some(point_index + 1),
                });
            }
        }

        let segment_key = format!("{track_id}:{segment_index}");
        normalized_segments.push(TrackSegment {
            id: Uuid::new_v5(&Uuid::NAMESPACE_URL, segment_key.as_bytes()),
            source_index: segment_index,
            points,
        });
    }

    build_track(TrackBuildInput {
        id: track_id,
        track_name,
        source_path,
        relative_path,
        hash_sha256,
        source_crs,
        segments: normalized_segments,
        warnings,
    })
}

struct TrackBuildInput {
    id: Uuid,
    track_name: Option<String>,
    source_path: String,
    relative_path: String,
    hash_sha256: String,
    source_crs: CoordinateSystem,
    segments: Vec<TrackSegment>,
    warnings: Vec<TrackWarning>,
}

fn build_track(input: TrackBuildInput) -> AppResult<Track> {
    let TrackBuildInput {
        id,
        track_name,
        source_path,
        relative_path,
        hash_sha256,
        source_crs,
        segments,
        warnings,
    } = input;
    let all_points: Vec<&TrackPoint> = segments
        .iter()
        .flat_map(|segment| segment.points.iter())
        .collect();
    let first = all_points
        .first()
        .ok_or_else(|| track_parse_error("轨迹没有有效点。"))?;
    let mut min_lat = first.normalized.lat;
    let mut min_lon = first.normalized.lon;
    let mut max_lat = first.normalized.lat;
    let mut max_lon = first.normalized.lon;
    let mut start_utc = first.time_utc;
    let mut end_utc = first.time_utc;
    let mut min_elevation: Option<f64> = None;
    let mut max_elevation: Option<f64> = None;

    for point in &all_points {
        min_lat = min_lat.min(point.normalized.lat);
        min_lon = min_lon.min(point.normalized.lon);
        max_lat = max_lat.max(point.normalized.lat);
        max_lon = max_lon.max(point.normalized.lon);
        start_utc = start_utc.min(point.time_utc);
        end_utc = end_utc.max(point.time_utc);
        if let Some(elevation) = point.elevation {
            min_elevation = Some(min_elevation.map_or(elevation, |value| value.min(elevation)));
            max_elevation = Some(max_elevation.map_or(elevation, |value| value.max(elevation)));
        }
    }

    let distance_meters = segments
        .iter()
        .map(|segment| {
            segment
                .points
                .windows(2)
                .map(|pair| haversine_meters(pair[0].normalized, pair[1].normalized))
                .sum::<f64>()
        })
        .sum();
    let fallback_name = Path::new(&source_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("未命名轨迹")
        .to_owned();

    Ok(Track {
        id,
        name: track_name
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(fallback_name),
        source_path,
        relative_path,
        hash_sha256,
        source_crs,
        point_count: all_points.len(),
        bounds: GeoBounds {
            min_lat,
            min_lon,
            max_lat,
            max_lon,
        },
        statistics: TrackStatistics {
            distance_meters,
            duration_seconds: (end_utc - start_utc).num_seconds(),
            min_elevation,
            max_elevation,
            segment_count: segments.len(),
        },
        start_utc,
        end_utc,
        segments,
        warnings,
        normalized_cache: None,
    })
}

fn track_parse_error(message: impl Into<String>) -> AppError {
    AppError::new(
        ErrorCode::TrackParseFailed,
        message,
        "请确认文件是结构完整的 GPX，并检查轨迹点、时间和坐标字段。",
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEGMENTED_GPX: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="test">
  <trk>
    <name>Segmented</name>
    <trkseg>
      <trkpt lat="30.0000" lon="120.0000"><ele>10</ele><time>2024-01-01T00:00:10Z</time></trkpt>
      <trkpt lat="30.0001" lon="120.0001"><ele>11</ele><time>2024-01-01T00:00:00Z</time></trkpt>
    </trkseg>
    <trkseg>
      <trkpt lat="31.0000" lon="121.0000"><time>2024-01-01T01:00:00Z</time></trkpt>
    </trkseg>
  </trk>
</gpx>"#;

    #[test]
    fn preserves_segments_and_sorts_inside_each_segment() {
        let track = parse_gpx_bytes(
            SEGMENTED_GPX.as_bytes(),
            "/tmp/segmented.gpx".to_owned(),
            "segmented.gpx".to_owned(),
            "fixture-hash".to_owned(),
            CoordinateSystem::Wgs84,
        )
        .expect("parse");
        assert_eq!(track.segments.len(), 2);
        assert_eq!(track.point_count, 3);
        assert!(track.segments[0].points[0].time_utc < track.segments[0].points[1].time_utc);
        assert!(track
            .warnings
            .iter()
            .any(|warning| warning.code == "TRACK_TIME_NOT_SORTED"));
    }

    #[test]
    fn rejects_missing_time() {
        let missing_time = br#"<gpx><trk><trkseg><trkpt lat="1" lon="2"/></trkseg></trk></gpx>"#;
        let error = parse_gpx_bytes(
            missing_time,
            "missing.gpx".to_owned(),
            "missing.gpx".to_owned(),
            "hash".to_owned(),
            CoordinateSystem::Wgs84,
        )
        .expect_err("must reject");
        assert_eq!(error.code, ErrorCode::TrackNoTime);
    }
}
