//! Explicit assertion-prefix specialization for narrowly eligible pattern sets.
//!
//! This module is available only with the non-default
//! `unstable-assertion-prefix-v1` feature. Constructing its separate matcher is
//! a deliberate opt-in; enabling the feature alone does not change
//! [`crate::Matcher`] or [`crate::MatcherBuilder`].

use std::error;
use std::fmt;

use crate::api::SharedCohortMatcher;
use crate::prefilter::{
    AssertionPrefixDecision, MIN_LITERAL_INPUT_UNITS, MIN_LITERAL_PATTERN_COUNT,
};
use crate::{Error, Match, MatcherBuilder, PatternFlags, PatternId, Utf16Text};

/// A statically proven v1 assertion-prefix capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticEligibility {
    assertion_pattern_count: usize,
    shared_ascii_prefix: [u8; 5],
}

impl StaticEligibility {
    /// Returns the number of assertion-bearing patterns covered by the proof.
    #[must_use]
    pub const fn assertion_pattern_count(self) -> usize {
        self.assertion_pattern_count
    }

    /// Returns the shared five-unit ASCII prefix used to discover candidates.
    #[must_use]
    pub const fn shared_ascii_prefix(self) -> [u8; 5] {
        self.shared_ascii_prefix
    }

    /// Returns the minimum assertion-bearing pattern count required by v1.
    #[must_use]
    pub const fn minimum_assertion_pattern_count() -> usize {
        MIN_LITERAL_PATTERN_COUNT
    }
}

/// Why a pattern set cannot construct the v1 matcher.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum StaticIneligibility {
    /// No registered pattern uses an assertion.
    NoAssertionBearingPatterns,
    /// The assertion-bearing cohort is below the frozen v1 minimum.
    TooFewAssertionBearingPatterns {
        /// Number of assertion-bearing patterns that were registered.
        found: usize,
        /// Minimum number required by this version.
        minimum: usize,
    },
    /// The assertion-bearing patterns do not prove one shared five-unit ASCII prefix.
    NoSharedFiveUnitAsciiPrefix,
}

impl fmt::Display for StaticIneligibility {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAssertionBearingPatterns => {
                formatter.write_str("the pattern set has no assertion-bearing cohort")
            }
            Self::TooFewAssertionBearingPatterns { found, minimum } => write!(
                formatter,
                "the assertion-bearing cohort contains {found} patterns; v1 requires at least {minimum}"
            ),
            Self::NoSharedFiveUnitAsciiPrefix => formatter.write_str(
                "the assertion-bearing cohort does not prove one shared five-unit ASCII prefix",
            ),
        }
    }
}

/// Failure while registering or compiling an experimental matcher.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BuildError {
    /// An ordinary rustmatch registration or compilation check failed.
    Matcher(Error),
    /// The pattern set cannot use the versioned specialization.
    Ineligible(StaticIneligibility),
}

impl fmt::Display for BuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Matcher(error) => error.fmt(formatter),
            Self::Ineligible(reason) => write!(formatter, "pattern set is ineligible: {reason}"),
        }
    }
}

impl error::Error for BuildError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Matcher(error) => Some(error),
            Self::Ineligible(_) => None,
        }
    }
}

impl From<Error> for BuildError {
    fn from(error: Error) -> Self {
        Self::Matcher(error)
    }
}

/// Caller-selected behavior when the input cannot activate specialization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ScanPolicy {
    /// Refuse the scan before delivering callbacks unless v1 will activate.
    RequireSpecialized,
    /// Preserve exact behavior through the generic scanner and report why.
    AllowExactFallback,
}

/// The exact reason v1 did not activate for one input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FallbackReason {
    /// The input is shorter than the frozen v1 minimum.
    InputTooShort {
        /// Actual input length in UTF-16 code units.
        input_units: usize,
        /// Minimum input length required by this version.
        minimum_input_units: usize,
    },
    /// At least half of the frozen 64 Ki-unit sample consisted of candidates.
    CandidateSampleTooDense,
    /// Candidate acceleration was disabled by the internal configuration.
    AccelerationDisabled,
    /// Literal candidate acceleration was disabled by the internal configuration.
    LiteralAccelerationDisabled,
}

