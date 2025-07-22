use std::error::Error;
use time::{Duration, OffsetDateTime};

#[derive(Debug, Clone)]
pub struct TrackPoint {
    pub lat: f64,
    pub lon: f64,
    pub time: OffsetDateTime,
}

#[derive(Debug)]
pub enum TrimRange {
    Duration { start: Duration, end: Duration },
    Timestamp { start: Duration, end: Duration },
}

pub fn parse_duration(s: &str) -> Result<Duration, Box<dyn Error>> {
    if s.is_empty() {
        return Err("Empty duration".into());
    }

    let (num_str, unit) = s.split_at(s.len() - 1);
    let num: i64 = num_str.parse()?;

    match unit {
        "s" => Ok(Duration::seconds(num)),
        "m" => Ok(Duration::minutes(num)),
        "h" => Ok(Duration::hours(num)),
        _ => Err(format!("Invalid duration unit: {unit}").into()),
    }
}

pub fn parse_timestamp(s: &str) -> Result<Duration, Box<dyn Error>> {
    let parts: Vec<&str> = s.split(':').collect();

    let (hours, minutes, seconds) = match parts.len() {
        2 => (0, parts[0].parse::<i64>()?, parts[1].parse::<i64>()?),
        3 => (
            parts[0].parse::<i64>()?,
            parts[1].parse::<i64>()?,
            parts[2].parse::<i64>()?,
        ),
        _ => return Err("Invalid timestamp format".into()),
    };

    Ok(Duration::hours(hours) + Duration::minutes(minutes) + Duration::seconds(seconds))
}

pub fn parse_range(range_str: &str) -> Result<TrimRange, Box<dyn Error>> {
    let parts: Vec<&str> = range_str.split(',').collect();
    if parts.len() != 2 {
        return Err("Range must have exactly two parts separated by comma".into());
    }

    let start_str = parts[0].trim();
    let end_str = parts[1].trim();

    if start_str.contains(':') || end_str.contains(':') {
        let start = parse_timestamp(start_str)?;
        let end = parse_timestamp(end_str)?;
        Ok(TrimRange::Timestamp { start, end })
    } else {
        let start = parse_duration(start_str)?;
        let end = parse_duration(end_str)?;
        Ok(TrimRange::Duration { start, end })
    }
}

/// Calculates the great circle distance between two GPS coordinates using the haversine formula.
///
/// This is the standard method for calculating distances on a sphere and is appropriate for
/// GPS applications because:
/// 1. It accounts for the Earth's spherical shape (unlike simple Euclidean distance)
/// 2. It's accurate for short to medium distances typical in GPS tracks
/// 3. It's computationally efficient compared to more complex ellipsoid formulations
///
/// For very precise applications over long distances, ellipsoid-based calculations like
/// Vincenty's formula would be more accurate, but for activity tracking the haversine
/// formula provides sufficient precision with much simpler computation.
///
/// References:
/// - R.W. Sinnott, "Virtues of the Haversine", Sky and Telescope, vol. 68, no. 2, 1984, p. 159
/// - https://en.wikipedia.org/wiki/Haversine_formula
/// - https://www.movable-type.co.uk/scripts/latlong.html
pub fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const EARTH_RADIUS: f64 = 6371000.0; // Mean Earth radius in meters (WGS84: 6371008.8m)

    // Convert latitude and longitude differences to radians
    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();
    let delta_lat = (lat2 - lat1).to_radians();
    let delta_lon = (lon2 - lon1).to_radians();

    // Haversine formula: a = sin²(Δφ/2) + cos φ1 ⋅ cos φ2 ⋅ sin²(Δλ/2)
    let a = (delta_lat / 2.0).sin().powi(2)
        + lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);

    // c = 2 ⋅ atan2(√a, √(1−a))
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

    // Distance = R ⋅ c (where R is Earth's radius)
    EARTH_RADIUS * c
}

pub fn calculate_speed(p1: &TrackPoint, p2: &TrackPoint) -> f64 {
    let distance = haversine_distance(p1.lat, p1.lon, p2.lat, p2.lon);
    let time_diff = (p2.time - p1.time).as_seconds_f64();

    if time_diff > 0.0 {
        distance / time_diff
    } else {
        0.0
    }
}

