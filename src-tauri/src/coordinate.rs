use crate::domain::{CoordinateSystem, GeoPoint};
use crate::error::{AppError, AppResult, ErrorCode};

const PI: f64 = std::f64::consts::PI;
const EARTH_SEMI_MAJOR: f64 = 6_378_245.0;
const EARTH_ECCENTRICITY_SQUARED: f64 = 0.006_693_421_622_965_943;
const X_PI: f64 = PI * 3000.0 / 180.0;

pub fn validate_point(point: GeoPoint) -> AppResult<()> {
    if !point.lat.is_finite()
        || !point.lon.is_finite()
        || !(-90.0..=90.0).contains(&point.lat)
        || !(-180.0..=180.0).contains(&point.lon)
    {
        return Err(AppError::invalid(format!(
            "无效坐标：lat={}, lon={}",
            point.lat, point.lon
        )));
    }
    Ok(())
}

pub fn convert(
    point: GeoPoint,
    source: CoordinateSystem,
    target: CoordinateSystem,
) -> AppResult<GeoPoint> {
    validate_point(point)?;
    if source == CoordinateSystem::Unknown || target == CoordinateSystem::Unknown {
        return Err(AppError::new(
            ErrorCode::CrsUnconfirmed,
            "轨迹坐标系尚未确认。",
            "请选择 WGS84、GCJ-02 或 BD-09，并检查叠加预览。",
            true,
        ));
    }
    if source == target {
        return Ok(point);
    }

    let wgs84 = match source {
        CoordinateSystem::Wgs84 => point,
        CoordinateSystem::Gcj02 => gcj02_to_wgs84(point),
        CoordinateSystem::Bd09 => gcj02_to_wgs84(bd09_to_gcj02(point)),
        CoordinateSystem::Unknown => unreachable!(),
    };

    let converted = match target {
        CoordinateSystem::Wgs84 => wgs84,
        CoordinateSystem::Gcj02 => wgs84_to_gcj02(wgs84),
        CoordinateSystem::Bd09 => gcj02_to_bd09(wgs84_to_gcj02(wgs84)),
        CoordinateSystem::Unknown => unreachable!(),
    };
    validate_point(converted)?;
    Ok(converted)
}

pub fn wgs84_to_gcj02(point: GeoPoint) -> GeoPoint {
    if outside_china(point) {
        return point;
    }
    let (delta_lat, delta_lon) = gcj_delta(point);
    GeoPoint {
        lat: point.lat + delta_lat,
        lon: point.lon + delta_lon,
    }
}

pub fn gcj02_to_wgs84(point: GeoPoint) -> GeoPoint {
    if outside_china(point) {
        return point;
    }

    // Fixed-iteration inverse keeps matching deterministic while reaching
    // sub-meter precision for normal GCJ-02 coordinates.
    let mut estimate = point;
    for _ in 0..8 {
        let projected = wgs84_to_gcj02(estimate);
        estimate = GeoPoint {
            lat: estimate.lat - (projected.lat - point.lat),
            lon: estimate.lon - (projected.lon - point.lon),
        };
    }
    estimate
}

pub fn gcj02_to_bd09(point: GeoPoint) -> GeoPoint {
    let radius = (point.lon * point.lon + point.lat * point.lat).sqrt()
        + 0.00002 * (point.lat * X_PI).sin();
    let angle = point.lat.atan2(point.lon) + 0.000003 * (point.lon * X_PI).cos();
    GeoPoint {
        lon: radius * angle.cos() + 0.0065,
        lat: radius * angle.sin() + 0.006,
    }
}

pub fn bd09_to_gcj02(point: GeoPoint) -> GeoPoint {
    let x = point.lon - 0.0065;
    let y = point.lat - 0.006;
    let radius = (x * x + y * y).sqrt() - 0.00002 * (y * X_PI).sin();
    let angle = y.atan2(x) - 0.000003 * (x * X_PI).cos();
    GeoPoint {
        lon: radius * angle.cos(),
        lat: radius * angle.sin(),
    }
}

