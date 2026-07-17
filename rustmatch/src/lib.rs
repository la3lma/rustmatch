//! Rust-native many-pattern matching with explicit UTF-16 coordinates.
//!
//! rustmatch builds an immutable matcher from caller-identified patterns and
//! can reuse it across inputs. The executable semantic spine accepts non-empty
//! patterns composed of UTF-16 literals, dot, character classes, ranges,
//! escapes, ASCII shorthand classes, alternation, grouping without capture
//! semantics, greedy repetition, prefix or typed case-insensitive flags, line
//! anchors, and ASCII word boundaries. Inputs and predicates cover exact UTF-16
//! code units, including isolated surrogates. Unary and counted quantifiers
//! bind to the immediately preceding atom or group, and counted bounds are
//! capped at 1,000. This language runs through the parser, HIR, shared NFA,
//! immutable database, scan engine, and callback boundaries.
//! Assertion-free scans lazily cache deterministic state sets within a bounded,
//! scan-local budget; callers can lower that budget or set it to zero to use
//! the exact NFA path. Cache pressure never changes matching semantics.
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
mod case_fold;
mod engine;
mod error;
mod hir;
mod nfa;
mod parser;
mod predicate;
mod types;

#[cfg(feature = "benchmark-internals")]
#[doc(hidden)]
pub use api::ScanDiagnostics;
pub use api::{Matcher, MatcherBuilder};
pub use error::Error;
pub use types::{Match, PatternFlags, PatternId, Utf16Span, Utf16Text};
