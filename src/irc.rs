//! Support for reading twitch chat via IRC.
//!
//! Run `[play_stream]` to start the client, which will display the chat on the
//! `[ratatui::Terminal]` passed

use std::borrow::Borrow;
use std::collections::VecDeque;
use std::process::Stdio;

use crossterm::event::{Event, EventStream, KeyCode};
use curl::easy::Easy;
use futures::{SinkExt, StreamExt};
use irc::client::prelude::Config;
use irc::client::{Client, ClientStream};
use irc::proto::{self, Capability};
use ratatui::backend::Backend;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Span, Spans};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Tabs};
use ratatui::Terminal;
use serde::Deserialize;
use simd_json::from_slice;
use textwrap::wrap;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process;
use tokio::time::{interval, Duration};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol;

use crate::config::*;
use crate::utils::*;

/// Connect to the channel's IRC server and return it's `ClientStream`.
async fn connect_irc_client(login: &str) -> ClientStream {
	let mut client = Client::from_config(Config {
		channels: vec![["#", login].concat()],
		// Anonymous
		nickname: Some("justinfan0".to_owned()),
		server: Some("irc.chat.twitch.tv".to_owned()),
		..Config::default()
	})
	.await
	.expect("Should be able to open IRC connection");

	// We need this so the server sends chat metadata
	let _ = client.send_cap_req(&[
		Capability::Custom("twitch.tv/tags"),
		Capability::Custom("twitch.tv/commands"),
	]);

	let _ = client.identify();

	client.stream().expect("Should be able to get IRC stream")
}

/// Add an item to a queue, removing the first item if it's over the limit
fn add_to_queue<T>(queue: &mut VecDeque<T>, item: T, limit: u16) {
	// Remove the first element if the queue is at the limit
	if queue.len() as u16 == limit {
		queue.pop_front();
	}

	// Add this item to the queue
	queue.push_back(item);
}