fn gcj_delta(point: GeoPoint) -> (f64, f64) {
    let shifted_lon = point.lon - 105.0;
    let shifted_lat = point.lat - 35.0;
    let mut delta_lat = transform_latitude(shifted_lon, shifted_lat);
    let mut delta_lon = transform_longitude(shifted_lon, shifted_lat);
    let rad_lat = point.lat / 180.0 * PI;
    let magic_sine = rad_lat.sin();
    let magic = 1.0 - EARTH_ECCENTRICITY_SQUARED * magic_sine * magic_sine;
    let sqrt_magic = magic.sqrt();
    delta_lat = (delta_lat * 180.0)
        / ((EARTH_SEMI_MAJOR * (1.0 - EARTH_ECCENTRICITY_SQUARED))
            / (magic * sqrt_magic)
            * PI);
    delta_lon =
        (delta_lon * 180.0) / (EARTH_SEMI_MAJOR / sqrt_magic * rad_lat.cos() * PI);
    (delta_lat, delta_lon)
}

fn transform_latitude(x: f64, y: f64) -> f64 {
    let mut result =
        -100.0 + 2.0 * x + 3.0 * y + 0.2 * y * y + 0.1 * x * y + 0.2 * x.abs().sqrt();
    result += (20.0 * (6.0 * x * PI).sin() + 20.0 * (2.0 * x * PI).sin()) * 2.0 / 3.0;
    result += (20.0 * (y * PI).sin() + 40.0 * (y / 3.0 * PI).sin()) * 2.0 / 3.0;
    result +=
        (160.0 * (y / 12.0 * PI).sin() + 320.0 * (y * PI / 30.0).sin()) * 2.0 / 3.0;
    result
}

fn transform_longitude(x: f64, y: f64) -> f64 {
    let mut result =
        300.0 + x + 2.0 * y + 0.1 * x * x + 0.1 * x * y + 0.1 * x.abs().sqrt();
    result += (20.0 * (6.0 * x * PI).sin() + 20.0 * (2.0 * x * PI).sin()) * 2.0 / 3.0;
    result += (20.0 * (x * PI).sin() + 40.0 * (x / 3.0 * PI).sin()) * 2.0 / 3.0;
    result +=
        (150.0 * (x / 12.0 * PI).sin() + 300.0 * (x / 30.0 * PI).sin()) * 2.0 / 3.0;
    result
}

fn outside_china(point: GeoPoint) -> bool {
    point.lon < 72.004 || point.lon > 137.8347 || point.lat < 0.8293 || point.lat > 55.8271
}

pub fn haversine_meters(first: GeoPoint, second: GeoPoint) -> f64 {
    let earth_radius_meters = 6_371_008.8;
    let lat1 = first.lat.to_radians();
    let lat2 = second.lat.to_radians();
    let delta_lat = (second.lat - first.lat).to_radians();
    let delta_lon = (second.lon - first.lon).to_radians();
    let haversine = (delta_lat / 2.0).sin().powi(2)
        + lat1.cos() * lat2.cos() * (delta_lon / 2.0).sin().powi(2);
    2.0 * earth_radius_meters * haversine.sqrt().asin()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beijing_control_point_round_trips() {
        let wgs = GeoPoint {
            lat: 39.908_823,
            lon: 116.397_470,
        };
        let gcj = wgs84_to_gcj02(wgs);
        assert!((gcj.lat - 39.910_226).abs() < 0.001);
        assert!((gcj.lon - 116.403_714).abs() < 0.001);

        let restored = gcj02_to_wgs84(gcj);
        assert!((restored.lat - wgs.lat).abs() < 0.000_01);
        assert!((restored.lon - wgs.lon).abs() < 0.000_01);
    }

    #[test]
    fn coordinates_outside_china_are_unchanged_in_gcj() {
        let london = GeoPoint {
            lat: 51.5074,
            lon: -0.1278,
        };
        assert_eq!(wgs84_to_gcj02(london), london);
        assert_eq!(gcj02_to_wgs84(london), london);
    }

    #[test]
    fn bd09_round_trip_is_stable() {
        let gcj = GeoPoint {
            lat: 31.2304,
            lon: 121.4737,
        };
        let restored = bd09_to_gcj02(gcj02_to_bd09(gcj));
        assert!((restored.lat - gcj.lat).abs() < 0.000_01);
        assert!((restored.lon - gcj.lon).abs() < 0.000_01);
    }
}

