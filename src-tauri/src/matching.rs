use crate::coordinate::haversine_meters;
use crate::domain::{Calibration, MatchStatus, Photo, PhotoMatch, Track, TrackPoint, TrackSegment};
use crate::error::{AppError, AppResult, ErrorCode};
use chrono::{DateTime, Duration, Utc};
use std::cmp::Ordering;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use uuid::Uuid;

#[derive(Debug)]
struct Candidate<'a> {
    track: &'a Track,
    segment: &'a TrackSegment,
    previous: &'a TrackPoint,
    next: &'a TrackPoint,
    ratio: f64,
    interval_seconds: f64,
}

pub fn calculate_matches<F>(
    photos: &[Photo],
    tracks: &[Track],
    selected_photo_ids: Option<&[Uuid]>,
    selected_track_ids: Option<&[Uuid]>,
    calibration: &Calibration,
    cancelled: &AtomicBool,
    mut progress: F,
) -> AppResult<Vec<PhotoMatch>>
where
    F: FnMut(u64, u64, &str),
{
    let mut selected_tracks: Vec<&Track> = tracks
        .iter()
        .filter(|track| {
            selected_track_ids
                .map(|ids| ids.contains(&track.id))
                .unwrap_or(true)
        })
        .collect();
    selected_tracks.sort_by_key(|track| track.id);
    if selected_tracks.is_empty() {
        return Err(AppError::new(
            ErrorCode::MatchNotFound,
            "没有可用于匹配的轨迹。",
            "请先导入并选择至少一条含时间的轨迹。",
            true,
        ));
    }

    let mut selected_photos: Vec<&Photo> = photos
        .iter()
        .filter(|photo| {
            selected_photo_ids
                .map(|ids| ids.contains(&photo.id))
                .unwrap_or(true)
        })
        .collect();
    selected_photos.sort_by_key(|photo| photo.id);
    let total = selected_photos.len() as u64;
    let mut matches = Vec::with_capacity(selected_photos.len());

    for (index, photo) in selected_photos.into_iter().enumerate() {
        if cancelled.load(AtomicOrdering::Relaxed) {
            return Err(AppError::cancelled());
        }
        let result = match photo.capture_utc {
            Some(capture_utc) => {
                let corrected = capture_utc + Duration::milliseconds(calibration.fixed_offset_ms);
                match_photo(photo, corrected, &selected_tracks)
            }
            None => PhotoMatch {
                photo_id: photo.id,
                track_id: None,
                segment_id: None,
                lat: None,
                lon: None,
                elevation: None,
                method: "NONE".to_owned(),
                confidence: None,
                status: MatchStatus::NoCaptureTime,
                quality_status: None,
                reason: "照片没有可用拍摄时间。".to_owned(),
                existing_gps_conflict: photo.existing_gps.is_some(),
                matched_time_utc: None,
                previous_point_time_utc: None,
                next_point_time_utc: None,
                interval_seconds: None,
                estimated_error_meters: None,
            },
        };
        matches.push(result);
        progress(
            index as u64 + 1,
            total,
            &format!("已匹配 {}", photo.file_name),
        );
    }
    Ok(matches)
}

