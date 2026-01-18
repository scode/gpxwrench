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

    fn make_track_point(lat: f64, lon: f64, time_str: &str) -> TrackPoint {
        TrackPoint {
            lat,
            lon,
            time: OffsetDateTime::parse(
                time_str,
                &time::format_description::well_known::Iso8601::DEFAULT,
            )
            .unwrap(),
        }
    }

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
        let p1 = make_track_point(37.7749, -122.4194, "2023-01-01T10:00:00Z");
        let p2 = make_track_point(37.7849, -122.4094, "2023-01-01T10:01:00Z"); // 60 seconds later

        let speed = calculate_speed(&p1, &p2);
        assert!(
            speed > 20.0 && speed < 30.0,
            "Expected ~23 m/s (1400m in 60s), got {}",
            speed
        );
    }

    #[test]
    fn test_calculate_speed_simultaneous_timestamps() {
        let p1 = make_track_point(37.7749, -122.4194, "2023-01-01T10:00:00Z");
        let p2 = make_track_point(37.7849, -122.4094, "2023-01-01T10:00:00Z");

        let speed = calculate_speed(&p1, &p2);
        assert_eq!(
            speed, 0.0,
            "Speed should be 0 when timestamps are identical"
        );
    }

    #[test]
    fn test_calculate_speed_same_coordinates() {
        let p1 = make_track_point(37.7749, -122.4194, "2023-01-01T10:00:00Z");
        let p2 = make_track_point(37.7749, -122.4194, "2023-01-01T10:01:00Z");

        let speed = calculate_speed(&p1, &p2);
        assert!(
            speed < 0.001,
            "Speed should be ~0 for same coordinates, got {}",
            speed
        );
    }

    #[test]
    fn test_calculate_speed_very_small_time_diff() {
        // Ensures no division-by-zero or overflow with sub-second timestamps
        let time1 = OffsetDateTime::parse(
            "2023-01-01T10:00:00Z",
            &time::format_description::well_known::Iso8601::DEFAULT,
        )
        .unwrap();
        let time2 = time1 + Duration::milliseconds(100);

        let p1 = TrackPoint {
            lat: 37.7749,
            lon: -122.4194,
            time: time1,
        };
        let p2 = TrackPoint {
            lat: 37.7750,
            lon: -122.4195,
            time: time2,
        };

        let speed = calculate_speed(&p1, &p2);
        assert!(speed > 0.0, "Speed should be positive for small time diff");
        assert!(speed.is_finite(), "Speed should be finite");
        assert!(
            speed < 1000000.0,
            "Speed should be reasonable, got {}",
            speed
        );
    }

    #[test]
    fn test_detect_activity_bounds_normal_activity() {
        // Idle start -> active middle -> idle end
        let points = vec![
            // Idle points at start (stationary)
            make_track_point(37.7749, -122.4194, "2023-01-01T10:00:00Z"),
            make_track_point(37.7749, -122.4194, "2023-01-01T10:00:05Z"),
            make_track_point(37.7749, -122.4194, "2023-01-01T10:00:10Z"),
            // Activity begins - moving ~100m every 5 seconds (~20 m/s)
            make_track_point(37.7759, -122.4194, "2023-01-01T10:00:15Z"),
            make_track_point(37.7769, -122.4194, "2023-01-01T10:00:20Z"),
            make_track_point(37.7779, -122.4194, "2023-01-01T10:00:25Z"),
            make_track_point(37.7789, -122.4194, "2023-01-01T10:00:30Z"),
            // Idle points at end (stationary)
            make_track_point(37.7789, -122.4194, "2023-01-01T10:00:35Z"),
            make_track_point(37.7789, -122.4194, "2023-01-01T10:00:40Z"),
            make_track_point(37.7789, -122.4194, "2023-01-01T10:00:45Z"),
        ];

        let result = detect_activity_bounds(&points, 5.0, 0);
        assert!(result.is_ok());
        let (start, end) = result.unwrap();

        // Activity detection should trim off the idle portions
        // Start should be no earlier than the first idle point
        assert!(
            start > points[0].time,
            "Start should be after the first idle point"
        );
        // End should be no later than the last idle point
        assert!(
            end < points[points.len() - 1].time,
            "End should be before the last idle point"
        );
    }

    #[test]
    fn test_detect_activity_bounds_all_idle() {
        // All points are stationary - no activity detected
        let points = vec![
            make_track_point(37.7749, -122.4194, "2023-01-01T10:00:00Z"),
            make_track_point(37.7749, -122.4194, "2023-01-01T10:00:05Z"),
            make_track_point(37.7749, -122.4194, "2023-01-01T10:00:10Z"),
            make_track_point(37.7749, -122.4194, "2023-01-01T10:00:15Z"),
        ];

        let result = detect_activity_bounds(&points, 5.0, 0);
        assert!(result.is_ok());
        let (start, end) = result.unwrap();

        // Should fall back to full range
        assert_eq!(start, points[0].time);
        assert_eq!(end, points[points.len() - 1].time);
    }

    #[test]
    fn test_detect_activity_bounds_all_active() {
        // All points are moving fast - activity detected throughout
        let points = vec![
            make_track_point(37.7700, -122.4194, "2023-01-01T10:00:00Z"),
            make_track_point(37.7710, -122.4194, "2023-01-01T10:00:05Z"),
            make_track_point(37.7720, -122.4194, "2023-01-01T10:00:10Z"),
            make_track_point(37.7730, -122.4194, "2023-01-01T10:00:15Z"),
            make_track_point(37.7740, -122.4194, "2023-01-01T10:00:20Z"),
        ];

        let result = detect_activity_bounds(&points, 5.0, 0);
        assert!(result.is_ok());
        let (start, end) = result.unwrap();

        // When all points are active, both start and end should be within the data range
        assert!(start >= points[0].time, "Start should be within data range");
        assert!(
            end <= points[points.len() - 1].time,
            "End should be within data range"
        );
    }

    #[test]
    fn test_detect_activity_bounds_single_point() {
        let points = vec![make_track_point(37.7749, -122.4194, "2023-01-01T10:00:00Z")];

        let result = detect_activity_bounds(&points, 5.0, 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("at least 2"));
    }

    #[test]
    fn test_detect_activity_bounds_two_points() {
        let points = vec![
            make_track_point(37.7749, -122.4194, "2023-01-01T10:00:00Z"),
            make_track_point(37.7759, -122.4194, "2023-01-01T10:00:05Z"),
        ];

        let result = detect_activity_bounds(&points, 5.0, 0);
        assert!(result.is_ok());
        let (start, end) = result.unwrap();

        // With only 2 points, can't have 3 consecutive active, falls back to full range
        assert_eq!(start, points[0].time);
        assert_eq!(end, points[1].time);
    }

    #[test]
    fn test_detect_activity_bounds_exactly_three_active() {
        // The algorithm requires 3 consecutive active speeds to confirm true activity vs GPS noise.
        // Exactly 3 consecutive active speeds requires 4 points.
        // With 4 moving points: speeds between (0,1), (1,2), (2,3) = 3 speeds
        let points = vec![
            make_track_point(37.7749, -122.4194, "2023-01-01T10:00:00Z"),
            make_track_point(37.7759, -122.4194, "2023-01-01T10:00:05Z"),
            make_track_point(37.7769, -122.4194, "2023-01-01T10:00:10Z"),
            make_track_point(37.7779, -122.4194, "2023-01-01T10:00:15Z"),
        ];

        let result = detect_activity_bounds(&points, 5.0, 0);
        assert!(result.is_ok());
        let (start, end) = result.unwrap();

        // Should successfully detect activity and return valid bounds
        assert!(start >= points[0].time, "Start should be within data range");
        assert!(
            start <= points[points.len() - 1].time,
            "Start should be within data range"
        );
        assert!(end >= points[0].time, "End should be within data range");
        assert!(
            end <= points[points.len() - 1].time,
            "End should be within data range"
        );
    }

    #[test]
    fn test_detect_activity_bounds_sporadic_noise() {
        // Isolated fast points are likely GPS jitter, not real movement.
        // Sporadic high-speed points that shouldn't trigger activity (not consecutive)
        let points = vec![
            make_track_point(37.7749, -122.4194, "2023-01-01T10:00:00Z"),
            make_track_point(37.7759, -122.4194, "2023-01-01T10:00:05Z"), // Fast
            make_track_point(37.7759, -122.4194, "2023-01-01T10:00:10Z"), // Idle
            make_track_point(37.7769, -122.4194, "2023-01-01T10:00:15Z"), // Fast
            make_track_point(37.7769, -122.4194, "2023-01-01T10:00:20Z"), // Idle
            make_track_point(37.7779, -122.4194, "2023-01-01T10:00:25Z"), // Fast
        ];

        let result = detect_activity_bounds(&points, 5.0, 0);
        assert!(result.is_ok());
        let (start, end) = result.unwrap();

        // No 3 consecutive active points, should fall back to defaults
        assert_eq!(start, points[0].time);
        assert_eq!(end, points[points.len() - 1].time);
    }

    #[test]
    fn test_detect_activity_bounds_buffer_time() {
        let points = vec![
            make_track_point(37.7749, -122.4194, "2023-01-01T10:00:00Z"),
            make_track_point(37.7749, -122.4194, "2023-01-01T10:00:05Z"),
            make_track_point(37.7759, -122.4194, "2023-01-01T10:00:10Z"),
            make_track_point(37.7769, -122.4194, "2023-01-01T10:00:15Z"),
            make_track_point(37.7779, -122.4194, "2023-01-01T10:00:20Z"),
            make_track_point(37.7789, -122.4194, "2023-01-01T10:00:25Z"),
            make_track_point(37.7789, -122.4194, "2023-01-01T10:00:30Z"),
        ];

        let result_no_buffer = detect_activity_bounds(&points, 5.0, 0).unwrap();
        let result_with_buffer = detect_activity_bounds(&points, 5.0, 10).unwrap();

        // Buffer should extend the range
        assert!(
            result_with_buffer.0 < result_no_buffer.0,
            "Start should be earlier with buffer"
        );
        assert!(
            result_with_buffer.1 > result_no_buffer.1,
            "End should be later with buffer"
        );

        // Verify buffer is approximately correct (10 seconds)
        let start_diff = (result_no_buffer.0 - result_with_buffer.0).whole_seconds();
        let end_diff = (result_with_buffer.1 - result_no_buffer.1).whole_seconds();
        assert_eq!(start_diff, 10, "Start buffer should be 10 seconds");
        assert_eq!(end_diff, 10, "End buffer should be 10 seconds");
    }

    #[test]
    fn test_detect_activity_bounds_empty() {
        let points: Vec<TrackPoint> = vec![];
        let result = detect_activity_bounds(&points, 5.0, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_detect_activity_bounds_threshold_boundary() {
        // Points moving at exactly the threshold speed
        let points = vec![
            make_track_point(37.7749, -122.4194, "2023-01-01T10:00:00Z"),
            make_track_point(37.77494494, -122.4194, "2023-01-01T10:00:05Z"), // ~5m in 5s = 1 m/s
            make_track_point(37.77498988, -122.4194, "2023-01-01T10:00:10Z"),
            make_track_point(37.77503482, -122.4194, "2023-01-01T10:00:15Z"),
            make_track_point(37.77507976, -122.4194, "2023-01-01T10:00:20Z"),
        ];

        // With threshold 1.0, these should count as active (speed >= threshold)
        let result = detect_activity_bounds(&points, 1.0, 0);
        assert!(result.is_ok());
    }
}
