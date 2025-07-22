use crate::gpxxml::{extract_track_points, filter_xml_by_time_range};
use gpxwrench::detect_activity_bounds;
use std::error::Error;
use std::io::{self, Read, Write};

pub fn trim_to_activity_command(speed_threshold: f64, buffer: u64) -> Result<(), Box<dyn Error>> {
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
