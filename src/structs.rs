//! Structures used for JSON {,de}serialization in twitch HTTPS requests. These also provide some
//! convenience methods such as [`TwitchResponse::to_widgets`] and [`Node::select`].

// For the JSON stuff:
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
// Because we don't actually construct most of these structs
#![allow(dead_code)]

use std::borrow::Cow;
use std::io::stdout;
use std::process::Command;
use std::str::from_utf8;

use chrono::{DateTime, Utc};
use crossterm::execute;
use crossterm::terminal::{
	disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use curl::easy::Easy;
use ratatui::backend::Backend;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Span, Spans, Text};
use ratatui::widgets::{List, ListItem, Paragraph, Wrap};
use ratatui::Terminal;
use serde::{Deserialize, Serialize};
use simd_json::from_slice;

use crate::config::*;
use crate::utils::*;

/// Takes text and makes it take an extra line
fn spaced<'a, T: Into<Spans<'a>>>(text: T) -> Text<'a> {
	Text {
		lines: vec![text.into(), Spans::default()],
	}
}

// Takes a string and returns it as a [`Span`] with an underline.
fn header<'a, T: Into<Cow<'a, str>>>(content: T) -> Span<'a> {
	Span {
		content: content.into(),
		style: Style {
			add_modifier: Modifier::UNDERLINED,
			..Style::default()
		},
	}
}

/// Formats a date according to config.
fn format_date(string: &String) -> String {
	if let Ok(dt) = string.parse::<DateTime<Utc>>() {
		if let Some(fmt) = DATE_FORMAT {
			// Use user's format
			dt.format(fmt).to_string()
		} else {
			// Relative time
			let delta = Utc::now().signed_duration_since(dt).num_seconds();

			if delta < 0 {
				// It's in the future
				["In ", &format_seconds(delta.abs())].concat()
			} else {
				// It's in the past
				[&format_seconds(delta.abs()), " ago"].concat()
			}
		}
	} else {
		// If the date was invalid, it was probably the "Music" category (200,000 years ago was
		// about when music was invented).
		"200,000 years ago".to_owned()
	}
}

/// A title, no additional info
#[derive(Debug)]
pub enum Title {
	/// [`Vec`] of [`TitleTokenEdge`]s,
	Tokens(Vec<TitleTokenEdge>),
	/// Just one string
	Fallback(String),
}

// Request JSON

// PersonalSection

#[derive(Serialize)]
pub struct RecommendationContext {
	pub platform: Option<&'static str>,
	pub clientApp: Option<&'static str>,
	pub location: Option<&'static str>,
	pub referrerDomain: Option<&'static str>,
	pub viewportHeight: Option<u16>,
	pub viewportWidth: Option<u16>,
	pub channelName: Option<&'static str>,
	pub categoryName: Option<&'static str>,
	pub lastChannelName: Option<&'static str>,
	pub lastCategoryName: Option<&'static str>,
	pub pageviewContent: Option<&'static str>,
	pub pageviewContentType: Option<&'static str>,
	pub pageviewLocation: Option<&'static str>,
	pub pageviewMedium: Option<&'static str>,
	pub previousPageviewContent: Option<&'static str>,
	pub previousPageviewContentType: Option<&'static str>,
	pub previousPageviewLocation: Option<&'static str>,
	pub previousPageviewMedium: Option<&'static str>,
}

#[derive(Serialize)]
pub struct PersonalSectionsInput {
	pub sectionInputs: Vec<&'static str>,
	// It's optional for RECOMMENDED_SECTION, but required for SIMILAR_SECTION
	pub recommendationContext: RecommendationContext,
	pub contextChannelName: Option<&'static str>,
}

pub trait Variables: Default {
	const SHA256HASH: &'static str;
}

#[derive(Serialize)]
pub struct PersonalSectionsVariables {
	pub input: PersonalSectionsInput,
	pub creatorAnniversariesExperimentEnabled: bool,
}
impl Variables for PersonalSectionsVariables {
	const SHA256HASH: &'static str =
		"f8cc9b91bb629f2d09dd8299d9f07c4daefe019236a19fc12fa2b14eb95c359e";
}

// Shelves

#[derive(Serialize)]
pub struct ShelvesContext {
	pub clientApp: Option<&'static str>,
	pub location: Option<&'static str>,
	pub referrerDomain: Option<&'static str>,
	pub viewportHeight: Option<u16>,
	pub viewportWidth: Option<u16>,
}

#[derive(Serialize)]
pub struct ShelvesVariables {
	// `u64` just in case
	pub imageWidth: Option<u64>,
	pub itemsPerRow: u16,
	pub langWeightedCCU: Option<bool>,
	pub platform: &'static str,
	pub requestID: &'static str,
	pub context: Option<ShelvesContext>,
	pub verbose: Option<bool>,
}
impl Variables for ShelvesVariables {
	const SHA256HASH: &'static str =
		"41858598cc637cf9e6153818f5a4d274a08e8743e4a85903cdfe39c464152404";
}

#[derive(Serialize, Default)]
struct VideoAccessToken_ClipVariables {
	slug: String,
}
impl Variables for VideoAccessToken_ClipVariables {
	const SHA256HASH: &'static str =
		"36b89d2507fce29e5ca551df756d27c1cfe079e2609642b4390aa4c35796eb11";
}

#[derive(Serialize)]
pub struct DirectoryPage_GameOptions {
	pub sort: &'static str,
	pub recommendationsContext: Option<RecommendationContext>,
	pub requestID: Option<&'static str>,
	pub freeformTags: Option<Vec<&'static str>>,
	pub tags: Option<Vec<&'static str>>,
}

