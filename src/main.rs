use quick_xml::events::Event;
use quick_xml::{Reader, Writer};
use std::error::Error;
use std::io::{self, Read, Write};
use time::{Duration, OffsetDateTime};

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
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
    let mut reader = Reader::from_reader(input);
    let mut writer = Writer::new(io::stdout());
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