/// Backend that produced the reported match events.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ScanBackend {
    /// The versioned assertion-prefix candidate scanner activated.
    AssertionPrefixV1,
    /// The generic exact assertion scanner handled the input.
    GenericExactFallback,
}

/// Failure while scanning with an explicit policy.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ScanError {
    /// The exact engine rejected the input.
    Matcher(Error),
    /// `RequireSpecialized` could not activate for this input.
    SpecializationUnavailable {
        /// Exact dynamic reason for refusing before callback delivery.
        reason: FallbackReason,
    },
}

impl fmt::Display for ScanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Matcher(error) => error.fmt(formatter),
            Self::SpecializationUnavailable { reason } => {
                write!(
                    formatter,
                    "assertion-prefix specialization is unavailable: {reason:?}"
                )
            }
        }
    }
}

impl error::Error for ScanError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Matcher(error) => Some(error),
            Self::SpecializationUnavailable { .. } => None,
        }
    }
}

impl From<Error> for ScanError {
    fn from(error: Error) -> Self {
        Self::Matcher(error)
    }
}

/// Observable outcome of one explicitly policy-controlled scan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScanReport {
    policy: ScanPolicy,
    eligibility: StaticEligibility,
    backend: ScanBackend,
    fallback_reason: Option<FallbackReason>,
    candidate_count: usize,
    candidate_bytes: usize,
}

impl ScanReport {
    /// Returns the policy named by the caller for this scan.
    #[must_use]
    pub const fn requested_policy(self) -> ScanPolicy {
        self.policy
    }

    /// Returns the construction-time semantic proof.
    #[must_use]
    pub const fn static_eligibility(self) -> StaticEligibility {
        self.eligibility
    }

    /// Returns the exact backend that produced the events.
    #[must_use]
    pub const fn activated_backend(self) -> ScanBackend {
        self.backend
    }

    /// Returns whether the v1 specialized backend activated.
    #[must_use]
    pub const fn specialized_backend_activated(self) -> bool {
        matches!(self.backend, ScanBackend::AssertionPrefixV1)
    }

    /// Returns why the generic exact fallback ran, if it did.
    #[must_use]
    pub const fn fallback_reason(self) -> Option<FallbackReason> {
        self.fallback_reason
    }

    /// Returns the unique candidate starts retained for semantic verification.
    #[must_use]
    pub const fn candidate_count(self) -> usize {
        self.candidate_count
    }

    /// Returns the bytes retained by the scan-local candidate bitmap.
    #[must_use]
    pub const fn candidate_bytes(self) -> usize {
        self.candidate_bytes
    }
}

/// Collects patterns for the separate v1 matcher.
#[derive(Debug, Default)]
pub struct AssertionPrefixMatcherBuilder {
    inner: MatcherBuilder,
}

impl AssertionPrefixMatcherBuilder {
    /// Creates an empty experimental matcher builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one caller-identified pattern.
    ///
    /// # Errors
    ///
    /// Returns the same registration failures as [`MatcherBuilder::add`].
    pub fn add(&mut self, pattern_id: PatternId, pattern: &str) -> Result<(), BuildError> {
        self.inner
            .add(pattern_id, pattern)
            .map_err(BuildError::from)
    }

    /// Registers one pattern with explicit compile-time matching options.
    ///
    /// # Errors
    ///
    /// Returns the same registration failures as [`MatcherBuilder::add_with_flags`].
    pub fn add_with_flags(
        &mut self,
        pattern_id: PatternId,
        pattern: &str,
        flags: PatternFlags,
    ) -> Result<(), BuildError> {
        self.inner
            .add_with_flags(pattern_id, pattern, flags)
            .map_err(BuildError::from)
    }

