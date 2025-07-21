use clap::{Parser, Subcommand};
use quick_xml::events::Event;
use quick_xml::{Reader, Writer};
use std::error::Error;
use std::io::{self, Read, Write};
use time::{Duration, OffsetDateTime};

#[derive(Parser)]
#[command(name = "gpxwrench", about = "A CLI tool for processing GPX files")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Trim GPX track points to keep only those within the first 5 seconds")]
    Trim,
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
        Commands::Trim => trim_command(),
    }
}

fn trim_command() -> Result<(), Box<dyn Error>> {
    let stdin = io::stdin();

    let mut input = Vec::new();
    stdin.lock().read_to_end(&mut input)?;

    let min_time = find_minimum_time(&input)?;

    if let Some(min_t) = min_time {
        let threshold = min_t + Duration::seconds(5);
        filter_xml_by_time(&input, threshold)?;
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

fn filter_xml_by_time(input: &[u8], threshold: OffsetDateTime) -> Result<(), Box<dyn Error>> {
    filter_xml_by_time_to_writer(input, threshold, io::stdout())
}

fn filter_xml_by_time_to_writer<W: Write>(
    input: &[u8],
    threshold: OffsetDateTime,
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
                    // Decide whether to include this trkpt based on time
                    let include_point = if let Some(point_time) = trkpt_time {
                        point_time <= threshold
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
        filter_xml_by_time_to_writer(SAMPLE_GPX.as_bytes(), threshold, &mut output).unwrap();

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
}
