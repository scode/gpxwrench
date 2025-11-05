use gpxwrench::TrackPoint;
use quick_xml::events::Event;
use quick_xml::{Reader, Writer};
use std::error::Error;
use std::io::Write;
use time::OffsetDateTime;

pub fn find_minimum_time(input: &[u8]) -> Result<Option<OffsetDateTime>, Box<dyn Error>> {
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
                    match OffsetDateTime::parse(
                        &time_text,
                        &time::format_description::well_known::Iso8601::DEFAULT,
                    ) {
                        Ok(parsed_time) if min_time.is_none_or(|t| parsed_time < t) => {
                            min_time = Some(parsed_time);
                        }
                        _ => {}
                    }
                }
            }

            Event::Text(ref e) => {
                if in_trkpt && in_time_element
                    && let Ok(text) = std::str::from_utf8(e) {
                        time_text.push_str(text);
                    }
            }

            _ => {}
        }

        buf.clear();
    }

    Ok(min_time)
}

pub fn filter_xml_by_time_range(
    input: &[u8],
    start_threshold: OffsetDateTime,
    end_threshold: OffsetDateTime,
) -> Result<(), Box<dyn Error>> {
    filter_xml_by_time_to_writer(
        input,
        start_threshold,
        Some(end_threshold),
        std::io::stdout(),
    )
}

pub fn filter_xml_by_time_to_writer<W: Write>(
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
                    writer.write_event(event.clone())?;
                }
            }

            Event::End(ref e) => {
                if e.name().as_ref() == b"trkseg" {
                    in_trkseg = false;
                    just_filtered_trkpt = false;
                    writer.write_event(event.clone())?;
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
                            writer.write_event(buffered_event.clone())?;
                        }
                        writer.write_event(event.clone())?;
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
                    writer.write_event(event.clone())?;
                }
            }

            Event::Text(ref e) => {
                if in_trkpt {
                    if in_time_element
                        && let Ok(text) = std::str::from_utf8(e) {
                            time_text.push_str(text);
                        }
                    trkpt_buffer.push(event.clone());
                } else {
                    // Skip whitespace-only text nodes after filtered track points within track segments
                    let is_whitespace_only = e.iter().all(|&b| b.is_ascii_whitespace());
                    if in_trkseg && just_filtered_trkpt && is_whitespace_only {
                        // Skip this whitespace text node
                    } else {
                        writer.write_event(event.clone())?;
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
                    writer.write_event(event)?;
                }
            }
        }

        buf.clear();
    }

    Ok(())
}

pub fn extract_track_points(input: &[u8]) -> Result<Vec<TrackPoint>, Box<dyn Error>> {
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
                if in_trkpt && in_time_element
                    && let Ok(text) = std::str::from_utf8(e) {
                        time_text.push_str(text);
                    }
            }

            _ => {}
        }

        buf.clear();
    }

    Ok(track_points)
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
}