fn match_photo(photo: &Photo, capture_utc: DateTime<Utc>, tracks: &[&Track]) -> PhotoMatch {
    let mut candidates = Vec::new();
    let mut inside_any_track_range = false;

    for track in tracks {
        if capture_utc >= track.start_utc && capture_utc <= track.end_utc {
            inside_any_track_range = true;
        }
        for segment in &track.segments {
            if let Some(candidate) = candidate_for_segment(track, segment, capture_utc) {
                candidates.push(candidate);
            }
        }
    }

    candidates.sort_by(compare_candidates);
    let Some(candidate) = candidates.first() else {
        let (status, reason) = if inside_any_track_range {
            (
                MatchStatus::SegmentGap,
                "照片时间位于轨迹总体范围内，但落在分段空档中。",
            )
        } else {
            (
                MatchStatus::OutOfRange,
                "照片时间不在所选轨迹的时间范围内。",
            )
        };
        return PhotoMatch {
            photo_id: photo.id,
            track_id: None,
            segment_id: None,
            lat: None,
            lon: None,
            elevation: None,
            method: "NONE".to_owned(),
            confidence: None,
            status,
            quality_status: None,
            reason: reason.to_owned(),
            existing_gps_conflict: photo.existing_gps.is_some(),
            matched_time_utc: Some(capture_utc),
            previous_point_time_utc: None,
            next_point_time_utc: None,
            interval_seconds: None,
            estimated_error_meters: None,
        };
    };

    let latitude = interpolate(
        candidate.previous.normalized.lat,
        candidate.next.normalized.lat,
        candidate.ratio,
    );
    let longitude = interpolate(
        candidate.previous.normalized.lon,
        candidate.next.normalized.lon,
        candidate.ratio,
    );
    let elevation = match (candidate.previous.elevation, candidate.next.elevation) {
        (Some(previous), Some(next)) => Some(interpolate(previous, next, candidate.ratio)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    };
    let (confidence, quality_status, estimated_error) = confidence(candidate);
    let existing_gps_conflict = photo.existing_gps.is_some();
    let status = if existing_gps_conflict {
        MatchStatus::AlreadyHasGps
    } else {
        quality_status
    };
    let reason = match quality_status {
        MatchStatus::MatchedHigh => "轨迹点间隔较小且速度、精度正常。",
        MatchStatus::MatchedMedium => "轨迹点间隔或精度信息使结果需要复核。",
        MatchStatus::MatchedLow => "轨迹点间隔较大或存在速度/精度异常，请人工确认。",
        _ => unreachable!(),
    };

    PhotoMatch {
        photo_id: photo.id,
        track_id: Some(candidate.track.id),
        segment_id: Some(candidate.segment.id),
        lat: Some(latitude),
        lon: Some(longitude),
        elevation,
        method: if candidate.interval_seconds == 0.0 {
            "EXACT_TRACK_POINT".to_owned()
        } else {
            "LINEAR_TIME_INTERPOLATION".to_owned()
        },
        confidence: Some(confidence),
        status,
        quality_status: Some(quality_status),
        reason: reason.to_owned(),
        existing_gps_conflict,
        matched_time_utc: Some(capture_utc),
        previous_point_time_utc: Some(candidate.previous.time_utc),
        next_point_time_utc: Some(candidate.next.time_utc),
        interval_seconds: Some(candidate.interval_seconds),
        estimated_error_meters: Some(estimated_error),
    }
}

fn candidate_for_segment<'a>(
    track: &'a Track,
    segment: &'a TrackSegment,
    capture_utc: DateTime<Utc>,
) -> Option<Candidate<'a>> {
    if segment.points.is_empty() {
        return None;
    }
    let index = lower_bound(&segment.points, capture_utc);
    if index < segment.points.len() && segment.points[index].time_utc == capture_utc {
        let point = &segment.points[index];
        return Some(Candidate {
            track,
            segment,
            previous: point,
            next: point,
            ratio: 0.0,
            interval_seconds: 0.0,
        });
    }
    if index == 0 || index >= segment.points.len() {
        return None;
    }
    let previous = &segment.points[index - 1];
    let next = &segment.points[index];
    let interval_ms = (next.time_utc - previous.time_utc).num_milliseconds();
    if interval_ms <= 0 {
        return None;
    }
    let offset_ms = (capture_utc - previous.time_utc).num_milliseconds();
    Some(Candidate {
        track,
        segment,
        previous,
        next,
        ratio: offset_ms as f64 / interval_ms as f64,
        interval_seconds: interval_ms as f64 / 1000.0,
    })
}

fn lower_bound(points: &[TrackPoint], target: DateTime<Utc>) -> usize {
    let mut left = 0;
    let mut right = points.len();
    while left < right {
        let middle = left + (right - left) / 2;
        if points[middle].time_utc < target {
            left = middle + 1;
        } else {
            right = middle;
        }
    }
    left
}

fn compare_candidates(left: &Candidate<'_>, right: &Candidate<'_>) -> Ordering {
    left.interval_seconds
        .total_cmp(&right.interval_seconds)
        .then_with(|| left.track.id.cmp(&right.track.id))
        .then_with(|| left.segment.source_index.cmp(&right.segment.source_index))
}

fn confidence(candidate: &Candidate<'_>) -> (f64, MatchStatus, f64) {
    if candidate.interval_seconds == 0.0 {
        let hdop = candidate.previous.hdop.unwrap_or(1.0);
        let score = if hdop > 10.0 {
            0.65
        } else if hdop > 5.0 {
            0.78
        } else {
            0.98
        };
        let status = status_for_score(score);
        return (score, status, (hdop * 3.0).max(3.0));
    }

    let gap = candidate.interval_seconds;
    let mut score = if gap <= 15.0 {
        0.95
    } else if gap <= 60.0 {
        0.95 - ((gap - 15.0) / 45.0) * 0.25
    } else {
        (0.45 - ((gap - 60.0) / 900.0) * 0.20).max(0.20)
    };
    let distance = haversine_meters(candidate.previous.normalized, candidate.next.normalized);
    let speed = distance / gap;
    if speed > 120.0 {
        score *= 0.35;
    } else if speed > 55.0 {
        score *= 0.60;
    } else if speed > 30.0 {
        score *= 0.80;
    }

    let hdop = match (candidate.previous.hdop, candidate.next.hdop) {
        (Some(previous), Some(next)) => Some((previous + next) / 2.0),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    };
    match hdop {
        Some(value) if value > 10.0 => score *= 0.60,
        Some(value) if value > 5.0 => score *= 0.80,
        None => score *= 0.88,
        _ => {}
    }

    let edge_distance = candidate.ratio.min(1.0 - candidate.ratio);
    if edge_distance < 0.02 {
        score *= 0.92;
    }
    score = score.clamp(0.0, 1.0);

    // This value is deliberately heuristic. UI copy must label it as an
    // estimate rather than a measurement accuracy guarantee.
    let estimated_error = (speed * gap.min(60.0) * 0.25 + hdop.unwrap_or(4.0) * 3.0).max(5.0);
    (score, status_for_score(score), estimated_error)
}

