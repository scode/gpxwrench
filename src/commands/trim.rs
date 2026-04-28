use crate::gpxxml::{filter_xml_by_time_range, find_minimum_time};
use gpxwrench::{MAX_INPUT_BYTES, TrimRange, parse_range, read_to_end_limited};
use std::error::Error;
use std::io;
use time::OffsetDateTime;

pub fn trim_command(range_str: &str) -> Result<(), Box<dyn Error>> {
    let range = parse_range(range_str)?;

    let stdin = io::stdin();
    let input = read_to_end_limited(stdin.lock(), MAX_INPUT_BYTES)?;

    let min_time = find_minimum_time(&input)?;

    if let Some(min_t) = min_time {
        let (start_threshold, end_threshold) = match range {
            TrimRange::Duration { start, end } => (
                min_t
                    .checked_add(start)
                    .ok_or("Trim start exceeds supported timestamp range")?,
                min_t
                    .checked_add(end)
                    .ok_or("Trim end exceeds supported timestamp range")?,
            ),
            TrimRange::Timestamp { start, end } => (
                min_t
                    .checked_add(start)
                    .ok_or("Trim start exceeds supported timestamp range")?,
                min_t
                    .checked_add(end)
                    .ok_or("Trim end exceeds supported timestamp range")?,
            ),
        };

        filter_xml_by_time_range(&input, start_threshold, end_threshold)?;
    } else {
        filter_xml_by_time_range(
            &input,
            OffsetDateTime::UNIX_EPOCH,
            OffsetDateTime::UNIX_EPOCH,
        )?;
    }

    Ok(())
}
