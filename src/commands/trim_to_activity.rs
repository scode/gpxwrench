use crate::gpxxml::{extract_track_points, filter_xml_by_time_range_inclusive_end};
use gpxwrench::detect_activity_bounds;
use std::error::Error;
use std::io::{self, Read};

pub fn trim_to_activity_command(speed_threshold: f64, buffer: u64) -> Result<(), Box<dyn Error>> {
    let stdin = io::stdin();
    let mut input = Vec::new();
    stdin.lock().read_to_end(&mut input)?;

    let track_points = extract_track_points(&input)?;

    let (start_time, end_time) = detect_activity_bounds(&track_points, speed_threshold, buffer)?;

    filter_xml_by_time_range_inclusive_end(&input, start_time, end_time)?;
    Ok(())
}
