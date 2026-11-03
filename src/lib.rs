//! ResMUD: shared, programmable worlds on ikigai.
//!
//! Nothing is built yet beyond the names: the design of the first playable build is
//! `docs/design/smallest-playable.md`. Every ResMUD name lives under [`NAMESPACE`], and resolves
//! through an [`ikigai_core`] kernel.
#![forbid(unsafe_code)]

pub use ikigai_core;

/// The URN prefix every ResMUD resource is named under, the type first: `urn:resmud:room:id:{room}`.
///
/// ```
/// assert!(resmud::NAMESPACE.ends_with(':'));
/// assert_eq!(format!("{}room:id:kitchen", resmud::NAMESPACE), "urn:resmud:room:id:kitchen");
/// ```
pub const NAMESPACE: &str = "urn:resmud:";

/// The core vocabulary's namespace (`rm:`), published by the `resmud-vocab` repository and
/// served at this IRI.
///
/// ```
/// assert_eq!(resmud::VOCABULARY, "https://iriref.org/resmud/core#");
/// ```
pub const VOCABULARY: &str = "https://iriref.org/resmud/core#";

/// The spatial vocabulary's namespace (`rms:`), from the same repository.
pub const SPACE_VOCABULARY: &str = "https://iriref.org/resmud/space#";