fn status_for_score(score: f64) -> MatchStatus {
    if score >= 0.80 {
        MatchStatus::MatchedHigh
    } else if score >= 0.55 {
        MatchStatus::MatchedMedium
    } else {
        MatchStatus::MatchedLow
    }
}

fn interpolate(first: f64, second: f64, ratio: f64) -> f64 {
    first + ratio * (second - first)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        CoordinateSystem, FileFingerprint, GeoBounds, GeoPoint, OriginalPoint, PhotoMetadataStatus,
        TimezoneSource, TrackStatistics,
    };

    fn time(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("time")
            .with_timezone(&Utc)
    }

    fn point(value: &str, lat: f64, lon: f64) -> TrackPoint {
        TrackPoint {
            time_utc: time(value),
            original: OriginalPoint {
                lat,
                lon,
                crs: CoordinateSystem::Wgs84,
            },
            normalized: GeoPoint { lat, lon },
            elevation: Some(lat),
            hdop: Some(1.0),
        }
    }

    fn track() -> Track {
        let track_id = Uuid::new_v5(&Uuid::NAMESPACE_URL, b"track");
        let first_segment = TrackSegment {
            id: Uuid::new_v5(&Uuid::NAMESPACE_URL, b"segment-1"),
            source_index: 0,
            points: vec![
                point("2024-01-01T00:00:00Z", 0.0, 0.0),
                point("2024-01-01T00:00:10Z", 0.0001, 0.0001),
            ],
        };
        let second_segment = TrackSegment {
            id: Uuid::new_v5(&Uuid::NAMESPACE_URL, b"segment-2"),
            source_index: 1,
            points: vec![
                point("2024-01-01T00:01:00Z", 20.0, 20.0),
                point("2024-01-01T00:01:10Z", 30.0, 30.0),
            ],
        };
        Track {
            id: track_id,
            name: "track".to_owned(),
            source_path: "/track.gpx".to_owned(),
            relative_path: "track.gpx".to_owned(),
            hash_sha256: "hash".to_owned(),
            source_crs: CoordinateSystem::Wgs84,
            start_utc: time("2024-01-01T00:00:00Z"),
            end_utc: time("2024-01-01T00:01:10Z"),
            point_count: 4,
            bounds: GeoBounds {
                min_lat: 0.0,
                min_lon: 0.0,
                max_lat: 30.0,
                max_lon: 30.0,
            },
            statistics: TrackStatistics {
                distance_meters: 0.0,
                duration_seconds: 70,
                min_elevation: None,
                max_elevation: None,
                segment_count: 2,
            },
            warnings: Vec::new(),
            normalized_cache: None,
            segments: vec![first_segment, second_segment],
        }
    }

    fn photo(capture: Option<&str>) -> Photo {
        Photo {
            id: Uuid::new_v5(&Uuid::NAMESPACE_URL, capture.unwrap_or("none").as_bytes()),
            path: "/photo.jpg".to_owned(),
            relative_path: "photo.jpg".to_owned(),
            file_name: "photo.jpg".to_owned(),
            extension: "jpg".to_owned(),
            fingerprint: FileFingerprint {
                sha256: "hash".to_owned(),
                size_bytes: 1,
                modified_unix_ms: 0,
            },
            capture_local: None,
            capture_utc: capture.map(time),
            timezone_source: TimezoneSource::ProjectDefault,
            existing_gps: None,
            thumbnail: None,
            metadata_status: PhotoMetadataStatus::Ready,
            metadata_error: None,
        }
    }

    #[test]
    fn linearly_interpolates_inside_one_segment() {
        let track = track();
        let photo = photo(Some("2024-01-01T00:00:05Z"));
        let result = match_photo(&photo, photo.capture_utc.expect("capture"), &[&track]);
        assert_eq!(result.lat, Some(0.00005));
        assert_eq!(result.lon, Some(0.00005));
        assert_eq!(result.status, MatchStatus::MatchedHigh);
    }

    #[test]
    fn never_interpolates_across_segments() {
        let track = track();
        let photo = photo(Some("2024-01-01T00:00:30Z"));
        let result = match_photo(&photo, photo.capture_utc.expect("capture"), &[&track]);
        assert_eq!(result.status, MatchStatus::SegmentGap);
        assert!(result.lat.is_none());
    }

    #[test]
    fn reports_out_of_range_and_missing_time() {
        let track = track();
        let outside = photo(Some("2023-12-31T23:00:00Z"));
        let outside_match = match_photo(&outside, outside.capture_utc.expect("capture"), &[&track]);
        assert_eq!(outside_match.status, MatchStatus::OutOfRange);

        let missing = photo(None);
        let cancelled = AtomicBool::new(false);
        let matches = calculate_matches(
            &[missing],
            &[track],
            None,
            None,
            &Calibration::default(),
            &cancelled,
            |_, _, _| {},
        )
        .expect("calculate");
        assert_eq!(matches[0].status, MatchStatus::NoCaptureTime);
    }
}