/// Handles Incoming RFC message
fn handle_irc_command(
	message: proto::Message,
	chat: &mut VecDeque<ListItem>,
	info: &mut Vec<ListItem>,
	log: &mut VecDeque<ListItem>,
	terminal_rect: Rect,
) {
	match message.command {
		// Welcome message
		proto::Command::Response(_, mut response) => add_to_queue(
			chat,
			ListItem::new(response.swap_remove(1)),
			terminal_rect.height - 3,
		),
		proto::Command::Raw(command, response) => match &*command {
			// Someone was banned or had a message removed, let's put it in chat
			"CLEARCHAT" if response.len() != 1 => add_to_queue(
				chat,
				ListItem::new(
					[
						&*response.last().expect("We already know there are elements"),
						&*message
							.tags
							.expect("CLEARCHAT should have tags")
							.iter()
							.find(|x| x.0 == "ban-duration")
							.map_or("'s message was removed".to_owned(), |x| {
								[
									" banned for ",
									&x.1.clone().expect("ban-duration tag should have a value"),
									" minutes",
								]
								.concat()
							}),
					]
					.concat(),
				),
				terminal_rect.height - 3,
			),
			// Chat metadata
			"ROOMSTATE" => {
				// Add appropriate tags
				for tag in message.tags.expect("ROOMSTATE should have tags") {
					match &*tag.0 {
						"emote-only" => info.push(ListItem::new(
							[
								"Emote only: ",
								if tag.1.is_some_and(|x| &x == "1") {
									"On"
								} else {
									"Off"
								},
							]
							.concat(),
						)),
						"followers-only" => info.push(ListItem::new(
							[
								"Followers only: ",
								// "Off" if it's `None` or `-1`
								&tag.1.filter(|x| x != "-1").map_or("Off".to_owned(), |x| {
									if x == "0" {
										"Any followers".to_owned()
									} else {
										format_seconds(
											x.parse::<i64>()
												.expect("Response numbers should be valid") * 60,
										)
									}
								}),
							]
							.concat(),
						)),
						// Unique chat mode
						"r9k" => info.push(ListItem::new(
							[
								"Unique chat: ",
								if tag.1.is_some_and(|x| &x == "1") {
									"On"
								} else {
									"Off"
								},
							]
							.concat(),
						)),
						"slow" => info.push(ListItem::new(
							[
								"Slow chat: ",
								// "Off" if it's `None` or `0`
								&tag.1.filter(|x| x != "0").map_or("Off".to_owned(), |x| {
									format_seconds(
										x.parse::<i64>().expect("Response numbers should be valid"),
									)
								}),
							]
							.concat(),
						)),
						"subs-only" => info.push(ListItem::new(
							[
								"Subs only: ",
								if tag.1.is_some_and(|x| &x == "1") {
									"On"
								} else {
									"Off"
								},
							]
							.concat(),
						)),
						_ => (),
					}
				}
			}
			// Notice, i.e. someone subscribing
			"USERNOTICE" => {
				let mut tags = message
					.tags
					.expect("USERNOTICE should have tags")
					.into_iter();

				add_to_queue(
					chat,
					ListItem::new(Span {
						style: Style {
							fg: tags
								.find(|x| x.0 == "color")
								.map(|x| {
									x.1.filter(|x| !x.is_empty())
										.as_ref()
										.map(|x| parse_colour(&x[1..]))
								})
								.flatten(),
							..Style::default()
						},
						content: tags
							.find(|x| x.0 == "system-msg")
							.expect("USERNOTICE should have a system-msg tag")
							.1
							.expect("system-msg should have a value")
							.into(),
					}),
					terminal_rect.height - 3,
				)
			}
			_ => (),
		},
		// New message
		proto::Command::PRIVMSG(_, msg) => {
			// The parts of the message
			let mut vec = Vec::with_capacity(2);

			// The number of months the person has subscribed for
			let mut subscriber = None;
			// The user's most recent prediction, if any
			let mut predictions = None;
			// The user's colour
			let mut colour = None;

			for tag in message.tags.expect("PRIVMSG should have tags") {
				match &*tag.0 {
					// Info for tags
					"badge-info" => {
						if let Some(badge_info) = tag.1.filter(|x| !x.is_empty()) {
							for badge in badge_info.split(',') {
								let parts = badge
									.split_once('/')
									.expect("badge-info item should have a '/'");

								match parts.0 {
									// Their most recent prediction
									"predictions" => predictions = Some(parts.1.to_owned()),
									// This is also in `badges`, but sometimes wrong
									"subscriber" => subscriber = Some(parts.1.to_owned()),
									// Log unknown badges
									_ => (),
								}
							}
						}
					}
					"badges" => {
						if let Some(badges) = tag.1.filter(|x| !x.is_empty()) {
							for badge in badges.split(',') {
								let parts =
									badge.split_once('/').expect("badge item should have a '/'");

								match parts.0 {
									// Verified
									"partner" => vec.push(Span {
										content: "✓".into(),
										style: Style {
											fg: Some(Color::White),
											bg: Some(Color::Magenta),
											..Style::default()
										},
									}),

									"predictions" => {
										let parts = parts
											.1
											.split_once('-')
											.expect("Predictions badge should have '-'");

										vec.push(Span {
											content: [
												predictions
													.as_ref()
													.map_or(parts.1, Borrow::borrow),
												" ",
											]
											.concat()
											.into(),
											style: Style {
												fg: match parts.0 {
													"blue" => Some(Color::Blue),
													"pink" => Some(Color::Magenta),
													// Log unknown colour
													c => {
														add_to_queue(
															log,
															ListItem::new(
																["Unknown colour: ", c].concat(),
															),
															terminal_rect.height - 3,
														);

														None
													}
												},
												..Style::default()
											},
										});
									}
									// This person has twitch premium, diplay a crown
									"premium" => vec.push(Span {
										content: "👑".into(),
										style: Style {
											fg: Some(Color::White),
											bg: Some(Color::Blue),
											..Style::default()
										},
									}),
									// This user is a moderator
									"moderator" => vec.push(Span {
										// Closest to the actual moderator badge
										content: "🗡️".into(),
										style: Style {
											fg: Some(Color::White),
											bg: Some(Color::Green),
											..Style::default()
										},
									}),
									// This person has had n moments on this channel.
									// Display a camera with appropriate colours.
									"moments" => vec.push(Span {
										content: "📷".into(),
										style: Style {
											fg: Some(Color::White),
											bg: Some(
												match parts
													.1
													.parse::<u16>()
													.expect("Moments number should be valid")
												{
													// Bronze (#CD7F32)
													..20 => Color::Rgb(0xCD, 0x7F, 0x32),
													// Silver (#C0C0C0)
													20..60 => Color::Rgb(0xC0, 0xC0, 0xC0),
													// Gold (#FFD700)
													60..120 => Color::Rgb(0xFF, 0xD7, 0x00),
													// Diamond (#B9F2FF)
													120..200 => Color::Rgb(0xB9, 0xF2, 0xFF),
													// Purple (#800080)
													200.. => Color::Rgb(0x80, 0x00, 0x80),
												},
											),
											..Style::default()
										},
									}),
									// This user is watching without audio
									"no_audio" => vec.push(Span {
										content: "🔇".into(),
										style: Style {
											fg: Some(Color::White),
											bg: Some(Color::Black),
											..Style::default()
										},
									}),
									// This user is listening without video
									// Eye with strikethrough
									"no_video" => vec.push(Span {
										content: "👁".into(),
										style: Style {
											fg: Some(Color::White),
											bg: Some(Color::Black),
											add_modifier: Modifier::CROSSED_OUT,
											..Style::default()
										},
									}),
									// This person has gifted subs
									"sub-gifter" => vec.push(Span {
										content: "🎁".into(),
										style: Style {
											// It will always be one of these, so we don't need to
											// parse the int.
											// These are meant to be the same as the official
											// colours
											fg: Some(match parts.1 {
												"1" => Color::Magenta,
												"5" => Color::Cyan,
												"10" => Color::Blue,
												"25" => Color::Red,
												"50" => Color::LightMagenta,
												"100" => Color::Green,
												_ => Color::Yellow,
											}),
											..Style::default()
										},
									}),
									"subscriber" => vec.push(
										[
											"sub/",
											subscriber.as_ref().map_or(parts.1, Borrow::borrow),
											" ",
										]
										.concat()
										.into(),
									),
									"vip" => vec.push(Span {
										content: "💎".into(),
										style: Style {
											fg: Some(Color::White),
											bg: Some(Color::LightMagenta),
											..Style::default()
										},
									}),
									// Ignore other badges
									_ => (),
								}
							}
						}
					}
					"color" => colour = tag.1,
					"display-name" => vec.push(Span {
						content: [&tag.1.expect("Should be a display name"), " "]
							.concat()
							.into(),
						style: Style {
							fg: colour
								.as_ref()
								.filter(|x| !x.is_empty())
                                // Remove the first character of the hex code '#'
								.map(|x| parse_colour(&x[1..])),
							..Style::default()
						},
					}),
					// Ignore other tags
					_ => (),
				}
			}

			// Width of the user metadata (badges/name)
			let meta_width = vec.iter().map(Span::width).sum();

			// Wrap text if it needs to be
			let wrapped_text = wrap(&msg, terminal_rect.width as usize - meta_width - 2);

			// Add the first line to the same line
			vec.push(wrapped_text[0].clone().into_owned().into());

			add_to_queue(
				chat,
				ListItem::new::<Spans>(vec.into()),
				terminal_rect.height - 3,
			);

			// Add any new lines for text if needed
			for line in &wrapped_text[1..] {
				add_to_queue(
					chat,
					ListItem::new([&*" ".repeat(meta_width), &*line].concat()),
					terminal_rect.height - 3,
				);
			}
		}
		// Ignore any other responses
		_ => (),
	}
}