#[derive(Serialize)]
pub struct DirectoryPage_GameVariables {
	pub imageWidth: Option<u64>,
	/// The category name
	pub name: String,
	pub options: DirectoryPage_GameOptions,
	pub sortTypeIsRecency: bool,
	pub limit: u32,
}
impl Variables for DirectoryPage_GameVariables {
	const SHA256HASH: &'static str =
		"df4bb6cc45055237bfaf3ead608bbafb79815c7100b6ee126719fac3762ddf8b";
}

#[derive(Serialize)]
pub struct Target {
	pub index: &'static str,
}

#[derive(Serialize)]
pub struct SearchResultsPage_SearchResultsOptions {
	pub targets: Option<Vec<Target>>,
}

#[derive(Serialize)]
pub struct SearchResultsVariables {
	/// The search
	pub query: String,
	pub options: Option<SearchResultsPage_SearchResultsOptions>,
	pub requestID: Option<String>,
}
impl Variables for SearchResultsVariables {
	const SHA256HASH: &'static str =
		"6ea6e6f66006485e41dbe3ebd69d5674c5b22896ce7b595d7fce6411a3790138";
}

#[derive(Serialize)]
pub struct PlaybackAccessTokenVariables {
	/// Should always be `false`
	pub isLive: bool,
	/// Must be `true`
	pub isVod: bool,
	pub login: &'static str,
	pub playerType: &'static str,
	/// Set at runtime
	pub vodID: String,
}
impl Variables for PlaybackAccessTokenVariables {
	const SHA256HASH: &'static str =
		"0828119ded1c13477966434e15800ff57ddacf13ba1911c129dc2200705b0712";
}

#[derive(Serialize)]
pub struct PersistedQuery {
	pub sha256hash: &'static str,
}

#[derive(Serialize)]
pub struct RequestExtensions {
	pub persistedQuery: PersistedQuery,
}

/// POST data for API calls.
#[derive(Serialize)]
pub struct TwitchRequest<T: Variables> {
	pub variables: T,
	pub extensions: RequestExtensions,
}
impl<T: Variables> Default for TwitchRequest<T> {
	fn default() -> Self {
		Self {
			variables: T::default(),
			extensions: RequestExtensions {
				persistedQuery: PersistedQuery {
					sha256hash: T::SHA256HASH,
				},
			},
		}
	}
}

