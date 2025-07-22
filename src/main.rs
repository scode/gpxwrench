use clap::{Parser, Subcommand};
use quick_xml::events::Event;
use quick_xml::{Reader, Writer};
use std::error::Error;
use std::io::{self, Read, Write};
use time::{Duration, OffsetDateTime};

#[derive(Debug, Clone)]
struct TrackPoint {
    lat: f64,
    lon: f64,
    time: OffsetDateTime,
}

#[derive(Debug)]
enum TrimRange {
    Duration { start: Duration, end: Duration },
    Timestamp { start: Duration, end: Duration },
}

fn parse_duration(s: &str) -> Result<Duration, Box<dyn Error>> {
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

fn parse_timestamp(s: &str) -> Result<Duration, Box<dyn Error>> {
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

fn parse_range(range_str: &str) -> Result<TrimRange, Box<dyn Error>> {
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

#[derive(Parser)]
#[command(name = "gpxwrench", about = "A CLI tool for processing GPX files")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Trim GPX track points using duration or timestamp ranges")]
    Trim {
        #[arg(help = "Range specification: DUR1,DUR2 (e.g. 5s,10s) or TS1,TS2 (e.g. 00:05,01:30)")]
        range: String,
    },
    #[command(about = "Trim GPX to detected activity period based on speed analysis")]
    TrimToActivity {
        #[arg(
            long,
            short,
            default_value = "1.0",
            help = "Minimum speed (m/s) to consider as activity"
        )]
        speed_threshold: f64,
        #[arg(
            long,
            short,
            default_value = "30",
            help = "Buffer time (seconds) to add before/after detected activity"
        )]
        buffer: u64,
    },
}

fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    match cli.command {
        Commands::Trim { range } => trim_command(&range),
        Commands::TrimToActivity {
            speed_threshold,
            buffer,
        } => trim_to_activity_command(speed_threshold, buffer),
    }
}

fn trim_command(range_str: &str) -> Result<(), Box<dyn Error>> {
    let range = parse_range(range_str)?;

    let stdin = io::stdin();
    let mut input = Vec::new();
    stdin.lock().read_to_end(&mut input)?;

    let min_time = find_minimum_time(&input)?;

    if let Some(min_t) = min_time {
        let (start_threshold, end_threshold) = match range {
            TrimRange::Duration { start, end } => (min_t + start, min_t + end),
            TrimRange::Timestamp { start, end } => (min_t + start, min_t + end),
        };

        filter_xml_by_time_range(&input, start_threshold, end_threshold)?;
    } else {
        io::stdout().write_all(&input)?;
    }

    Ok(())
}

fn find_minimum_time(input: &[u8]) -> Result<Option<OffsetDateTime>, Box<dyn Error>> {
    let mut reader = Reader::from_reader(input);
    let mut buf = Vec::new();
    let mut min_time: Option<OffsetDateTime> = None;

    let mut in_trkpt = false;
    let mut in_time_element = false;
    let mut time_text = String::new();

    loop {
        let event = match reader.read_event_into(&mut buf) {
            Err(e) => {
                return Err(
                    format!("Error at position {}: {:?}", reader.buffer_position(), e).into(),
                );
            }
            Ok(Event::Eof) => break,
            Ok(event) => event.into_owned(),
        };

        match event {
            Event::Start(ref e) => {
                if e.name().as_ref() == b"trkpt" {
                    in_trkpt = true;
                } else if in_trkpt && e.name().as_ref() == b"time" {
                    in_time_element = true;
                    time_text.clear();
                }
            }

            Event::End(ref e) => {
                if e.name().as_ref() == b"trkpt" {
                    in_trkpt = false;
                } else if e.name().as_ref() == b"time" && in_trkpt {
                    in_time_element = false;
                    // Parse the collected time text
                    if let Ok(parsed_time) = OffsetDateTime::parse(
                        &time_text,
                        &time::format_description::well_known::Iso8601::DEFAULT,
                    ) {
                        if min_time.is_none() || parsed_time < min_time.unwrap() {
                            min_time = Some(parsed_time);
                        }
                    }
                }
            }

            Event::Text(ref e) => {
                if in_trkpt && in_time_element {
                    time_text.push_str(&e.unescape().unwrap_or_default());
                }
            }

            _ => {}
        }

        buf.clear();
    }

    Ok(min_time)
}

fn filter_xml_by_time_range(
    input: &[u8],
    start_threshold: OffsetDateTime,
    end_threshold: OffsetDateTime,
) -> Result<(), Box<dyn Error>> {
    filter_xml_by_time_to_writer(input, start_threshold, Some(end_threshold), io::stdout())
}