/// A user from  websocket response
#[derive(Deserialize)]
struct User {
	display_name: String, // Ignore `id` and `login`
}

/// Information about a reward
#[derive(Deserialize)]
struct Reward {
	title: String,
	// Max cost is 2^31 - 1, so fits in 32 bits
	cost: u32,
	background_color: String,
	// Ignore `id`, `channel_id`, `prompt`, `is_user_input_required`, `is_sub_only`, `image`,
	// `default_image`, `is_enabled`, `is_paused`, `is_in_stock` and `max_per_stream`
}

/// Information about a reward redemption
#[derive(Deserialize)]
struct Redemption {
	user: User,
	reward: Reward, // Ignore `id`, `channel_id`, `redeemed_at`, `status` and `cursor`
}

/// Data for community points event
#[derive(Deserialize)]
struct CommunityPointsChannelV1Data {
	redemption: Redemption,
	// Ignore `timestamp`
}

/// An event to do with community points, i.e. redeeming a reward
#[derive(Deserialize)]
struct CommunityPointsChannelV1 {
	data: CommunityPointsChannelV1Data, // Ignore `type`
}

/// View count
#[derive(Deserialize)]
struct VideoPlaybackById {
	viewers: u32,
	// Ignore `type` and `server_time`
}

/// Data from a websocket response message
#[derive(Deserialize)]
struct WebsocketMessageData {
	topic: String,
	message: String,
}

/// Message from the twitch websocket
#[derive(Deserialize)]
struct WebsocketMessage {
	data: Option<WebsocketMessageData>,
	// Ignore `type`
}

