//! Finnish Hockey League (Liiga) Teletext Viewer Library
//!
//! This library provides functionality for fetching and displaying Finnish Hockey League
//! game data in a teletext-style format.
//!
//! # Examples
//!
//! ```rust,no_run
//! use liiga_teletext::data_fetcher::api::fetch_liiga_data;
//! use liiga_teletext::teletext_ui::{TeletextPage, GameResultData};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Fetch game data
//!     let (games, date) = fetch_liiga_data(Some("2024-01-15".to_string())).await?;
//!
//!     // Create teletext page
//!     let mut page = TeletextPage::new(
//!         221,
//!         "JÄÄKIEKKO".to_string(),
//!         "RUNKOSARJA".to_string(),
//!         false,
//!         true,
//!         false,
//!     );
//!
//!     // Add games to the page
//!     for game in &games {
//!         page.add_game_result(GameResultData::new(game));
//!     }
//!
//!     // Render the page to stdout
//!     let mut stdout = std::io::stdout();
//!     page.render(&mut stdout)?;
//!
//!     Ok(())
//! }
//! ```

pub mod config;
pub mod data_fetcher;
pub mod error;
pub mod teletext_ui;

// Re-export commonly used types for convenience
pub use config::Config;
pub use data_fetcher::api::fetch_liiga_data;
pub use data_fetcher::models::{DetailedGameResponse, GameData, ScheduleResponse};
pub use error::AppError;
pub use teletext_ui::{GameResultData, TeletextPage};

/// Current version of the library
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Library name
pub const NAME: &str = env!("CARGO_PKG_NAME");
