//! Public domain values used at the crate boundary.

use std::fmt;
use std::ops::{BitOr, BitOrAssign};

/// Compile-time options applied to one registered pattern.
///
/// Flags affect only the registration that receives them. [`Self::NONE`]
/// preserves ordinary matching, while [`Self::CASE_INSENSITIVE`] applies the
/// Java-compatible single-UTF-16-unit case mapping at compile time.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct PatternFlags(u8);

impl PatternFlags {
    /// No optional pattern behavior.
    pub const NONE: Self = Self(0);

    /// Match literals and character classes without regard to single-unit case.
    pub const CASE_INSENSITIVE: Self = Self(1);

    pub(crate) const fn is_case_insensitive(self) -> bool {
        self.0 & Self::CASE_INSENSITIVE.0 != 0
    }
}

impl BitOr for PatternFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for PatternFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// Stable caller-provided identity for one registered pattern.
///
/// Pattern IDs are returned with matches and are independent of pattern text.
/// The same text may be registered under different IDs in one matcher.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PatternId(u32);

impl PatternId {
    /// Creates an ID from its application-defined numeric value.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the application-defined numeric value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl fmt::Display for PatternId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Zero-based, half-open positions measured in UTF-16 code units.
///
/// A span contains `start` and excludes `end`, so its length is `end - start`.
/// The engine only constructs spans for which `start <= end`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Utf16Span {
    start: u64,
    end: u64,
}

impl Utf16Span {
    pub(crate) fn from_bounds(start: u64, end: u64) -> Self {
        debug_assert!(start <= end);
        Self { start, end }
    }

    /// Returns the included start position in UTF-16 code units.
    #[must_use]
    pub const fn start(self) -> u64 {
        self.start
    }

    /// Returns the excluded end position in UTF-16 code units.
    #[must_use]
    pub const fn end(self) -> u64 {
        self.end
    }

    /// Returns the span length in UTF-16 code units.
    #[must_use]
    pub const fn len(self) -> u64 {
        self.end - self.start
    }

    /// Returns whether the span consumes no code units.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

/// One reported pattern match.
///
/// Callback order is unspecified. A match is identified by its caller-provided
/// pattern ID and its half-open UTF-16 span.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Match {
    pattern_id: PatternId,
    span: Utf16Span,
}

impl Match {
    pub(crate) const fn new(pattern_id: PatternId, span: Utf16Span) -> Self {
        Self { pattern_id, span }
    }

    /// Returns the identity supplied when the pattern was registered.
    #[must_use]
    pub const fn pattern_id(self) -> PatternId {
        self.pattern_id
    }

    /// Returns the zero-based, half-open UTF-16 match span.
    #[must_use]
    pub const fn span(self) -> Utf16Span {
        self.span
    }
}

/// Finite text materialized as owned UTF-16 code units.
///
/// Constructing this value from a Rust string performs the UTF-8 to UTF-16
/// encoding once, before scanning. Raw construction also admits isolated
/// surrogate code units for exact compatibility and specialized inputs.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Utf16Text {
    units: Vec<u16>,
}

impl Utf16Text {
    /// Creates finite input from its exact UTF-16 code units.
    ///
    /// No Unicode-scalar validation is performed: isolated surrogates are
    /// valid engine symbols, matching the Java `char` model.
    #[must_use]
    pub fn from_units(units: Vec<u16>) -> Self {
        Self { units }
    }

    /// Returns the exact UTF-16 code units scanned by the matcher.
    #[must_use]
    pub fn as_units(&self) -> &[u16] {
        &self.units
    }

    pub(crate) fn units(&self) -> &[u16] {
        &self.units
    }

    /// Returns whether the input contains no UTF-16 code units.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }
}

impl From<&str> for Utf16Text {
    fn from(text: &str) -> Self {
        Self {
            units: text.encode_utf16().collect(),
        }
    }
}

impl From<String> for Utf16Text {
    fn from(text: String) -> Self {
        Self::from(text.as_str())
    }
}

impl From<Vec<u16>> for Utf16Text {
    fn from(units: Vec<u16>) -> Self {
        Self::from_units(units)
    }
}