fn handle_websocket_message(
	mut text: String,
	terminal_size: Rect,
	chat: &mut VecDeque<ListItem>,
	log: &mut VecDeque<ListItem>,
	viewers: &mut Paragraph,
) {
	if let Ok(WebsocketMessage {
		data: Some(mut data),
	}) = from_slice::<WebsocketMessage>(unsafe { text.as_bytes_mut() })
	{
		let (topic, channel_id) = data.topic.split_once('.').expect("Topic should have a dot");

		let message = unsafe { data.message.as_bytes_mut() };

		match topic {
			"community-points-channel-v1" => {
				let redemption = from_slice::<CommunityPointsChannelV1>(message)
					.expect("Websocket message should be valid JSON")
					.data
					.redemption;

				add_to_queue(
					chat,
					ListItem::new(Span {
						content: [
							&redemption.user.display_name,
							" redeemed ",
							&redemption.reward.title,
							" (",
							&redemption.reward.cost.to_string(),
							")",
						]
						.concat()
						.into(),
						style: Style {
							fg: Some(parse_colour(&redemption.reward.background_color[1..])),
							..Style::default()
						},
					}),
					terminal_size.height - 3,
				);
			}
			"video-playback-by-id" => {
				if let Ok(video_playback_by_id) = &from_slice::<VideoPlaybackById>(message) {
					*viewers = Paragraph::new(Span {
						content: ["👤", &video_playback_by_id.viewers.to_string()]
							.concat()
							.into(),
						style: Style {
							fg: Some(Color::Red),
							..Style::default()
						},
					});
				}
			}
			// Log unknown message
			u => wrap(&[u, " ", &data.message].concat(), {
				let mut options = textwrap::Options::new(terminal_size.width as usize - 2);
				options.word_separator = textwrap::WordSeparator::UnicodeBreakProperties;
				options
			})
			.into_iter()
			.for_each(|x| add_to_queue(log, ListItem::new([x].concat()), terminal_size.height - 3)),
		}
	}
}