fn filter_xml_by_time_to_writer<W: Write>(
    input: &[u8],
    start_threshold: OffsetDateTime,
    end_threshold: Option<OffsetDateTime>,
    output: W,
) -> Result<(), Box<dyn Error>> {
    let mut reader = Reader::from_reader(input);
    let mut writer = Writer::new(output);
    let mut buf = Vec::new();

    let mut in_trkpt = false;
    let mut trkpt_buffer = Vec::new();
    let mut trkpt_time: Option<OffsetDateTime> = None;
    let mut in_time_element = false;
    let mut time_text = String::new();
    let mut in_trkseg = false;
    let mut just_filtered_trkpt = false;

    loop {
        let event = match reader.read_event_into(&mut buf) {
            Err(e) => {
                return Err(
                    format!("Error at position {}: {:?}", reader.buffer_position(), e).into(),
                );
            }
            Ok(Event::Eof) => break,
            Ok(event) => event.into_owned(),
        };

        match event {
            Event::Start(ref e) => {
                if e.name().as_ref() == b"trkseg" {
                    in_trkseg = true;
                    just_filtered_trkpt = false;
                } else if e.name().as_ref() == b"trkpt" {
                    in_trkpt = true;
                    trkpt_buffer.clear();
                    trkpt_time = None;
                    time_text.clear();
                    just_filtered_trkpt = false;
                }

                if in_trkpt {
                    if e.name().as_ref() == b"time" {
                        in_time_element = true;
                        time_text.clear();
                    }
                    trkpt_buffer.push(event.clone());
                } else {
                    writer.write_event(&event)?;
                }
            }

            Event::End(ref e) => {
                if e.name().as_ref() == b"trkseg" {
                    in_trkseg = false;
                    just_filtered_trkpt = false;
                    writer.write_event(&event)?;
                } else if e.name().as_ref() == b"trkpt" {
                    // Decide whether to include this trkpt based on time range
                    let include_point = if let Some(point_time) = trkpt_time {
                        if let Some(end_thresh) = end_threshold {
                            point_time >= start_threshold && point_time < end_thresh
                        } else {
                            point_time <= start_threshold
                        }
                    } else {
                        false // Exclude points without time
                    };

                    if include_point {
                        // Write all buffered events for this trkpt
                        for buffered_event in &trkpt_buffer {
                            writer.write_event(buffered_event)?;
                        }
                        writer.write_event(&event)?;
                        just_filtered_trkpt = false;
                    } else {
                        just_filtered_trkpt = true;
                    }

                    in_trkpt = false;
                    trkpt_buffer.clear();
                } else if in_trkpt {
                    if e.name().as_ref() == b"time" {
                        in_time_element = false;
                        // Parse the collected time text
                        if let Ok(parsed_time) = OffsetDateTime::parse(
                            &time_text,
                            &time::format_description::well_known::Iso8601::DEFAULT,
                        ) {
                            trkpt_time = Some(parsed_time);
                        }
                    }
                    trkpt_buffer.push(event.clone());
                } else {
                    writer.write_event(&event)?;
                }
            }

            Event::Text(ref e) => {
                if in_trkpt {
                    if in_time_element {
                        time_text.push_str(&e.unescape().unwrap_or_default());
                    }
                    trkpt_buffer.push(event.clone());
                } else {
                    // Skip whitespace-only text nodes after filtered track points within track segments
                    let is_whitespace_only = e.iter().all(|&b| b.is_ascii_whitespace());
                    if in_trkseg && just_filtered_trkpt && is_whitespace_only {
                        // Skip this whitespace text node
                    } else {
                        writer.write_event(&event)?;
                        if !is_whitespace_only {
                            just_filtered_trkpt = false;
                        }
                    }
                }
            }

            event => {
                if in_trkpt {
                    trkpt_buffer.push(event.clone());
                } else {
                    writer.write_event(&event)?;
                }
            }
        }

        buf.clear();
    }

    Ok(())
}

fn trim_to_activity_command(speed_threshold: f64, buffer: u64) -> Result<(), Box<dyn Error>> {
    let stdin = io::stdin();
    let mut input = Vec::new();
    stdin.lock().read_to_end(&mut input)?;

    let track_points = extract_track_points(&input)?;

    if track_points.is_empty() {
        io::stdout().write_all(&input)?;
        return Ok(());
    }

    let (start_time, end_time) = detect_activity_bounds(&track_points, speed_threshold, buffer)?;

    filter_xml_by_time_range(&input, start_time, end_time)?;
    Ok(())
}

