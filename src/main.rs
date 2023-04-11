#![feature(exclusive_range_pattern)]

use std::io::{stdout, Read};

use config::*;
use crossterm::event::{read, Event, KeyCode, KeyEvent};
use crossterm::execute;
use crossterm::terminal::{
	disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use curl::easy::{self, Easy};

mod config;
mod structs;
use std::panic::{set_hook, take_hook};

use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Rect};
use ratatui::widgets::{Block, Borders, List, ListState, Paragraph};
use ratatui::Terminal;
use serde::Serialize;
use simd_json::{from_slice, to_vec};
use structs::*;

/// Current page + information on previous pages
enum Page {
	/// Home page, where the program starts
	Home {
		/// Where the cursor is
		selection: usize,
	},
	/// A category
	Game {
		name: String,
		selection: usize,
		/// Previous page, needs to be on heap to avoid recursive type
		previous: Box<Page>,
	},
	/// Search page
	Search {
		query: String,
		selection: usize,
		previous: Box<Page>,
	},
}
impl Page {
	/// Sends this page's request and returns the ratatui widgets.
	fn request<'a>(&self, easy: &mut Easy) -> (List<'a>, Vec<(Paragraph<'a>, Node)>) {
		from_slice::<TwitchResponse>(&mut match self {
			Page::Home { .. } => match HOME_PAGE {
				HomePage::PersonalSection => {
					request(easy, &TwitchRequest::<PersonalSectionsVariables>::default())
				}
				HomePage::Shelves => request(easy, &TwitchRequest::<ShelvesVariables>::default()),
				HomePage::Game(name) => request(
					easy,
					&TwitchRequest {
						variables: DirectoryPage_GameVariables {
							name: name.to_owned(),
							..TwitchRequest::default().variables
						},
						..TwitchRequest::default()
					},
				),
				HomePage::Search(query) => request(
					easy,
					&TwitchRequest {
						variables: SearchResultsVariables {
							query: query.to_owned(),
							..TwitchRequest::default().variables
						},
						..TwitchRequest::default()
					},
				),
			},

			Page::Game { name, .. } => request(
				easy,
				&TwitchRequest {
					variables: DirectoryPage_GameVariables {
						name: name.clone(),
						..TwitchRequest::default().variables
					},
					..TwitchRequest::default()
				},
			),
			Page::Search { query, .. } => request(
				easy,
				&TwitchRequest {
					variables: SearchResultsVariables {
						query: query.clone(),
						..TwitchRequest::default().variables
					},
					..TwitchRequest::default()
				},
			),
		})
		.expect("Response should be valid JSON")
		.to_widgets()
	}

	/// Selects the given item and returns `self`
	fn set_selection(mut self, s: usize) -> Self {
		let (Page::Home { ref mut selection }
		| Page::Game {
			ref mut selection, ..
		}
		| Page::Search {
			ref mut selection, ..
		}) = self;
		*selection = s;

		self
	}

	/// Returns the selected item
	fn get_selection(&self) -> usize {
		let (Page::Home { selection }
		| Page::Game { selection, .. }
		| Page::Search { selection, .. }) = self;

		*selection
	}
}
impl ToString for Page {
	/// Get this page's title
	fn to_string(&self) -> String {
		match self {
			Page::Home { .. } => "Home".to_owned(),
			Page::Game { name, .. } => name.clone(),
			Page::Search { query, .. } => query.clone(),
		}
	}
}

/// Send a request and return it as a `Vec<u8>`.
fn request<J: Serialize + ?Sized>(easy: &mut Easy, json: &J) -> Vec<u8> {
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

		transfer.perform().unwrap();
	}

	vec
}