/// Connect to a stream and display chat
#[tokio::main]
pub async fn play_stream<B: Backend>(
	terminal: &mut Terminal<B>,
	easy: &mut Easy,
	login: &str,
	id: &String,
	qualities: &[&str],
) {
	let mut child = process::Command::new("streamlink")
		.args([
			["-p=", &PLAYER.join(" ")].concat(),
			["twitch.tv/", login].concat(),
			qualities.join(","),
		])
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.expect("Should be able to spawn streamlink");

	// So we can add it to the log
	let mut stdout_reader = BufReader::new(
		child
			.stdout
			.as_mut()
			.expect("Should be able to access command stdout"),
	)
	.lines();
	let mut stderr_reader = BufReader::new(
		child
			.stderr
			.as_mut()
			.expect("Should be able to access command stdout"),
	)
	.lines();

	// Connect to IRC
	let mut client_stream = connect_irc_client(login).await;

	// Connect to websocket
	let mut web_socket_stream = connect_async("wss://pubsub-edge.twitch.tv/v1")
		.await
		.expect("Should be able to connect to twitch websocket")
		.0;

	// Ping every 4 minutes so it doesn't time out
	// It could be up to 7 minutes, but this is what the webapp does
	let mut ping_interval = interval(Duration::new(4 * 60, 0));

	// Listen to all the events that the web client does, minus "ads"/"ad-property-refresh"
	// The twitch websocket requires you to send each as an individual packet
	for topic in [
		/*"broadcast-settings-update",
		"channel-bounty-board-events.cta",
		"channel-drop-events",
		// Gifted subs
		"channel-sub-gifts-v1",
		"charity-campaign-donation-events-v1",
		"community-boost-events-v1",*/
		// Rewards
		"community-points-channel-v1",
		// Goal updates
		/*"creator-goals-events-v1",
		"extension-control",
		"guest-star-channel-v1",
		"hype-train-events-v1",
		"pinned-chat-updates-v1",
		"polls",
		"predictions-channel-v1",
		"pv-watch-party-events",
		"radio-events-v1",
		"raid",
		"request-to-join-channel-v1",
		"shoutout",
		"sponsorships-v1",*/
		// Rich chat (images/clips) (we can't display these)
		//"stream-chat-room-v1",
		// Current view count
		"video-playback-by-id",
	] {
		let _ = web_socket_stream
			.send(protocol::Message::Text(
				// rustfmt wants to make this one line, which is harder to read
				#[rustfmt::skip]
                [
                    "{\
                        \"type\":\"LISTEN\",\
                        \"data\":{\
                            \"topics\":[\
                                \"", topic, ".", &id, "\"\
                            ]\
                        }\
                    }"
                ].concat().to_owned(),
			))
			.await;
	}

	// Input (but async)
	let mut event_stream = EventStream::new();

	// Tab selected
	let mut tab = 0usize;

	// Amount of rows for chat to be displayed on
	let height = (terminal
		.size()
		.expect("Should be able to get terminal size")
		.height - 3) as usize;

	// Chat items, we use a queue for this to make truncation more performant
	// Reserve space for one item per available line
	let mut chat = VecDeque::with_capacity(height);

	// There are only 5 bits of info to display
	let mut info = Vec::with_capacity(5);

	// Items in the log
	let mut log = VecDeque::with_capacity(height);

	// View count
	let mut viewers = Paragraph::new(Span {
		content: "👤".into(),
		style: Style {
			fg: Some(Color::Red),
			..Style::default()
		},
	});

	// Run until streamlink dies
	//while let Ok(None) = child.try_wait() {

	// Run until the user inputs 'q'
	loop {
		// Wait for either a new message or keyboard input
		tokio::select! {
			// Read output from streamlink - add it to the log
			Ok(Some(line)) = stdout_reader.next_line() => add_to_queue(
				&mut log,
				ListItem::new(line),
				terminal
					.size()
					.expect("Should be able to get terminal dimensions")
					.height - 3
			),
			Ok(Some(line)) = stderr_reader.next_line() => add_to_queue(
				&mut log,
				ListItem::new(line),
				terminal
					.size()
					.expect("Should be able to get terminal dimensions")
					.height - 3
			),
			// Read new message in chat
			next = client_stream.next() => if let Some(Ok(message)) = next {
				handle_irc_command(
					message,
					&mut chat,
					&mut info,
					&mut log,
					terminal.size().expect("Should be able to get terminal dimensions")
				)
			} else {
				// The connection failed, let's try again
				add_to_queue(
					&mut log,
					ListItem::new("IRC connection failed, retrying"),
					terminal
						.size()
						.expect("Should be able to get terminal dimensions")
						.height - 3
				);

				client_stream = connect_irc_client(login).await;
			},
			// Read from websocket
			Some(Ok(protocol::Message::Text(text))) = web_socket_stream.next() => {
				handle_websocket_message(
					text,
					terminal
						.size()
						.expect("Should be able to get terminal dimensions"),
					&mut chat,
					&mut log,
					&mut viewers
				);
			}
			// Ping twitch websocket every 4 minutes
			_ = ping_interval.tick() => {
				// Twitch's websocket doesn't work with actual pings,
				// it has to be a message saying it
				let _ = web_socket_stream.send(protocol::Message::Text(
					r#"{"type":"PING"}"#.to_owned()
				)).await;
			}
			// Read keyboard input
			Some(Ok(event)) = event_stream.next() => {
				match event {
					Event::Key(key) => match key.code {
						// Quit
						KeyCode::Char('Q' | 'q') => break,
						// Select next tab to the left
						KeyCode::Left => tab = tab.saturating_sub(1),
						// Select next tab to the right
						KeyCode::Right => if tab != 2 { tab += 1 },
						_ => ()
					},
					Event::Resize(_, height) => {
						// Truncate lists if needed
						for queue in [&mut chat, &mut log] {
							if height - 3 < queue.len() as u16 {
								// Remove items from the front
								queue.drain(..queue.len() - (height - 3) as usize);
							}
						}
					},
					_ => (),
				}
			}
		}

		// Draw screen
		let _ = terminal.draw(|frame| {
			// Tabs at the top
			frame.render_widget(
				Tabs::new(vec!["Chat".into(), "Info".into(), "Log".into()])
					.block(
						Block::default()
							.borders(Borders::ALL)
							.title_alignment(TITLE_ALIGNMENT)
							.border_type(BORDER_TYPE),
					)
					.highlight_style(Style {
						add_modifier: Modifier::REVERSED,
						..Style::default()
					})
					.select(tab),
				Rect {
					height: 3,
					..frame.size()
				},
			);

			frame.render_widget_reusable(
				&viewers,
				Rect {
					// Enough space for 7 digits + 2 for symbol + 2 for spacing
					x: frame.size().width - 11,
					y: 1,
					width: 9,
					height: 1,
				},
			);

			frame.render_widget(
				List::new(
					// Which list should we render
					match tab {
						0 => chat.clone().into(),
						1 => info.clone(),
						2 => log.clone().into(),
						// We make sure it doesn't go past the bounds
						_ => unreachable!(),
					},
				),
				Rect {
					x: 1,
					y: 3,
					width: frame.size().width - 2,
					height: frame.size().height - 3,
				},
			);
		});
	}
}
