// For some enum variants
#![allow(dead_code)]

use ratatui::layout::Alignment;
use ratatui::widgets::BorderType;

use crate::structs::*;

/// Program and args used to play videos and streams
pub const PLAYER: &[&str] = &["ffplay", "-autoexit"];

/// Quality of the streams/videos played, first item is prioritised.
/// The first item can be changed at runtime with +/-.
/// Case-insensitive (lower case) for clips and VODs.
/// If there are no items it will default to `best`.
/// Should be one of: audio_only, worst, 160p, 360p, 480p, 720p, 720p60, 1080p60, best
pub const QUALITY: &[&str] = &["best"];

/// HTTP headers for requests.
pub const HEADERS: &[&str] = &[
	// This is required, this ID is from the webapp
	"Client-Id:kimne78kx3ncx6brgo4mv6wki5h1ko",
	// This header is required for some requests, it can be anything
	"X-Device-Id:A",
	// The language-locale for recommendations and title localization
	// You can find more info
	// [on mozilla's docs](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Accept-Language)
	"Accept-Language:en",
	// You can add more, but they probably won't have any effect
];

/// Show download progress?
pub const DOWNLOAD_PROGRESS: bool = true;

/// The request used for the home page.
/// Usually either `Shelves` (the main home page) or `PersonalSection` (The bit on the left on the
/// webapp). It could also be a category (`Game("Just Chatting")`) or a search (`Search("Lol")`).
///
/// I recommend setting this to `PersonalSection` if you don't usually use the home page or you
/// want quicker load times, since it's only ~9kb, and `Shelves` is ~1mb (~100x larger).
pub const HOME_PAGE: HomePage = HomePage::PersonalSection;

/// How to display dates.
/// `None` means to show a relative date (i.e. "18 hours ago"),
/// You can use i.e. `Some("%c")` to show an absolute date with the specified format.
/// You can see documentation for this formatting [here](https://docs.rs/chrono/latest/chrono/format/strftime/index.html)
pub const DATE_FORMAT: Option<&str> = None;

// ----------------
// The following settings are for the program's style.
// ----------------

/// Where the title is at the top of the screen.
/// Can be `Left`, `Center` or `Right`.
pub const TITLE_ALIGNMENT: Alignment = Alignment::Left;

/// The style of the UI's borders.
/// Can be `Plain`, `Thick`, `Double` or `Rounded`.
pub const BORDER_TYPE: BorderType = BorderType::Plain;

// ----------------
// The following settings are for API request options, changing some of these could cause the
// server to return errors, which may cause this program to panic. Edit them at your own risk.
// ----------------

// These 3 are for the reccommended section (The column on the left on the twitch website), which
// isn't fetched by default. You can set it above.

impl Default for RecommendationContext {
	fn default() -> Self {
		// Most of these fields are `Option<&str>`
		// If you want to set one, use `Some("foo")`
		Self {
			platform: None,
			clientApp: None,
			location: None,
			referrerDomain: None,
			// These two are `Option<u16>`
			// You should use `Some(69)`
			viewportHeight: None,
			viewportWidth: None,
			channelName: None,
			categoryName: None,
			lastChannelName: None,
			lastCategoryName: None,
			pageviewContent: None,
			pageviewContentType: None,
			pageviewLocation: None,
			pageviewMedium: None,
			previousPageviewContent: None,
			previousPageviewContentType: None,
			previousPageviewLocation: None,
			previousPageviewMedium: None,
		}
	}
}

enum PersonalSectionType {
	RecommendedSection,
	SimilarSection,
}
impl Into<&str> for PersonalSectionType {
	fn into(self) -> &'static str {
		match self {
			PersonalSectionType::RecommendedSection => "RECOMMENDED_SECTION",
			PersonalSectionType::SimilarSection => "SIMILAR_SECTION",
		}
	}
}

impl Default for PersonalSectionsInput {
	fn default() -> Self {
		// `sectionInputs` is the `Vec` of sections in the personal section
		Self {
			sectionInputs: vec![PersonalSectionType::RecommendedSection.into()],
			recommendationContext: RecommendationContext::default(),
			// Add `PersonalSectionType::SimilarSection.into()` to `sectionInputs` if you want to
			// use this.
			// `Some("someone")`
			contextChannelName: None,
		}
	}
}

