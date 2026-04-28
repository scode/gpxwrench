use crate::gpxxml::{extract_track_points, filter_xml_by_time_range_inclusive_end};
use gpxwrench::{MAX_INPUT_BYTES, detect_activity_bounds, read_to_end_limited};
use std::error::Error;
use std::io;

pub fn trim_to_activity_command(speed_threshold: f64, buffer: u64) -> Result<(), Box<dyn Error>> {
    if !speed_threshold.is_finite() || speed_threshold < 0.0 {
        return Err("Speed threshold must be a finite non-negative number".into());
    }

    let stdin = io::stdin();
    let input = read_to_end_limited(stdin.lock(), MAX_INPUT_BYTES)?;

    let track_points = extract_track_points(&input)?;

    let (start_time, end_time) = detect_activity_bounds(&track_points, speed_threshold, buffer)?;

    filter_xml_by_time_range_inclusive_end(&input, start_time, end_time)?;
    Ok(())
}
