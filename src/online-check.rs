//! Check if the FOLLOWING channels are online, returning a space separated list of online
//! channels. Fails silently if it can't parse the server's response.

#![allow(non_snake_case)]
#![allow(dead_code)]

use std::io::Read;

use curl::easy::{Easy, List};
use serde::{Deserialize, Serialize};
use simd_json::{from_slice, to_vec};

/// The channels to check
const FOLLOWING: &[&str] = &["keshaeuw", "swaggersouls", "tubbo", "xqc", "rtgame"];

#[derive(Serialize)]
struct Variables {
	channelLogin: &'static str,
	isLive: bool,
	isVod: bool,
	videoID: &'static str,
}

#[derive(Serialize)]
struct PersistedQuery {
	sha256hash: &'static str,
}

#[derive(Serialize)]
struct Extensions {
	persistedQuery: PersistedQuery,
}

#[derive(Serialize)]
struct SignupPromptCategory {
	variables: Variables,
	extensions: Extensions,
}

#[derive(Deserialize)]
struct Stream {
	// Ignore `id`, `game` and `__typename`
}

#[derive(Deserialize)]
struct User {
	stream: Option<Stream>, // Ignore `id` and `__typename`
}

#[derive(Deserialize)]
struct Data {
	/// Will be null if the user doesn't exist or has been banned
	user: Option<User>,
}

#[derive(Deserialize)]
struct Response {
	data: Data,
	// Ignore `extensions`
}

fn main() {
	let mut data = &*to_vec(
		&FOLLOWING
			.iter()
			.map(|channelLogin| SignupPromptCategory {
				variables: Variables {
					channelLogin,
					isLive: true,
					isVod: false,
					videoID: "",
				},
				extensions: Extensions {
					persistedQuery: PersistedQuery {
						sha256hash:
							"21c86683bbfd1a6e9e6636c2b460f94c5014272dcb56f0aa04a7d28d0633502c",
					},
				},
			})
			.collect::<Vec<_>>(),
	)
	.unwrap();

	let mut vec = Vec::new();

	let mut easy = Easy::new();

	let _ = easy.url("https://gql.twitch.tv/gql");
	let _ = easy.post(true);

	let mut easy_list = List::new();
	let _ = easy_list.append("Client-Id:kimne78kx3ncx6brgo4mv6wki5h1ko");
	let _ = easy.http_headers(easy_list);

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
	
	if let Ok(responses) = from_slice::<Vec<Response>>(&mut vec) {
		print!(
			"{}",
			responses
				.into_iter()
				.enumerate()
				.filter_map(|(i, response)| response.data.user.and_then(|user| user.stream).map(|_| FOLLOWING[i]))
				.collect::<Vec<_>>()
				.join(" ")
		);
	};
}
