//! Useful functions that are used in multiple files in the program

use std::io::Read;

use curl::easy::Easy;
use ratatui::style::Color;
use serde::Serialize;
use simd_json::to_vec;

/// Send a request and return it as a `Vec<u8>`.
pub fn request<J: Serialize + ?Sized>(easy: &mut Easy, json: &J) -> Vec<u8> {
	let mut data = &*to_vec(json).expect("Should be able to serialize POST data");

	let mut vec = Vec::new();

	// Make sure `transfer` is dropped before we use can `vec` again
	{
		let mut transfer = easy.transfer();

		let _ = transfer.read_function(|slice| Ok(data.read(slice).unwrap_or(0)));
		let _ = transfer.write_function(|slice| {
			// Copy the packet to the buffer
			vec.extend_from_slice(slice);
			Ok(slice.len())
		});

		let _ = transfer.perform();
	}

	vec
}

/// Formats a number of seconds in a human-readable format, i.e. "18 hours"
pub fn format_seconds(seconds: i64) -> String {
	// This is needed since expressions can't be used in match conditions
	const MINUTE: i64 = 60;
	const HOUR: i64 = 60 * MINUTE;
	const DAY: i64 = 24 * HOUR;
	const MONTH: i64 = 365 / 12 * DAY;
	const YEAR: i64 = 365 * DAY;

	match seconds {
		..MINUTE => [&seconds.to_string(), " Seconds"].concat(),
		MINUTE..HOUR => [&(seconds / MINUTE).to_string(), " Minutes"].concat(),
		HOUR..DAY => [&(seconds / HOUR).to_string(), " Hours"].concat(),
		DAY..MONTH => [&(seconds / DAY).to_string(), " Days"].concat(),
		MONTH..YEAR => [&(seconds / MONTH).to_string(), " Months"].concat(),
		YEAR.. => [&(seconds / YEAR).to_string(), " Years"].concat(),
	}
}

/// Parses a colour string
pub fn parse_colour(string: &str) -> Color {
	let parsed = i32::from_str_radix(string, 16).expect("Server sent an invalid hex colour");

	Color::Rgb((parsed >> 16) as u8, (parsed >> 8) as u8, parsed as u8)
}