    /// Compiles a statically eligible, immutable experimental matcher.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError::Ineligible`] when the assertion-bearing cohort
    /// does not meet the frozen v1 proof, or wraps an ordinary build failure.
    pub fn build(self) -> Result<AssertionPrefixMatcher, BuildError> {
        let inner = self
            .inner
            .build_assertion_prefix_experimental()
            .map_err(BuildError::from)?;
        let assertion_pattern_count = inner.assertion_pattern_count();
        if assertion_pattern_count == 0 {
            return Err(BuildError::Ineligible(
                StaticIneligibility::NoAssertionBearingPatterns,
            ));
        }
        if assertion_pattern_count < MIN_LITERAL_PATTERN_COUNT {
            return Err(BuildError::Ineligible(
                StaticIneligibility::TooFewAssertionBearingPatterns {
                    found: assertion_pattern_count,
                    minimum: MIN_LITERAL_PATTERN_COUNT,
                },
            ));
        }
        let Some(shared_ascii_prefix) = inner.assertion_prefix() else {
            return Err(BuildError::Ineligible(
                StaticIneligibility::NoSharedFiveUnitAsciiPrefix,
            ));
        };
        Ok(AssertionPrefixMatcher {
            inner,
            eligibility: StaticEligibility {
                assertion_pattern_count,
                shared_ascii_prefix,
            },
        })
    }
}

/// Immutable, reusable v1 assertion-prefix matcher.
#[derive(Debug)]
pub struct AssertionPrefixMatcher {
    inner: SharedCohortMatcher,
    eligibility: StaticEligibility,
}

impl AssertionPrefixMatcher {
    /// Returns the construction-time semantic proof.
    #[must_use]
    pub const fn static_eligibility(&self) -> StaticEligibility {
        self.eligibility
    }

    /// Scans one input under an explicit activation/fallback policy.
    ///
    /// Match semantics are identical to [`crate::Matcher::scan`]. Events are
    /// buffered before delivery, so a specialization refusal or engine error
    /// invokes no callbacks. A callback panic propagates normally and does not
    /// poison the matcher.
    ///
    /// # Errors
    ///
    /// Returns [`ScanError::SpecializationUnavailable`] before callback
    /// delivery when `policy` is [`ScanPolicy::RequireSpecialized`] and the
    /// frozen input-size or density rule cannot activate v1. Ordinary scan
    /// failures are wrapped in [`ScanError::Matcher`].
    pub fn scan_with_policy(
        &self,
        input: &Utf16Text,
        policy: ScanPolicy,
        mut sink: impl FnMut(Match),
    ) -> Result<ScanReport, ScanError> {
        if policy == ScanPolicy::RequireSpecialized {
            let decision = self.inner.preflight_experimental(input);
            if decision != AssertionPrefixDecision::Activated {
                return Err(ScanError::SpecializationUnavailable {
                    reason: fallback_reason(decision, input.as_units().len()),
                });
            }
        }

        let scan = self.inner.scan_experimental(input)?;
        let fallback_reason = (scan.decision != AssertionPrefixDecision::Activated)
            .then(|| fallback_reason(scan.decision, input.as_units().len()));
        if policy == ScanPolicy::RequireSpecialized {
            if let Some(reason) = fallback_reason {
                return Err(ScanError::SpecializationUnavailable { reason });
            }
        }
        for matched in scan.events {
            sink(matched);
        }
        Ok(ScanReport {
            policy,
            eligibility: self.eligibility,
            backend: if fallback_reason.is_some() {
                ScanBackend::GenericExactFallback
            } else {
                ScanBackend::AssertionPrefixV1
            },
            fallback_reason,
            candidate_count: scan.candidate_count,
            candidate_bytes: scan.candidate_bytes,
        })
    }
}

