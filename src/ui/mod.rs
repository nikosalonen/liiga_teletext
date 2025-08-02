//! User Interface module for the liiga_teletext application
//!
//! This module contains all UI-related functionality including dynamic layout calculation,
//! content adaptation, resize handling, and the main interactive UI loop.
//!
//! ## Modules
//!
//! - [`content_adapter`]: Adapts game content to different detail levels based on screen space
//! - [`interactive`]: Main interactive UI loop with user input handling
//! - [`layout`]: Dynamic layout calculation with caching and performance optimizations
//! - [`resize`]: Terminal resize detection and debouncing
//!
//! ## Dynamic UI Features
//!
//! The UI system automatically adapts to different terminal sizes:
//!
//! - **Minimal Detail** (< 100 chars wide): Basic game information
//! - **Standard Detail** (100-119 chars wide): Enhanced information with more details
//! - **Extended Detail** (≥ 120 chars wide): Full information with complete details
//!
//! ## Performance Optimizations
//!
//! - Layout calculations are cached with TTL to avoid redundant work
//! - Incremental updates for small terminal size changes
//! - String buffer pooling to reduce memory allocations
//! - Automatic cache cleanup to prevent memory growth
//!
//! ## Usage Example
//!
//! ```rust
//! use crate::ui::{LayoutCalculator, DetailLevel, ContentAdapter};
//!
//! // Create layout calculator
//! let mut calculator = LayoutCalculator::new();
//! let layout = calculator.calculate_layout((120, 40));
//!
//! // Adapt content based on layout
//! let adapter = ContentAdapter::new();
//! let content = adapter.adapt_game_content(&game_data, layout.detail_level, layout.content_width);
//! ```

pub mod content_adapter;
pub mod interactive;
pub mod layout;
pub mod resize;

pub use content_adapter::ContentAdapter;
pub use layout::{ContentPositioning, DetailLevel, LayoutCalculator, LayoutConfig};