fn extract_track_points(input: &[u8]) -> Result<Vec<TrackPoint>, Box<dyn Error>> {
    let mut reader = Reader::from_reader(input);
    let mut buf = Vec::new();
    let mut track_points = Vec::new();

    let mut in_trkpt = false;
    let mut current_lat: Option<f64> = None;
    let mut current_lon: Option<f64> = None;
    let mut current_time: Option<OffsetDateTime> = None;
    let mut in_time_element = false;
    let mut time_text = String::new();

    loop {
        let event = match reader.read_event_into(&mut buf) {
            Err(e) => {
                return Err(
                    format!("Error at position {}: {:?}", reader.buffer_position(), e).into(),
                );
            }
            Ok(Event::Eof) => break,
            Ok(event) => event.into_owned(),
        };

        match event {
            Event::Start(ref e) => {
                if e.name().as_ref() == b"trkpt" {
                    in_trkpt = true;
                    current_lat = None;
                    current_lon = None;
                    current_time = None;

                    for attr in e.attributes().flatten() {
                        match attr.key.as_ref() {
                            b"lat" => {
                                if let Ok(lat_str) = std::str::from_utf8(&attr.value) {
                                    current_lat = lat_str.parse().ok();
                                }
                            }
                            b"lon" => {
                                if let Ok(lon_str) = std::str::from_utf8(&attr.value) {
                                    current_lon = lon_str.parse().ok();
                                }
                            }
                            _ => {}
                        }
                    }
                } else if in_trkpt && e.name().as_ref() == b"time" {
                    in_time_element = true;
                    time_text.clear();
                }
            }

            Event::End(ref e) => {
                if e.name().as_ref() == b"trkpt" {
                    if let (Some(lat), Some(lon), Some(time)) =
                        (current_lat, current_lon, current_time)
                    {
                        track_points.push(TrackPoint { lat, lon, time });
                    }
                    in_trkpt = false;
                } else if e.name().as_ref() == b"time" && in_trkpt {
                    in_time_element = false;
                    if let Ok(parsed_time) = OffsetDateTime::parse(
                        &time_text,
                        &time::format_description::well_known::Iso8601::DEFAULT,
                    ) {
                        current_time = Some(parsed_time);
                    }
                }
            }

            Event::Text(ref e) => {
                if in_trkpt && in_time_element {
                    time_text.push_str(&e.unescape().unwrap_or_default());
                }
            }

            _ => {}
        }

        buf.clear();
    }

    Ok(track_points)
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
fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
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

fn calculate_speed(p1: &TrackPoint, p2: &TrackPoint) -> f64 {
    let distance = haversine_distance(p1.lat, p1.lon, p2.lat, p2.lon);
    let time_diff = (p2.time - p1.time).as_seconds_f64();

    if time_diff > 0.0 {
        distance / time_diff
    } else {
        0.0
    }
}

fn detect_activity_bounds(
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

    const SAMPLE_GPX: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="test">
  <trk>
    <name>Test Track</name>
    <trkseg>
      <trkpt lat="37.7749" lon="-122.4194">
        <ele>100</ele>
        <time>2023-01-01T10:00:00Z</time>
        <extensions>
          <ns3:TrackPointExtension xmlns:ns3="http://www.garmin.com/xmlschemas/TrackPointExtension/v1">
            <ns3:hr>150</ns3:hr>
          </ns3:TrackPointExtension>
        </extensions>
      </trkpt>
      <trkpt lat="37.7750" lon="-122.4195">
        <ele>101</ele>
        <time>2023-01-01T10:00:02Z</time>
        <extensions>
          <ns3:TrackPointExtension xmlns:ns3="http://www.garmin.com/xmlschemas/TrackPointExtension/v1">
            <ns3:hr>155</ns3:hr>
          </ns3:TrackPointExtension>
        </extensions>
      </trkpt>
      <trkpt lat="37.7751" lon="-122.4196">
        <ele>102</ele>
        <time>2023-01-01T10:00:10Z</time>
        <extensions>
          <ns3:TrackPointExtension xmlns:ns3="http://www.garmin.com/xmlschemas/TrackPointExtension/v1">
            <ns3:hr>160</ns3:hr>
          </ns3:TrackPointExtension>
        </extensions>
      </trkpt>
    </trkseg>
  </trk>
</gpx>"#;

    /// Tests that find_minimum_time correctly identifies the earliest timestamp from GPX track points.
    #[test]
    fn test_find_minimum_time_with_valid_data() {
        let result = find_minimum_time(SAMPLE_GPX.as_bytes()).unwrap();
        assert!(result.is_some());

        let min_time = result.unwrap();
        let expected = OffsetDateTime::parse(
            "2023-01-01T10:00:00Z",
            &time::format_description::well_known::Iso8601::DEFAULT,
        )
        .unwrap();
        assert_eq!(min_time, expected);
    }

    /// Tests that find_minimum_time returns None when GPX contains track points without time elements.
    #[test]
    fn test_find_minimum_time_with_no_time_elements() {
        let gpx_no_time = r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="test">
  <trk>
    <trkseg>
      <trkpt lat="37.7749" lon="-122.4194">
        <ele>100</ele>
      </trkpt>
    </trkseg>
  </trk>
</gpx>"#;

        let result = find_minimum_time(gpx_no_time.as_bytes()).unwrap();
        assert!(result.is_none());
    }

    /// Tests that find_minimum_time handles completely empty input gracefully.
    #[test]
    fn test_find_minimum_time_with_empty_input() {
        let result = find_minimum_time(b"").unwrap();
        assert!(result.is_none());
    }

    /// Tests that find_minimum_time ignores malformed timestamps and finds valid ones.
    #[test]
    fn test_find_minimum_time_with_malformed_time() {
        let gpx_bad_time = r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="test">
  <trk>
    <trkseg>
      <trkpt lat="37.7749" lon="-122.4194">
        <time>invalid-time</time>
      </trkpt>
      <trkpt lat="37.7750" lon="-122.4195">
        <time>2023-01-01T10:00:00Z</time>
      </trkpt>
    </trkseg>
  </trk>
</gpx>"#;

        let result = find_minimum_time(gpx_bad_time.as_bytes()).unwrap();
        assert!(result.is_some());
        let expected = OffsetDateTime::parse(
            "2023-01-01T10:00:00Z",
            &time::format_description::well_known::Iso8601::DEFAULT,
        )
        .unwrap();
        assert_eq!(result.unwrap(), expected);
    }

    /// Tests that filter_xml_by_time produces valid GPX output that can be parsed by the GPX crate.
    #[test]
    fn test_filter_xml_by_time_validates_with_gpx_crate() {
        use gpx::{Gpx, read};

        let threshold = OffsetDateTime::parse(
            "2023-01-01T10:00:05Z",
            &time::format_description::well_known::Iso8601::DEFAULT,
        )
        .unwrap();

        let mut output = Vec::new();
        filter_xml_by_time_to_writer(SAMPLE_GPX.as_bytes(), threshold, None, &mut output).unwrap();

        // Verify the output parses correctly with GPX crate
        let gpx_result: Result<Gpx, _> = read(output.as_slice());
        assert!(gpx_result.is_ok());

        let gpx = gpx_result.unwrap();
        assert_eq!(gpx.tracks.len(), 1);
        assert_eq!(gpx.tracks[0].segments.len(), 1);

        // Should have 2 points (first two within threshold)
        let points = &gpx.tracks[0].segments[0].points;
        assert_eq!(points.len(), 2);

        // Verify the times are correct
        assert!(points[0].time.is_some());
        assert!(points[1].time.is_some());
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

    #[test]
    fn test_extract_track_points() {
        let sample_gpx_with_movement = r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="test">
  <trk>
    <trkseg>
      <trkpt lat="37.7749" lon="-122.4194">
        <time>2023-01-01T10:00:00Z</time>
      </trkpt>
      <trkpt lat="37.7750" lon="-122.4195">
        <time>2023-01-01T10:00:05Z</time>
      </trkpt>
      <trkpt lat="37.7760" lon="-122.4180">
        <time>2023-01-01T10:00:10Z</time>
      </trkpt>
    </trkseg>
  </trk>
</gpx>"#;

        let track_points = extract_track_points(sample_gpx_with_movement.as_bytes()).unwrap();
        assert_eq!(track_points.len(), 3);

        assert_eq!(track_points[0].lat, 37.7749);
        assert_eq!(track_points[0].lon, -122.4194);
        assert_eq!(track_points[1].lat, 37.7750);
        assert_eq!(track_points[1].lon, -122.4195);
    }

    #[test]
    fn test_filter_xml_by_time_range() {
        use gpx::{Gpx, read};

        let start_threshold = OffsetDateTime::parse(
            "2023-01-01T10:00:00Z",
            &time::format_description::well_known::Iso8601::DEFAULT,
        )
        .unwrap();
        let end_threshold = OffsetDateTime::parse(
            "2023-01-01T10:00:03Z",
            &time::format_description::well_known::Iso8601::DEFAULT,
        )
        .unwrap();

        let mut output = Vec::new();
        filter_xml_by_time_to_writer(
            SAMPLE_GPX.as_bytes(),
            start_threshold,
            Some(end_threshold),
            &mut output,
        )
        .unwrap();

        // Verify the output parses correctly with GPX crate
        let gpx_result: Result<Gpx, _> = read(output.as_slice());
        assert!(gpx_result.is_ok());

        let gpx = gpx_result.unwrap();
        assert_eq!(gpx.tracks.len(), 1);
        assert_eq!(gpx.tracks[0].segments.len(), 1);

        // Should have 2 points (first two within range [10:00:00, 10:00:03))
        let points = &gpx.tracks[0].segments[0].points;
        assert_eq!(points.len(), 2);

        // Verify the times are correct
        assert!(points[0].time.is_some());
        assert!(points[1].time.is_some());
    }
}
