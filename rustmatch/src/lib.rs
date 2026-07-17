//! Rust-native many-pattern matching with explicit UTF-16 coordinates.
//!
//! rustmatch builds an immutable matcher from caller-identified patterns and
//! can reuse it across inputs. The current executable slice intentionally
//! accepts only non-empty 7-bit ASCII literal patterns and ASCII input. This
//! narrow language already runs through the final-shaped parser, HIR, shared
//! NFA, database, scan engine, and callback boundaries.
//!
//! # Example
//!
//! ```
//! use rustmatch::{MatcherBuilder, PatternId, Utf16Text};
//!
//! let mut builder = MatcherBuilder::new();
//! builder.add(PatternId::new(1), "cat")?;
//! let matcher = builder.build()?;
//! let input = Utf16Text::from("a cat");
//! let mut spans = Vec::new();
//!
//! matcher.scan(&input, |matched| spans.push(matched.span()))?;
//!
//! assert_eq!(spans[0].start(), 2);
//! assert_eq!(spans[0].end(), 5);
//! # Ok::<(), rustmatch::Error>(())
//! ```

#![forbid(unsafe_code)]

mod api;
mod engine;
mod error;
mod hir;
mod nfa;
mod parser;
mod types;

pub use api::{Matcher, MatcherBuilder};
pub use error::Error;
pub use types::{Match, PatternId, Utf16Span, Utf16Text};
