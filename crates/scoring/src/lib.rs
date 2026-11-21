//! Scoring and ranking rules for Swiss Equestrian shows.
//!
//! Everything in this crate is a pure function of what was entered at the show (marks, faults,
//! times, eliminations) and of the published rules (dressage programmes, jumping scoring
//! tables). No storage, no network, no clock: the same input always gives the same ranking.
//!
//! # How to review
//!
//! Each module starts with the rule it implements, in plain language. Where the rule was decided
//! for this app (and differs from, or clarifies, what Rosson did), the module names the entry in
//! `docs/deviations.md` (S1, S2, ...). Tests come in two kinds:
//!
//! - `tests/cases/*.md`: hand-written cases as tables, one per rule, meant to be read and
//!   extended by people who know the regulations;
//! - the golden tests in `rosson-import`: every result of the real shows in `golden/corpus/`.
//!
//! # Modules
//!
//! - [`number`]: exact decimals and the one rounding rule.
//! - [`ranking`]: ranks, ties, "out", placed riders; shared by all disciplines.
//! - [`dressage`]: marks per judge → totals → percentage → ranking.
//! - [`jumping`]: scoring tables (how a class is judged) → ranking.
//! - [`rulebook`]: all of the above in words, `docs/rulebook.md`, rebuilt and checked by a test.

pub mod dressage;
pub mod driving;
pub mod eventing;
pub mod jumping;
pub mod number;
pub mod ranking;
pub mod rulebook;