/// Page loaded on start
pub enum HomePage {
	/// The bit on the left on the webapp
	PersonalSection,
	/// The main home page
	Shelves,
	/// A category
	Game(&'static str),
	/// A search
	Search(&'static str),
}

// Response JSON

// Personalsection

#[derive(Deserialize, Debug)]
struct PersonalSectionTitle {
	localizedFallback: String,
	// Ignore `localizedTokens` and `__typename`
}

#[derive(Deserialize, Debug)]
struct BroadcastSettings {
	title: String, // Ignore `id` and `__typename`
}

#[derive(Deserialize, Debug)]
struct UserRoles {
	isPartner: bool, // Ignore `__typename`
}

#[derive(Deserialize, Debug)]
pub struct User {
	login: String,
	displayName: String,
	primaryColorHex: Option<String>,
	broadcastSettings: Option<BroadcastSettings>,
	roles: Option<UserRoles>,
	// Ignore `id`, `profileImageURL`, `largeProfileImageURL` and `__typename`
}

impl User {
	// Get a [`ratatui::Style`] with the user's colour as forground.
	fn style(&self) -> Style {
		Style {
			fg: self
				.primaryColorHex
				.as_ref()
				.map(|primary_colour_hex| parse_colour(&primary_colour_hex)),
			..Style::default()
		}
	}
}

#[derive(Deserialize, Debug)]
struct Tag {
	localizedName: String, // Ignore `id`, `isLanguageTag`, `tagName`, `__typename`
}

#[derive(Deserialize, Debug)]
pub struct Game {
	viewersCount: Option<u32>,
	name: String,
	displayName: Option<String>,
	#[serde(alias = "tags")]
	gameTags: Option<Vec<Tag>>,
	originalReleaseDate: Option<String>, // Ignore `id`, `boxArtURL and `__typename`
}

#[derive(Deserialize, Debug)]
struct PersonalSectionContent {
	viewersCount: u32,
	game: Game,
	// Ignore `id`, `previewImageURL`, `broadcaster`, `type` and `__typename`
}

#[derive(Deserialize, Debug)]
pub struct PersonalSectionChannel {
	user: User,
	content: PersonalSectionContent, /* Ignore `trackingID`, `promotionsCampaignID`, `label` and
	                                  * `__typename` */
}

#[derive(Deserialize, Debug)]
struct PersonalSection {
	title: PersonalSectionTitle,
	items: Vec<PersonalSectionChannel>, // Ignore `type` and `__typename`
}

// Shelves

#[derive(Deserialize, Debug)]
struct BrowsableCollectionTitle {
	fallbackLocalizedTitle: String, // Ignore `__typename`
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum TextToken {
	BrowsableCollection {
		collectionName: BrowsableCollectionTitle,
		// Ignore `id`, `slug` and `__typename`
	},
	Game(Game),
	TextToken {
		text: String,
		hasEmphasis: bool,
		// Ignore `location` and `__typename`
	},
	None,
}

#[derive(Deserialize, Debug)]
pub struct TitleTokenEdge {
	node: TextToken,
	// Ignore `__typename`
}

#[derive(Deserialize, Debug)]
struct ShelfTitle {
	fallbackLocalizedTitle: String,
	localizedTitleTokens: Vec<TitleTokenEdge>,
	// Ignore `key`, `context` and `__typename`
}

#[derive(Deserialize, Debug)]
pub struct FreeformTag {
	name: String, // Ignore `id`, `__typename`
}

#[derive(Deserialize, Debug)]
struct PlaybackAccessToken {
	signature: String,
	value: String,
	// Ignore `__typename`
}

#[derive(Deserialize, Debug)]
struct ClipVideoQuality {
	quality: String,
	sourceURL: String,
	// Ignore `frameRate` and `__typename`
}

#[derive(Deserialize, Debug)]
struct Clip {
	playbackAccessToken: PlaybackAccessToken,
	videoQualities: Vec<ClipVideoQuality>, // Ignore `id` and `__typename`
}

#[derive(Deserialize, Debug)]
struct VideoAccessToken_ClipData {
	clip: Clip,
}

#[derive(Deserialize, Debug)]
struct VideoAccessToken_ClipResponse {
	data: VideoAccessToken_ClipData,
	// Ignore `extensions`
}

#[derive(Deserialize, Debug)]
struct PlaybackAccessTokenData {
	videoPlaybackAccessToken: PlaybackAccessToken,
}

#[derive(Deserialize, Debug)]
struct PlaybackAccessTokenResponse {
	data: PlaybackAccessTokenData, // Ignore `extensions`
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum Node {
	Clip {
		slug: String,
		clipTitle: String,
		clipViewCount: u32,
		curator: User,
		game: Game,
		broadcaster: User,
		clipCreatedAt: String,
		// Clips are 60 seconds max
		durationSeconds: u8,
		language: String,
		// Ignore `id`, `url`, `embedURl`, `thumbnailURL`, `champBadge` and `__typename`
	},
	Game(Game),
	Stream {
		broadcaster: User,
		game: Option<Game>,
		freeformTags: Vec<FreeformTag>,
		viewersCount: u32,
		createdAt: Option<String>,
		// Ignore `id`, `previewImageUrl`, `type` and `__typename`
	},
	/// Property is the VOD ID
	Video(String),
	None,
}
impl Node {
	/// Select this node. Returns the game name if it needs to be moved into.
	pub fn select<B: Backend>(
		&self,
		terminal: &mut Terminal<B>,
		easy: &mut Easy,
		qualities: &[&str],
	) -> Option<String> {
		match self {
			Node::Clip { slug, .. } => {
				let _ = disable_raw_mode();

				// We want to be in a normal terminal
				let _ = execute!(stdout(), LeaveAlternateScreen);

				let response = from_slice::<VideoAccessToken_ClipResponse>(&mut request(
					easy,
					&TwitchRequest {
						variables: VideoAccessToken_ClipVariables { slug: slug.clone() },
						..TwitchRequest::default()
					},
				))
				.expect("Response should be valid JSON");

				// Default to best quality
				let mut source_url = &response.data.clip.videoQualities[0].sourceURL;
				for quality in qualities {
					match *quality {
						"audio_only" | "worst" => {
							// Get last quality
							source_url = &response
								.data
								.clip
								.videoQualities
								.last()
								.expect("Server should give at least one quality")
								.sourceURL;
							break;
						}
						"best" => {
							// Use first quality
							break;
						}
						_ => {
							// If quality ends in a p
							if matches!(quality.as_bytes()[quality.len() - 1], b'P' | b'p') {
								// See if the requested quality is available
								if let Some(clip_video_quality) =
									response.data.clip.videoQualities.iter().find(
										|clip_video_quality| {
											clip_video_quality.quality
												== quality[..quality.len() - 1]
										},
									) {
									source_url = &clip_video_quality.sourceURL;
									break;
								}
							}
							// Otherwise, the string is wrong
						}
					}
				}

				let _ = Command::new(PLAYER[0])
					.args(&PLAYER[1..])
					.arg(
						[
							source_url,
							"?sig=",
							&response.data.clip.playbackAccessToken.signature,
							"&token=",
							// token needs to be urlencoded again, luckily we just need to replace
							// `%`s
							&response
								.data
								.clip
								.playbackAccessToken
								.value
								.replace("%", "%25"),
						]
						.concat(),
					)
					.spawn()
					.expect(&["Should be able to spawn PLAYER (", &PLAYER.join(" "), ")"].concat())
					.wait();

				let _ = enable_raw_mode();
				let _ = execute!(stdout(), EnterAlternateScreen);

				None
			}
			Node::Game(Game { name, .. }) => Some(name.clone()),
			Node::Stream {
				broadcaster: User { login, .. },
				..
			} => {
				// Load chat UI if enabled
				#[cfg(feature = "chat")]
				crate::irc::play_stream(terminal, login, qualities);

				// Otherwise, just run the stream
				#[cfg(not(feature = "chat"))]
				{
					let _ = disable_raw_mode();
					// We want to be in a normal terminal
					let _ = execute!(stdout(), LeaveAlternateScreen);

					let _ = Command::new("streamlink")
						.args([
							["-p=", &PLAYER.join(" ")].concat(),
							["twitch.tv/", login].concat(),
							qualities.join(","),
						])
						.spawn()
						.expect("Should be able to spawn streamlink")
						.wait();

					let _ = enable_raw_mode();
					let _ = execute!(stdout(), EnterAlternateScreen);
				}

				None
			}
			Node::Video(vodID) => {
				let _ = disable_raw_mode();

				// We want to be in a normal terminal
				let _ = execute!(stdout(), LeaveAlternateScreen);

				let response = from_slice::<PlaybackAccessTokenResponse>(&mut request(
					easy,
					&TwitchRequest {
						variables: PlaybackAccessTokenVariables {
							vodID: vodID.to_owned(),
							..TwitchRequest::default().variables
						},
						..TwitchRequest::default()
					},
				))
				.expect("Response should be valid JSON");

				let mut new_easy = Easy::new();

				let _ = new_easy.url(
					&[
						"https://usher.ttvnw.net/vod/",
						vodID,
						".m3u8?sig=",
						&response.data.videoPlaybackAccessToken.signature,
						"&token=",
						&response.data.videoPlaybackAccessToken.value,
					]
					.concat(),
				);

				// Make sure `vec` lives longer than `transfer`
				let mut vec = Vec::new();

				{
					let mut transfer = new_easy.transfer();

					let _ = transfer.write_function(|slice| {
						vec.extend_from_slice(slice);
						Ok(slice.len())
					});

					let _ = transfer.perform();
				}

				// Set to `Some` when the appropriate URL is found
				let mut url = None;

				// Split response into lines
				let mut split = from_utf8(&vec)
					.expect("Response should be valid utf8")
					.split('\n');
				for quality in qualities {
					match *quality {
						"audio_only" | "worst" => {
							// Get last URL
							url = Some(split.clone().last());
							break;
						}
						"best" => {
							// Get first URL
							url = Some(split.nth(4));
							break;
						}
						_ => {
							// Iterate through each `#EXT-X-STREAM-INF` line.
							for (i, line) in split.clone().enumerate().skip(3).step_by(2) {
								// If this line is the requested quality
								if line.contains(quality) {
									// The next line is the URL
									url = Some(split.nth(i + 1));
								}
							}
							// This quality isn't available, try the next one
						}
					};
				}

				let _ = Command::new(PLAYER[0])
					.args(&PLAYER[1..])
					.arg(
						// Default to best quality
						url.unwrap_or(split.nth(4))
							.expect("Should be able to get a VOD URL"),
					)
					.spawn()
					.expect(&["Should be able to spawn PLAYER (", &PLAYER.join(" "), ")"].concat())
					.wait();

				let _ = enable_raw_mode();

				let _ = execute!(stdout(), EnterAlternateScreen);

				None
			}
			Node::None => None,
		}
	}
}
impl Into<Node> for String {
	/// Get a `Node::Stream` object from a username string.
	fn into(self) -> Node {
		// Doesn't need most of this information, just `broadcaster.login`
		Node::Stream {
			broadcaster: User {
				login: self,
				displayName: String::new(),
				primaryColorHex: None,
				broadcastSettings: Some(BroadcastSettings {
					title: String::new(),
				}),
				roles: None,
			},
			game: None,
			freeformTags: Vec::new(),
			viewersCount: 0,
			createdAt: None,
		}
	}
}

#[derive(Deserialize, Debug)]
pub struct ShelfContentEdge {
	node: Node, /* Ignore `cursor`, `trackingID`, `promotionsCampaignID`, `sourceType`
	             * and `__typename` */
}

#[derive(Deserialize, Debug)]
struct ShelfContentConnection {
	edges: Vec<ShelfContentEdge>,
	// Ignore `__typename`
}

#[derive(Deserialize, Debug)]
struct Shelf {
	title: ShelfTitle,
	content: ShelfContentConnection,
	// Ignore `id`, `trackingInfo` and `__typename`
}

#[derive(Deserialize, Debug)]
struct ShelfEdge {
	node: Shelf,
	// Ignore `__typename`
}

#[derive(Deserialize, Debug)]
struct ShelfConnection {
	edges: Vec<ShelfEdge>,
	// Ignore `verboseResults` and `__typename`
}

#[derive(Deserialize, Debug)]
struct Stream {
	title: String,
	viewersCount: u32,
	createdAt: String,
	broadcaster: User,
	freeformTags: Vec<FreeformTag>,
	game: Game, // Ignore `id`, `previewImageUrl`, `type` and `__typename`
}

#[derive(Deserialize, Debug)]
struct StreamEdge {
	node: Stream, // Ignore `cursor`, `trackingID` and `__typename`
}

#[derive(Deserialize, Debug)]
struct StreamConnection {
	edges: Vec<StreamEdge>,
	// Ignore `pageInfo` and `__typename`
}

#[derive(Deserialize, Debug)]
struct Category {
	streams: StreamConnection,
	// Ignore `id`, `name`, `displayName` and `__typename`
}

#[derive(Deserialize, Debug)]
struct FollowerConnection {
	totalCount: u32,
	// Ignore `__typename`
}

#[derive(Deserialize, Debug)]
struct Broadcast {
	startedAt: Option<String>,
	// Ignore `id` and `__typename`
}

#[derive(Deserialize, Debug)]
struct ScheduleSegmentGame {
	name: String, // Ignore `id` and `__typename`
}

#[derive(Deserialize, Debug)]
struct ScheduleSegment {
	startAt: String,
	endAt: Option<String>,
	title: String,
	categories: Vec<ScheduleSegmentGame>, // Ignore `id`, `hasReminder` and `__typename`
}

#[derive(Deserialize, Debug)]
struct Schedule {
	nextSegment: Option<ScheduleSegment>, // Ignore `id` and `__typename`
}

#[derive(Deserialize, Debug)]
struct Channel {
	schedule: Option<Schedule>, // Ignore `id` and `__typename`
}

#[derive(Deserialize, Debug)]
struct Video {
	id: String,
	// Up to 48 hours
	lengthSeconds: u32,
	// Ignore `title`, `previewThumbnailURL` and `__typename`
}

#[derive(Deserialize, Debug)]
struct VideoEdge {
	node: Video, // Ignore `__typename`
}

#[derive(Deserialize, Debug)]
struct VideoConnection {
	edges: Vec<VideoEdge>,
	// Ignore `__typrname`
}

#[derive(Deserialize, Debug)]
struct SearchForEdgeClip {
	title: String,
	// Max 60 seconds, fits in u8
	durationSeconds: u8,
	slug: String, // Ignore `id`, `thumbnailURL` and `__typename`
}

#[derive(Deserialize, Debug)]
struct ClipEdge {
	node: SearchForEdgeClip, // Ignore `__typename`
}

#[derive(Deserialize, Debug)]
struct ClipConnection {
	edges: Vec<ClipEdge>, // Ignore `__typename`
}

#[derive(Deserialize, Debug)]
struct SearchForEdgeStream {
	game: Game,
	freeformTags: Vec<FreeformTag>,
	viewersCount: u32, // Ignore `id`, `previewImageURL`, `type` and `__typename`
}

/// [`User`] returned by a search
#[derive(Deserialize, Debug)]
struct SearchForEdgeUser {
	broadcastSettings: BroadcastSettings,
	displayName: String,
	followers: FollowerConnection,
	lastBroadcast: Broadcast,
	login: String,
	description: Option<String>,
	channel: Channel,
	latestVideo: VideoConnection,
	topClip: ClipConnection,
	roles: UserRoles,
	stream: Option<SearchForEdgeStream>,
	// Ignore `id`, `profileImageURL`, `self`, `watchParty` and `__typename`
}

impl SearchForEdgeUser {
	/// Adds this item's info to the given `Vec`
	fn add_items_to(self, items_list: &mut (Vec<Span>, Vec<(Paragraph, Node)>)) {
		items_list.0.push(self.displayName.into());

		let mut lines = vec![
			self.broadcastSettings.title.into(),
			"".into(),
			["Followers: ", &self.followers.totalCount.to_string()]
				.concat()
				.into(),
			[
				"Started: ",
				&self
					.lastBroadcast
					.startedAt
					.as_ref()
					.map_or("Never".to_owned(), |x| format_date(&x)),
			]
			.concat()
			.into(),
			["Partner: ", if self.roles.isPartner { "Yes" } else { "No" }]
				.concat()
				.into(),
		];

		// Add the appropriate items for stream/VOD/nothing
		let node = if self.lastBroadcast.startedAt.is_some() {
			if let Some(stream) = self.stream {
				//They're streaming right now
				lines.extend([
					[
						"Game: ",
						&stream.game.displayName.unwrap_or(stream.game.name),
					]
					.concat()
					.into(),
					["Viewers: ", &stream.viewersCount.to_string()]
						.concat()
						.into(),
					[
						"Tags: ",
						&stream
							.freeformTags
							.iter()
							.map(|tag| tag.name.clone())
							.collect::<Vec<String>>()
							.join(", "),
					]
					.concat()
					.into(),
				]);

				// Their current stream
				self.login.into()
			} else if self.latestVideo.edges.len() == 0 {
				// They have streamed before, but we didn't get a VOD
				Node::None
			} else {
				// They're not currently streaming, show most recent VOD
				lines.extend([
					"".into(),
					"Not currently streaming, you can watch their latest VOD".into(),
					[
						"Length: ",
						&self.latestVideo.edges[0].node.lengthSeconds.to_string(),
						" s",
					]
					.concat()
					.into(),
				]);

				// Their last stream
				Node::Video(self.latestVideo.edges[0].node.id.clone())
			}
		} else {
			// They've never streamed
			Node::None
		};

		if let Some(description) = self.description {
			lines.extend(["".into(), description.into()]);
		}

		if let Some(Schedule {
			nextSegment: Some(next_segment),
		}) = self.channel.schedule
		{
			lines.extend([
				"".into(),
				"Next scheduled stream:".into(),
				next_segment.title.into(),
				["Starts: ", &format_date(&next_segment.startAt)]
					.concat()
					.into(),
				[
					"Ends: ",
					&if let Some(end_at) = &next_segment.endAt {
						format_date(&end_at)
					} else {
						"tbd".to_owned()
					},
				]
				.concat()
				.into(),
				[
					"Categories: ",
					&next_segment
						.categories
						.into_iter()
						.map(|game| game.name.clone())
						.collect::<Vec<String>>()
						.join(", "),
				]
				.concat()
				.into(),
			]);
		}

		items_list
			.1
			.push((Paragraph::new(lines).wrap(Wrap { trim: false }), node));

		// If there is a top clip
		if self.topClip.edges.len() == 1 {
			items_list.0.push("| Top clip".into());

			items_list.1.push((
				Paragraph::new(vec![
					self.topClip.edges[0].node.title.clone().into(),
					"".into(),
					[
						"Duration: ",
						&self.topClip.edges[0].node.durationSeconds.to_string(),
						" s",
					]
					.concat()
					.into(),
				])
				.wrap(Wrap { trim: false }),
				// We just need the slug
				Node::Clip {
					slug: self.topClip.edges[0].node.slug.clone(),
					clipTitle: String::new(),
					clipViewCount: 0,
					curator: User {
						login: String::new(),
						displayName: String::new(),
						primaryColorHex: None,
						broadcastSettings: None,
						roles: None,
					},
					game: Game {
						viewersCount: None,
						name: String::new(),
						displayName: None,
						gameTags: None,
						originalReleaseDate: None,
					},
					broadcaster: User {
						login: String::new(),
						displayName: String::new(),
						primaryColorHex: None,
						broadcastSettings: None,
						roles: None,
					},
					clipCreatedAt: String::new(),
					durationSeconds: 0,
					language: String::new(),
				},
			));
		}
	}
}

#[derive(Deserialize, Debug)]
struct SearchForEdge {
	item: SearchForEdgeUser, // Ignore `trackingID` and `__typename`
}

#[derive(Deserialize, Debug)]
struct SearchForResultUsers {
	edges: Vec<SearchForEdge>,
	// It's from 1 to 5, so would fit in a u8
	// However, we want to use it later for indexing
	score: usize,
	// Max 10,000, so fits in u16
	totalMatches: u16,
	// Ignore `cursor` and `__typename`
}

#[derive(Deserialize, Debug)]
struct SearchForEdgeGame {
	item: Game, // Ignore `trackingID` and `__typename`
}

// Same as `SearchForResultUsers`
#[derive(Deserialize, Debug)]
struct SearchForResultGames {
	edges: Vec<SearchForEdgeGame>,
	// It's from 1 to 5, so would fit in a u8
	// However, we want to use it later for indexing
	score: usize,
	// Max 10,000, so fits in u16
	totalMatches: u16,
	// Ignore `cursor` and `__typename`
}

#[derive(Deserialize, Debug)]
struct SearchForEdgeVideoVideo {
	createdAt: String,
	owner: User,
	id: String,
	game: Game,
	// Up to 2 days
	lengthSeconds: u32,
	title: String,
	viewCount: u32,
	// Ignore `id`, `previewThumbnailURL` and `__typename`
}

#[derive(Deserialize, Debug)]
struct SearchForEdgeVideo {
	item: SearchForEdgeVideoVideo, // Ignore `trackingID` and `__typename`
}

#[derive(Deserialize, Debug)]
struct SearchForResultVideos {
	edges: Vec<SearchForEdgeVideo>,
	// It's from 1 to 5, so would fit in a u8
	// However, we want to use it later for indexing
	score: usize,
	// Max 10,000, so fits in u16
	totalMatches: u16,
	// Ignore `cursor` and `__typename`
}

#[derive(Deserialize, Debug)]
struct SearchForEdgeRelatedLiveChannelsStream {
	viewersCount: u32,
	game: Game,
	broadcaster: User, // Ignore `id`, `previewImageURL` and `__typename`
}

#[derive(Deserialize, Debug)]
struct SearchForEdgeRelatedLiveChannelsUser {
	stream: SearchForEdgeRelatedLiveChannelsStream, // Ignore `id`, `watchParty` and `__typename`
}

#[derive(Deserialize, Debug)]
struct SearchForEdgeRelatedLiveChannels {
	item: SearchForEdgeRelatedLiveChannelsUser, // Ignore `trackingId` and `__typename`
}

#[derive(Deserialize, Debug)]
struct SearchForResultRelatedLiveChannels {
	edges: Vec<SearchForEdgeRelatedLiveChannels>,
	// It's from 1 to 5, so would fit in a u8
	// However, we want to use it later for indexing
	score: usize,
	// Ignore `__typename`
}

#[derive(Deserialize, Debug)]
struct SearchFor {
	channels: SearchForResultUsers,
	channelsWithTag: SearchForResultUsers,
	games: SearchForResultGames,
	videos: SearchForResultVideos,
	relatedLiveChannels: SearchForResultRelatedLiveChannels,
	// Ignore `__typename`
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum Data {
	PersonalSection {
		personalSections: Vec<PersonalSection>,
	},
	Shelves {
		shelves: ShelfConnection,
	},
	Game {
		game: Category,
	},
	SearchFor {
		searchFor: SearchFor,
	},
}

/// Response from the `PersonalSections` API call.
#[derive(Deserialize, Debug)]
pub struct TwitchResponse {
	data: Data,
	// Ignore `extensions`
}

impl TwitchResponse {
	/// Converts the data to a main [`List`] widget and a [`Vec`] of data widgets.
	pub fn to_widgets<'a>(self) -> (List<'a>, Vec<(Paragraph<'a>, Node)>) {
		let mut titles = Vec::new();
		let mut info = Vec::new();

		match self.data {
			Data::PersonalSection { personalSections } => {
				for personal_section in personalSections {
					// Title
					titles.push(ListItem::new(spaced(header(
						personal_section.title.localizedFallback,
					))));

					// No info for title
					info.push((Paragraph::new(Text { lines: Vec::new() }), Node::None));

					// Channels
					for channel in personal_section.items.into_iter() {
						// Item foreground colour
						let style = channel.user.style();

						titles.push(
							ListItem::new(spaced(channel.user.displayName.clone())).style(style),
						);
						info.push((
							Paragraph::new(Text {
								lines: vec![
									channel
										.user
										.broadcastSettings
										.expect("Should be a broadcast")
										.title
										.into(),
									"".into(),
									channel.user.displayName.into(),
									["Viewers: ", &channel.content.viewersCount.to_string()]
										.concat()
										.into(),
									[
										"Game: ",
										&channel
											.content
											.game
											.displayName
											.unwrap_or(channel.content.game.name),
									]
									.concat()
									.into(),
								],
							})
							.style(style)
							.wrap(Wrap { trim: false }),
							channel.user.login.into(),
						));
					}
				}
			}
			Data::Shelves {
				shelves: ShelfConnection { edges },
			} => {
				for edge in edges {
					// Gategory title
					// Use fallback title if any tokens are null
					titles.push(
						if edge
							.node
							.title
							.localizedTitleTokens
							.iter()
							.any(|x| matches!(x.node, TextToken::None))
						{
							// Fallback string
							ListItem::new(spaced(header(edge.node.title.fallbackLocalizedTitle)))
						} else {
							ListItem::new(spaced(Spans(
								edge.node
									.title
									.localizedTitleTokens
									.into_iter()
									.map(|token| {
										match token.node {
											TextToken::BrowsableCollection {
												collectionName:
													BrowsableCollectionTitle {
														fallbackLocalizedTitle,
													},
											} => header(fallbackLocalizedTitle),
											TextToken::Game(Game {
												displayName, name, ..
											}) => header(displayName.unwrap_or(name)),
											TextToken::TextToken { text, hasEmphasis } => Span {
												content: text.into(),
												style: Style {
													add_modifier: if hasEmphasis {
														Modifier::BOLD
													} else {
														Modifier::empty()
													} | Modifier::UNDERLINED,
													..Style::default()
												},
											},
											// We already filtered this out
											TextToken::None => unreachable!(),
										}
									})
									.collect::<Vec<Span>>(),
							)))
						},
					);

					info.push((Paragraph::new(Text { lines: Vec::new() }), Node::None));

					for edge in edge.node.content.edges {
						// Category items
						let (title, lines) = match &edge.node {
							Node::Clip {
								clipTitle,
								clipViewCount,
								curator:
									User {
										displayName: curator_display_name,
										..
									},
								game:
									Game {
										displayName: game_display_name,
										name: game_name,
										..
									},
								broadcaster:
									User {
										displayName: broadcaster_display_name,
										..
									},
								clipCreatedAt,
								durationSeconds,
								language,
								..
							} => (
								clipTitle.clone(),
								vec![
									clipTitle.clone().into(),
									"".into(),
									["Views: ", &clipViewCount.to_string()].concat().into(),
									["Curator: ", &curator_display_name].concat().into(),
									[
										"Game: ",
										&game_display_name.clone().unwrap_or(game_name.clone()),
									]
									.concat()
									.into(),
									["Broadcaster: ", &broadcaster_display_name].concat().into(),
									["Clip created: ", &format_date(clipCreatedAt)]
										.concat()
										.into(),
									["Duration: ", &durationSeconds.to_string(), "s"]
										.concat()
										.into(),
									["Language: ", &language].concat().into(),
								],
							),
							Node::Game(Game {
								viewersCount,
								displayName,
								name,
								gameTags,
								originalReleaseDate,
								..
							}) => {
								let mut lines = vec![
									displayName.clone().unwrap_or(name.clone()).into(),
									"".into(),
								];

								if let Some(viewers_count) = viewersCount {
									lines.push(
										["Viewers: ", &viewers_count.to_string()].concat().into(),
									);
								}

								if let Some(game_tags) = gameTags {
									lines.push(
										[
											"Tags: ",
											&game_tags
												.iter()
												.map(|tag| tag.localizedName.clone())
												.collect::<Vec<String>>()
												.join(", "),
										]
										.concat()
										.into(),
									)
								}

								if let Some(original_release_date) = originalReleaseDate {
									lines.push(
										["Released: ", &format_date(original_release_date)]
											.concat()
											.into(),
									)
								}

								(displayName.clone().unwrap_or(name.clone()).clone(), lines)
							}
							Node::Stream {
								broadcaster:
									User {
										displayName: broadcaster_display_name,
										broadcastSettings: Some(BroadcastSettings { title }),
										..
									},
								game,
								freeformTags,
								viewersCount,
								createdAt,
							} => {
								let mut infos = vec![
									title.clone().into(),
									"".into(),
									broadcaster_display_name.clone().into(),
									[
										"Tags: ",
										&freeformTags
											.iter()
											.map(|tag| tag.name.clone())
											.collect::<Vec<String>>()
											.join(", "),
									]
									.concat()
									.into(),
									["Viewers: ", &viewersCount.to_string()].concat().into(),
								];

								if let Some(
									Game {
										displayName: Some(name),
										..
									}
									| Game { name, .. },
								) = game
								{
									infos.push(["Game: ", &name].concat().into());
								}

								if let Some(created_at) = createdAt {
									infos.push(
										["Created: ", &format_date(created_at)].concat().into(),
									);
								}

								(broadcaster_display_name.clone(), infos)
							}
							_ => continue,
						};

						titles.push(ListItem::new(spaced(title)));
						info.push((
							Paragraph::new(Text { lines }).wrap(Wrap { trim: false }),
							edge.node,
						));
					}
				}
			}
			Data::Game {
				game: Category {
					streams: StreamConnection { edges },
				},
			} => {
				for edge in edges {
					let style = edge.node.broadcaster.style();

					titles.push(
						ListItem::new(spaced(edge.node.broadcaster.displayName.clone()))
							.style(style),
					);
					info.push((
						Paragraph::new(Text {
							lines: vec![
								edge.node.title.into(),
								"".into(),
								edge.node.broadcaster.displayName.into(),
								["Viewers: ", &edge.node.viewersCount.to_string()]
									.concat()
									.into(),
								[
									"Game: ",
									&edge.node.game.displayName.unwrap_or(edge.node.game.name),
								]
								.concat()
								.into(),
								["Created: ", &format_date(&edge.node.createdAt)]
									.concat()
									.into(),
								[
									"Tags: ",
									&edge
										.node
										.freeformTags
										.iter()
										.map(|tag| tag.name.clone())
										.collect::<Vec<String>>()
										.join(", "),
								]
								.concat()
								.into(),
							],
						})
						.style(style)
						.wrap(Wrap { trim: false }),
						edge.node.broadcaster.login.into(),
					));
				}
			}
			Data::SearchFor {
				searchFor:
					SearchFor {
						channels,
						channelsWithTag,
						games,
						videos,
						relatedLiveChannels,
					},
			} => {
				// We need to add these later in the right order, based on score
				// We can't do `[_; 5]` because tuples don't implement `Copy`
				let mut items_to_add = [
					(Vec::new(), Vec::new()),
					(Vec::new(), Vec::new()),
					(Vec::new(), Vec::new()),
					(Vec::new(), Vec::new()),
					(Vec::new(), Vec::new()),
				];

				if channels.edges.len() != 0 {
					items_to_add[channels.score - 1].0.push(header("Channels"));

					items_to_add[channels.score - 1].1.push((
						Paragraph::new(
							["Total matches: ", &channels.totalMatches.to_string()].concat(),
						),
						Node::None,
					));

					for edge in channels.edges {
						edge.item
							.add_items_to(&mut items_to_add[channels.score - 1]);
					}
				}
				if channelsWithTag.edges.len() != 0 {
					items_to_add[channelsWithTag.score - 1]
						.0
						.push(header("Live channels with tag"));

					items_to_add[channelsWithTag.score - 1].1.push((
						Paragraph::new(
							["Total matches: ", &channelsWithTag.totalMatches.to_string()].concat(),
						),
						Node::None,
					));

					for edge in channelsWithTag.edges {
						edge.item
							.add_items_to(&mut items_to_add[channelsWithTag.score - 1]);
					}
				}
				if games.edges.len() != 0 {
					items_to_add[games.score - 1].0.push(header("Categories"));

					items_to_add[games.score - 1].1.push((
						Paragraph::new(
							["Total matches: ", &games.totalMatches.to_string()].concat(),
						),
						Node::None,
					));

					for edge in games.edges {
						items_to_add[games.score - 1].0.push(
							edge.item
								.displayName
								.unwrap_or(edge.item.name.clone())
								.into(),
						);

						let mut lines = Vec::new();

						if let Some(viewers_count) = edge.item.viewersCount {
							lines.push(["Viewers: ", &viewers_count.to_string()].concat().into());
						}

						if let Some(tags) = edge.item.gameTags {
							lines.push(
								[
									"Tags: ",
									&tags
										.into_iter()
										.map(|tag| tag.localizedName)
										.collect::<Vec<String>>()
										.join(", "),
								]
								.concat()
								.into(),
							);
						}

						items_to_add[games.score - 1].1.push((
							Paragraph::new(lines).wrap(Wrap { trim: false }),
							Node::Game(Game {
								viewersCount: None,
								name: edge.item.name,
								displayName: None,
								gameTags: None,
								originalReleaseDate: None,
							}),
						));
					}
				}
				if videos.edges.len() != 0 {
					items_to_add[videos.score - 1].0.push(header("Past videos"));

					items_to_add[videos.score - 1].1.push((
						Paragraph::new(
							["Total matches: ", &videos.totalMatches.to_string()].concat(),
						),
						Node::None,
					));

					for edge in videos.edges {
						items_to_add[videos.score - 1]
							.0
							.push(edge.item.title.into());

						let mut lines = vec![
							edge.item.owner.displayName.into(),
							"".into(),
							["Created: ", &format_date(&edge.item.createdAt)]
								.concat()
								.into(),
							[
								"Game: ",
								&edge.item.game.displayName.unwrap_or(edge.item.game.name),
							]
							.concat()
							.into(),
							["Length: ", &edge.item.lengthSeconds.to_string(), " s"]
								.concat()
								.into(),
							["Views: ", &edge.item.viewCount.to_string()]
								.concat()
								.into(),
						];

						if let Some(roles) = edge.item.owner.roles {
							lines.push(
								["Partner: ", if roles.isPartner { "Yes" } else { "No" }]
									.concat()
									.into(),
							);
						}

						items_to_add[videos.score - 1].1.push((
							Paragraph::new(lines).wrap(Wrap { trim: false }),
							Node::Video(edge.item.id),
						));
					}
				}

				if relatedLiveChannels.edges.len() != 0 {
					items_to_add[relatedLiveChannels.score - 1]
						.0
						.push(header("People searching also watch:"));

					items_to_add[relatedLiveChannels.score - 1]
						.1
						.push((Paragraph::new(""), Node::None));

					for edge in relatedLiveChannels.edges {
						let style = edge.item.stream.broadcaster.style();

						items_to_add[relatedLiveChannels.score - 1].0.push(Span {
							content: edge.item.stream.broadcaster.displayName.into(),
							style,
						});

						let mut lines = Vec::new();

						if let Some(broadcast_settings) =
							edge.item.stream.broadcaster.broadcastSettings
						{
							lines.extend([broadcast_settings.title.into(), "".into()]);
						}

						lines.extend([
							["Viewers: ", &edge.item.stream.viewersCount.to_string()]
								.concat()
								.into(),
							["Game: ", &edge.item.stream.game.name].concat().into(),
						]);

						if let Some(roles) = edge.item.stream.broadcaster.roles {
							lines.push(
								["Partner: ", &if roles.isPartner { "Yes" } else { "No" }]
									.concat()
									.into(),
							);
						}

						items_to_add[relatedLiveChannels.score - 1].1.push((
							Paragraph::new(lines)
								.style(style)
								.wrap(Wrap { trim: false }),
							edge.item.stream.broadcaster.login.into(),
						));
					}
				}

				// Add the sections in score order
				for items in items_to_add {
					titles.extend(items.0.into_iter().map(|span| {
						ListItem::new(spaced(span.clone())).style(Style {
							fg: span.style.fg,
							..Style::default()
						})
					}));
					info.extend(items.1);
				}
			}
		}

		(
			List::new(titles).highlight_style(Style {
				add_modifier: Modifier::REVERSED,
				..Style::default()
			}),
			info,
		)
	}
}