pub fn detect_activity_bounds(
    track_points: &[TrackPoint],
    speed_threshold: f64,
    buffer_seconds: u64,
) -> Result<(OffsetDateTime, OffsetDateTime), Box<dyn Error>> {
    if track_points.len() < 2 {
        return Err("Need at least 2 track points for activity detection".into());
    }

    let mut speeds = Vec::new();
    for i in 1..track_points.len() {
        let speed = calculate_speed(&track_points[i - 1], &track_points[i]);
        speeds.push((i, speed));
    }

    let min_activity_points = 3;
    let mut activity_start_idx = None;
    let mut activity_end_idx = None;

    let mut consecutive_active = 0;
    for (idx, speed) in &speeds {
        if *speed >= speed_threshold {
            consecutive_active += 1;
            if consecutive_active >= min_activity_points && activity_start_idx.is_none() {
                activity_start_idx = Some(*idx - consecutive_active + 1);
            }
        } else {
            consecutive_active = 0;
        }
    }

    consecutive_active = 0;
    for (idx, speed) in speeds.iter().rev() {
        if *speed >= speed_threshold {
            consecutive_active += 1;
            if consecutive_active >= min_activity_points && activity_end_idx.is_none() {
                activity_end_idx = Some(*idx);
            }
        } else {
            consecutive_active = 0;
        }
    }

    let start_idx = activity_start_idx.unwrap_or(0);
    let end_idx = activity_end_idx.unwrap_or(track_points.len() - 1);

    let buffer_duration = Duration::seconds(buffer_seconds as i64);
    let start_time = track_points[start_idx].time - buffer_duration;
    let end_time = track_points[end_idx].time + buffer_duration;

    Ok((start_time, end_time))
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::OffsetDateTime;

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("5s").unwrap(), Duration::seconds(5));
        assert_eq!(parse_duration("10m").unwrap(), Duration::minutes(10));
        assert_eq!(parse_duration("2h").unwrap(), Duration::hours(2));
        assert!(parse_duration("5x").is_err());
        assert!(parse_duration("").is_err());
    }

    #[test]
    fn test_parse_timestamp() {
        assert_eq!(
            parse_timestamp("01:30").unwrap(),
            Duration::minutes(1) + Duration::seconds(30)
        );
        assert_eq!(parse_timestamp("00:05").unwrap(), Duration::seconds(5));
        assert_eq!(
            parse_timestamp("01:02:03").unwrap(),
            Duration::hours(1) + Duration::minutes(2) + Duration::seconds(3)
        );
        assert!(parse_timestamp("1:2:3:4").is_err());
        assert!(parse_timestamp("invalid").is_err());
    }

    #[test]
    fn test_parse_range() {
        let range = parse_range("5s,10s").unwrap();
        match range {
            TrimRange::Duration { start, end } => {
                assert_eq!(start, Duration::seconds(5));
                assert_eq!(end, Duration::seconds(10));
            }
            _ => panic!("Expected Duration variant"),
        }

        let range = parse_range("00:05,01:30").unwrap();
        match range {
            TrimRange::Timestamp { start, end } => {
                assert_eq!(start, Duration::seconds(5));
                assert_eq!(end, Duration::minutes(1) + Duration::seconds(30));
            }
            _ => panic!("Expected Timestamp variant"),
        }

        assert!(parse_range("5s").is_err()); // Missing comma
        assert!(parse_range("5s,10s,15s").is_err()); // Too many parts
    }

    #[test]
    fn test_haversine_distance() {
        // Distance between two points in San Francisco (approximately 1km apart)
        let distance = haversine_distance(37.7749, -122.4194, 37.7849, -122.4094);
        assert!(
            (distance - 1400.0).abs() < 100.0,
            "Expected ~1400m, got {}",
            distance
        );

        // Same point should have 0 distance
        let distance = haversine_distance(37.7749, -122.4194, 37.7749, -122.4194);
        assert!(
            distance < 1.0,
            "Same point should have ~0 distance, got {}",
            distance
        );
    }

    #[test]
    fn test_calculate_speed() {
        let time1 = OffsetDateTime::parse(
            "2023-01-01T10:00:00Z",
            &time::format_description::well_known::Iso8601::DEFAULT,
        )
        .unwrap();
        let time2 = OffsetDateTime::parse(
            "2023-01-01T10:01:00Z", // 60 seconds later
            &time::format_description::well_known::Iso8601::DEFAULT,
        )
        .unwrap();

        let p1 = TrackPoint {
            lat: 37.7749,
            lon: -122.4194,
            time: time1,
        };
        let p2 = TrackPoint {
            lat: 37.7849,
            lon: -122.4094,
            time: time2,
        };

        let speed = calculate_speed(&p1, &p2);
        // Should be around 23 m/s (1400m in 60s)
        assert!(
            speed > 20.0 && speed < 30.0,
            "Expected ~23 m/s, got {}",
            speed
        );
    }
}