fn main() {
	// Default to ["best"]
	let mut qualities = if QUALITY.len() == 0 {
		vec!["best"]
	} else {
		let mut vec = Vec::with_capacity(QUALITY.len());
		QUALITY.clone_into(&mut vec);

		vec
	};

	let mut easy = Easy::new();
	let _ = easy.url("https://gql.twitch.tv/gql");
	let _ = easy.post(true);

	let mut easy_list = easy::List::new();
	for header in HEADERS {
		let _ = easy_list.append(header);
	}
	let _ = easy.http_headers(easy_list);

	let hook = take_hook();
	// Run cleanup code on panic
	set_hook(Box::new(move |panic_info| {
		let _ = disable_raw_mode();
		let _ = execute!(stdout(), LeaveAlternateScreen);
		hook(panic_info);
	}));

	let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))
		.expect("Should be able to initialize terminal");

	if DOWNLOAD_PROGRESS {
		// Display download progress
		let _ = easy.progress(true);
		let _ = easy.progress_function(|_, downloaded, _, _| {
			// POST requests don't return `Content-length`, so we can't know how long it will be
			print!("\r{} KiB recieved", downloaded as u32 / 1024);

			true
		});
	}

	let mut page = Page::Home { selection: 0 };

	// Fetch data
	let (mut list, mut info_vec) = page.request(&mut easy);

	// Init crossterm
	let _ = enable_raw_mode();

	let _ = execute!(stdout(), EnterAlternateScreen);

	// Clear screen
	let _ = terminal.clear();

	// Should we redraw this frame?
	let mut redraw = true;

	let mut list_state = ListState::default();
	list_state.select(Some(0));

	loop {
		// If something changed, redraw
		if redraw {
			let _ = terminal.draw(|frame| {
				// Left panel border
				frame.render_widget(
					Block::default()
						.title(page.to_string())
						.borders(Borders::ALL)
						.title_alignment(TITLE_ALIGNMENT)
						.border_type(BORDER_TYPE),
					Rect {
						width: frame.size().width / 2,
						..frame.size()
					},
				);
				// Left panel list
				frame.render_stateful_widget_reusable(
					&list,
					Rect {
						x: 2,
						y: 2,
						width: frame.size().width / 2 - 4,
						height: frame.size().height - 3,
					},
					&mut list_state,
				);

				// Right panel border
				frame.render_widget(
					Block::default()
						.borders(Borders::ALL)
						.title_alignment(TITLE_ALIGNMENT)
						.border_type(BORDER_TYPE),
					Rect {
						x: frame.size().width / 2,
						width: (frame.size().width + 1) / 2,
						..frame.size()
					},
				);
				// Top-right panel text
				frame.render_widget_reusable(
					&info_vec[list_state.selected().expect("Something should be selected")].0,
					Rect {
						x: frame.size().width / 2 + 2,
						y: 2,
						width: (frame.size().width - 7) / 2,
						height: frame.size().height - 4,
					},
				);

				// Bottom-right panel text
				frame.render_widget(
					Paragraph::new(vec![
						"back: b".into(),
						"search: s".into(),
						"refresh: r".into(),
						"quit: q".into(),
						"".into(),
						"quality: +-".into(),
						qualities[0].into(),
					])
					.alignment(Alignment::Right),
					Rect {
						x: frame.size().width / 2 + 2,
						y: frame.size().height - 9,
						width: (frame.size().width - 7) / 2,
						height: 7,
					},
				);
			});
		}

		redraw = true;

		// Read input
		match read().expect("IO error") {
			Event::Key(KeyEvent { code, .. }) => match code {
				// Quit
				KeyCode::Char('Q' | 'q') => break,
				// Move down
				KeyCode::Down | KeyCode::Char('J' | 'j') => {
					list_state.select(list_state.selected().map(|s| info_vec.len().min(s + 2) - 1))
				}
				// Move up
				KeyCode::Up | KeyCode::Char('K' | 'k') => {
					list_state.select(list_state.selected().map(|s| s.saturating_sub(1)))
				}
				KeyCode::PageDown => list_state.select(list_state.selected().map(|s| {
					info_vec.len().min(
						s + (terminal
							.size()
							.expect("Should be able to get terminal height")
							.height / 2) as usize,
					) - 1
				})),
				KeyCode::PageUp => list_state.select(list_state.selected().map(|s| {
					s.saturating_sub(
						(terminal
							.size()
							.expect("Should be able to get terminal height")
							.height / 2 - 1) as usize,
					)
				})),
				KeyCode::Right | KeyCode::Char('L' | 'l') => {
					// Enter
					if let Some(name) = info_vec
						[list_state.selected().expect("Something should be selected")]
					.1
					.select(&mut easy, &*qualities)
					{
						// If we selected a category

						// selection doesn't matter yet
						page = Page::Game {
							name,
							selection: 0,
							previous: Box::new(page.set_selection(
								list_state.selected().expect("Something should be selected"),
							)),
						};

						// Move cursor to the top
						list_state.select(Some(0));

						(list, info_vec) = page.request(&mut easy);
					}

					let _ = terminal.clear();
				}
				// Go back
				KeyCode::Left | KeyCode::Char('B' | 'b') => {
					match page {
						// Just move cursor to the top
						Page::Home { .. } => list_state.select(Some(0)),
						Page::Game { previous, .. } | Page::Search { previous, .. } => {
							page = *previous;
							(list, info_vec) = page.request(&mut easy);

							let _ = terminal.clear();

							list_state.select(Some(page.get_selection().min(info_vec.len() - 1)));
						}
					}
				}
				// home
				KeyCode::Char('H' | 'h') => {
					// Move cursor to the top
					list_state.select(Some(0));

					page = Page::Home { selection: 0 };
					(list, info_vec) = page.request(&mut easy);

					let _ = terminal.clear();
				}
				// Increase quality
				KeyCode::Char('+') => {
					qualities[0] = match qualities[0] {
						"audio_only" => "worst",
						"worst" => "160p",
						"160p" => "360p",
						"360p" => "480p",
						"480p" => "720p",
						"720p" => "720p60",
						"720p60" => "1080p60",
						"1080p60" => "best",
						_ => qualities[0],
					}
				}
				// Decrease quality
				KeyCode::Char('-') => {
					qualities[0] = match qualities[0] {
						"worst" => "audio_only",
						"160p" => "worst",
						"360p" => "160p",
						"480p" => "360p",
						"720p" => "480p",
						"720p60" => "720p",
						"1080p60" => "720p60",
						"best" => "1080p60",
						_ => qualities[0],
					}
				}
				// Search
				KeyCode::Char('S' | 's' | '/') => {
					// Show cursor
					let _ = terminal.show_cursor();

					let mut query = String::new();

					loop {
						let _ = terminal.draw(|frame| {
							// Width of the input box
							let width = (query.len() as u16 + 3).clamp(20, frame.size().width);

							frame.render_widget(
								Paragraph::new(query.clone()).block(
									Block::default()
										.borders(Borders::ALL)
										.title("Search for streams")
										.title_alignment(TITLE_ALIGNMENT)
										.border_type(BORDER_TYPE),
								),
								Rect {
									x: (frame.size().width - width) / 2,
									y: frame.size().height / 2 - 1,
									width,
									height: 3,
								},
							)
						});

						if let Event::Key(KeyEvent { code, .. }) =
							read().expect("Should be able to read input")
						{
							match code {
								KeyCode::Char(c) => query.push(c),
								KeyCode::Backspace => {
									query.pop();
								}
								KeyCode::Enter => break,
								_ => (),
							}
						}
					}

					list_state.select(Some(0));

					page = Page::Search {
						query,
						selection: 0,
						previous: Box::new(page.set_selection(
							list_state.selected().expect("Something should be selected"),
						)),
					};

					(list, info_vec) = page.request(&mut easy);

					let _ = terminal.clear();

					// Hide the cursor again
					let _ = terminal.hide_cursor();
				}
				// Refresh
				KeyCode::Char('R' | 'r') => {
					// Just send this page's request again and parse it
					(list, info_vec) = page.request(&mut easy);

					// Make sure the cursor isn't past the end of the data
					list_state.select(list_state.selected().map(|s| s.min(info_vec.len() - 1)));

					let _ = terminal.clear();
				}
				_ => redraw = false,
			},
			// We want to redraw
			Event::Resize(..) => (),
			_ => redraw = false,
		}
	}

	let _ = disable_raw_mode();
	let _ = execute!(stdout(), LeaveAlternateScreen);
}
