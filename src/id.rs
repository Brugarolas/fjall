use chrono::{Datelike, Timelike};
use rand::Rng;

const BASE_36_RADIX: u32 = 36;

fn to_base36(mut x: u32) -> String {
    let mut result = vec![];

    loop {
        let m = x % BASE_36_RADIX;
        x /= BASE_36_RADIX;

        result.push(std::char::from_digit(m, BASE_36_RADIX).unwrap());
        if x == 0 {
            break;
        }
    }

    result.into_iter().rev().collect()
}

/// Generate a ID for a segment
pub fn generate_segment_id() -> String {
    // Like https://cassandra.apache.org/_/blog/Apache-Cassandra-4.1-New-SSTable-Identifiers.html
    let now = chrono::Utc::now();

    let month = now.month();
    let day = now.day();

    let hour = now.hour();
    let min = now.minute();

    let nano = now.timestamp_subsec_nanos();

    let mut rng = rand::thread_rng();
    let random = rng.gen::<u32>();

    format!(
        "{}{}_{}{}_{}_{}",
        to_base36(month),
        to_base36(day),
        to_base36(hour),
        to_base36(min),
        to_base36(nano),
        to_base36(random),
    )
}