fn fallback_reason(decision: AssertionPrefixDecision, input_units: usize) -> FallbackReason {
    match decision {
        AssertionPrefixDecision::InputSize => FallbackReason::InputTooShort {
            input_units,
            minimum_input_units: MIN_LITERAL_INPUT_UNITS,
        },
        AssertionPrefixDecision::DenseSample => FallbackReason::CandidateSampleTooDense,
        AssertionPrefixDecision::Disabled => FallbackReason::AccelerationDisabled,
        AssertionPrefixDecision::LiteralDisabled => FallbackReason::LiteralAccelerationDisabled,
        AssertionPrefixDecision::Activated => {
            unreachable!("an activated scan does not have a fallback reason")
        }
    }
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use super::{
        AssertionPrefixMatcher, AssertionPrefixMatcherBuilder, BuildError, FallbackReason,
        ScanBackend, ScanError, ScanPolicy, StaticIneligibility,
    };
    use crate::{Match, Matcher, MatcherBuilder, PatternId, Utf16Text};

    #[test]
    fn build_rejects_each_static_ineligibility_class() -> Result<(), BuildError> {
        // Prepare
        let mut assertion_free = AssertionPrefixMatcherBuilder::new();
        assertion_free.add(PatternId::new(1), "word0001")?;
        let mut too_small = AssertionPrefixMatcherBuilder::new();
        too_small.add(PatternId::new(1), r"\bword0001\b")?;
        let mut mismatched = AssertionPrefixMatcherBuilder::new();
        add_word_patterns(&mut mismatched)?;
        mismatched.add(PatternId::new(257), r"\bother0256\b")?;

        // Test
        let no_assertions = assertion_free.build();
        let below_minimum = too_small.build();
        let no_shared_prefix = mismatched.build();

        // Assert
        assert!(matches!(
            no_assertions,
            Err(BuildError::Ineligible(
                StaticIneligibility::NoAssertionBearingPatterns
            ))
        ));
        assert!(matches!(
            below_minimum,
            Err(BuildError::Ineligible(
                StaticIneligibility::TooFewAssertionBearingPatterns {
                    found: 1,
                    minimum: 256
                }
            ))
        ));
        assert!(matches!(
            no_shared_prefix,
            Err(BuildError::Ineligible(
                StaticIneligibility::NoSharedFiveUnitAsciiPrefix
            ))
        ));
        Ok(())
    }

    #[test]
    fn sparse_boundary_scan_activates_and_reports_exact_events() -> Result<(), BuildError> {
        // Prepare
        let matcher = word_matcher()?;
        let mut units = vec![u16::from(b' '); 1024 * 1024];
        place_ascii(&mut units, 4_096, "word0001");
        place_ascii(&mut units, 524_288, "word0042");
        units[899_999] = 0xd800;
        place_ascii(&mut units, 900_000, "word0255");
        units[900_008] = 0xdc00;
        let input = Utf16Text::from_units(units);
        let mut events = Vec::new();

        // Test
        let report = matcher
            .scan_with_policy(&input, ScanPolicy::RequireSpecialized, |matched| {
                events.push(event(matched));
            })
            .unwrap_or_else(|error| panic!("eligible scan failed: {error}"));

        // Assert
        events.sort_unstable();
        assert_eq!(
            events,
            [
                (2, 4_096, 4_104),
                (43, 524_288, 524_296),
                (256, 900_000, 900_008)
            ]
        );
        assert_eq!(report.requested_policy(), ScanPolicy::RequireSpecialized);
        assert_eq!(report.activated_backend(), ScanBackend::AssertionPrefixV1);
        assert!(report.specialized_backend_activated());
        assert_eq!(report.fallback_reason(), None);
        assert_eq!(report.static_eligibility().assertion_pattern_count(), 256);
        assert_eq!(report.static_eligibility().shared_ascii_prefix(), *b"word0");
        assert!(report.candidate_count() >= 3);
        assert!(report.candidate_bytes() > 0);
        Ok(())
    }

    #[test]
    fn require_specialized_refuses_short_input_before_callbacks() -> Result<(), BuildError> {
        // Prepare
        let matcher = word_matcher()?;
        let input = Utf16Text::from("word0001 word0042");
        let mut callback_count = 0;

        // Test
        let result = matcher.scan_with_policy(&input, ScanPolicy::RequireSpecialized, |_| {
            callback_count += 1;
        });

        // Assert
        assert_eq!(callback_count, 0);
        assert_eq!(
            result,
            Err(ScanError::SpecializationUnavailable {
                reason: FallbackReason::InputTooShort {
                    input_units: 17,
                    minimum_input_units: 1024 * 1024,
                },
            })
        );
        Ok(())
    }

    #[test]
    fn exact_fallback_matches_the_normal_matcher_and_reports_reason() -> Result<(), BuildError> {
        // Prepare
        let experimental = word_matcher()?;
        let ordinary = ordinary_word_matcher()?;
        let input = Utf16Text::from("word0001 other word0042 word0255");
        let mut experimental_events = Vec::new();
        let mut ordinary_events = Vec::new();
        ordinary
            .scan(&input, |matched| ordinary_events.push(event(matched)))
            .map_err(BuildError::Matcher)?;

        // Test
        let report = experimental
            .scan_with_policy(&input, ScanPolicy::AllowExactFallback, |matched| {
                experimental_events.push(event(matched));
            })
            .unwrap_or_else(|error| panic!("fallback scan failed: {error}"));

        // Assert
        experimental_events.sort_unstable();
        ordinary_events.sort_unstable();
        assert_eq!(experimental_events, ordinary_events);
        assert_eq!(
            report.activated_backend(),
            ScanBackend::GenericExactFallback
        );
        assert_eq!(
            report.fallback_reason(),
            Some(FallbackReason::InputTooShort {
                input_units: input.as_units().len(),
                minimum_input_units: 1024 * 1024,
            })
        );
        assert_eq!(report.candidate_count(), 0);
        assert_eq!(report.candidate_bytes(), 0);
        Ok(())
    }

    #[test]
    fn dense_input_is_refused_before_generic_work_or_callbacks() -> Result<(), BuildError> {
        // Prepare
        let mut builder = AssertionPrefixMatcherBuilder::new();
        for ordinal in 0..256_u32 {
            builder.add(PatternId::new(ordinal + 1), r"\baaaaa\b")?;
        }
        let matcher = builder.build()?;
        let input = Utf16Text::from_units(vec![u16::from(b'a'); 1024 * 1024]);
        let mut callback_count = 0;

        // Test
        let result = matcher.scan_with_policy(&input, ScanPolicy::RequireSpecialized, |_| {
            callback_count += 1;
        });

        // Assert
        assert_eq!(callback_count, 0);
        assert_eq!(
            result,
            Err(ScanError::SpecializationUnavailable {
                reason: FallbackReason::CandidateSampleTooDense,
            })
        );
        Ok(())
    }

    #[test]
    fn matcher_is_reusable_after_callback_panic() -> Result<(), BuildError> {
        // Prepare
        let matcher = word_matcher()?;
        let mut units = vec![u16::from(b' '); 1024 * 1024];
        place_ascii(&mut units, 4_096, "word0001");
        let input = Utf16Text::from_units(units);

        // Test
        let panic_result = catch_unwind(AssertUnwindSafe(|| {
            let _ = matcher.scan_with_policy(&input, ScanPolicy::RequireSpecialized, |_| {
                panic!("controlled callback panic");
            });
        }));
        let mut recovered = Vec::new();
        matcher
            .scan_with_policy(&input, ScanPolicy::RequireSpecialized, |matched| {
                recovered.push(event(matched));
            })
            .unwrap_or_else(|error| panic!("recovery scan failed: {error}"));

        // Assert
        assert!(panic_result.is_err());
        assert_eq!(recovered, [(2, 4_096, 4_104)]);
        Ok(())
    }

    fn word_matcher() -> Result<AssertionPrefixMatcher, BuildError> {
        let mut builder = AssertionPrefixMatcherBuilder::new();
        add_word_patterns(&mut builder)?;
        builder.build()
    }

    fn ordinary_word_matcher() -> Result<Matcher, BuildError> {
        let mut builder = MatcherBuilder::new();
        for ordinal in 0..256_u32 {
            builder.add(
                PatternId::new(ordinal + 1),
                &format!(r"\bword{ordinal:04}\b"),
            )?;
        }
        builder.build().map_err(BuildError::Matcher)
    }

    fn add_word_patterns(builder: &mut AssertionPrefixMatcherBuilder) -> Result<(), BuildError> {
        for ordinal in 0..256_u32 {
            builder.add(
                PatternId::new(ordinal + 1),
                &format!(r"\bword{ordinal:04}\b"),
            )?;
        }
        Ok(())
    }

    fn place_ascii(units: &mut [u16], start: usize, text: &str) {
        for (offset, symbol) in text.bytes().enumerate() {
            units[start + offset] = u16::from(symbol);
        }
    }

    fn event(matched: Match) -> (u32, u64, u64) {
        (
            matched.pattern_id().get(),
            matched.span().start(),
            matched.span().end(),
        )
    }
}
