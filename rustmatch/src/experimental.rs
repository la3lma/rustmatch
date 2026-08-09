//! Explicit, feature-gated matcher experiments.
//!
//! Nothing in this module is available unless its corresponding non-default
//! Cargo feature is enabled. Experimental performance variance never relaxes
//! rustmatch's exact matching semantics.

/// Versioned assertion-prefix specialization.
#[cfg(feature = "unstable-assertion-prefix-v1")]
pub mod assertion_prefix_v1;