impl Default for PersonalSectionsVariables {
	fn default() -> Self {
		// I have no clue what `creatorAnniversariesExperimentEnabled` is.
		Self {
			input: PersonalSectionsInput::default(),
			creatorAnniversariesExperimentEnabled: false,
		}
	}
}

// The next 2 are for the main home page items, enabled by default

impl Default for ShelvesContext {
	fn default() -> Self {
		// If you want to use this, you'll need to set `context: Some(ShelvesContext::default())` in
		// `ShelvesVariables`  below
		Self {
			clientApp: None,
			location: None,
			referrerDomain: None,
			viewportHeight: None,
			viewportWidth: None,
		}
	}
}

impl Default for ShelvesVariables {
	fn default() -> Self {
		// Examples in comments:
		Self {
			// `Some(69)`
			imageWidth: None,
			itemsPerRow: 0,
			// `Some(true)`
			langWeightedCCU: None,
			platform: "",
			requestID: "",
			// `Some(ShelvesContext::default())`
			context: None,
			// `Some(true)`
			verbose: None,
		}
	}
}

// The next 2 are for categories, i.e. a game page

enum StreamSort {
	Relevance,
	ViewerCount,
}
impl Into<&str> for StreamSort {
	fn into(self) -> &'static str {
		match self {
			StreamSort::Relevance => "RELEVANCE",
			StreamSort::ViewerCount => "VIEWER_COUNT",
		}
	}
}

impl Default for DirectoryPage_GameOptions {
	fn default() -> Self {
		// Category section options.
		// Alternate examples in comments:
		Self {
			// `StreamSort::ViewerCount.into()`
			sort: StreamSort::Relevance.into(),
			// `Some(RecommendationContext::default())`
			recommendationsContext: None,
			// `Some("foo")`
			requestID: None,
			// `Some(vec!["English"])`
			freeformTags: None,
			// `Some(vec!["English"])`
			tags: None,
		}
	}
}

impl Default for DirectoryPage_GameVariables {
	fn default() -> Self {
		// Category section.
		// Alternate examples in comments:
		Self {
			// `Some(69)` / None
			// Needs to be `Some` to get colour.
			imageWidth: Some(0),
			// This will be set by the program
			name: "".into(),
			// `DirectoryPage_GameOptions { freeformTags: Some(vec!["English"]), .. }`
			options: DirectoryPage_GameOptions::default(),
			// `false`
			sortTypeIsRecency: true,
			// `69`
			limit: 30,
		}
	}
}

enum SearchIndex {
	// Live channels with query as a tag
	ChannelWithTag,
	// Live channels that match the search
	Channel,
	// Category matching query
	Game,
	// Past videos
	Vod,
}
impl Into<&str> for SearchIndex {
	fn into(self) -> &'static str {
		match self {
			SearchIndex::ChannelWithTag => "CHANNEL_WITH_TAG",
			SearchIndex::Channel => "CHANNEL",
			SearchIndex::Game => "GAME",
			SearchIndex::Vod => "VOD",
		}
	}
}

// The next 2 are for the search page

impl Default for SearchResultsPage_SearchResultsOptions {
	fn default() -> Self {
		// Filter search to results of certain types (see `SearchIndex` above)
		// To use this, you neeed to set
		// `options: Some(SearchResultsPage_SearchResultsOptions::default())`
		// in `SearchResultsPage_SearchResultsVariables` below.
		Self {
			// `Some(vec![Target { index: SearchIndex::ChannelWithTag.into() },])`
			targets: None,
		}
	}
}

impl Default for SearchResultsVariables {
	fn default() -> Self {
		// Search results
		// Alternate examples in comments
		Self {
			// This is set by the program
			query: "".into(),
			// `Some(SearchResultsPage_SearchResultsOptions::default())`
			options: None,
			// `Some("lol")`
			requestID: None,
		}
	}
}

// The next 1 is for VODs

impl Default for PlaybackAccessTokenVariables {
	fn default() -> Self {
		// Used for VODs
		Self {
			// Doesn't matter, but `false` is slightly more performant
			isLive: false,
			// Must be `true`
			isVod: true,
			login: "",
			playerType: "",
			// Set by the program
			vodID: String::new(),
		}
	}
}
