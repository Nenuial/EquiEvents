//! A horse show: its data model, the operation log every change goes through, and storage.
//!
//! Design: docs/architecture/data-and-sync.md.
//!
//! - [`model`]: the normalised data of a show (people, horses, classes, entries, results).
//! - [`oplog`]: operations and the hybrid logical clock that orders them.
//! - [`state`]: the current state, computed from the log.
//! - [`delivered`]: merging a new delivery of an entry from Swiss Equestrian with the office's changes.
//! - [`merge`]: changes made apart put together: start lists entry by entry, classes field by field.
//! - [`input`]: from the state to the scoring engine's input, class by class.
//! - [`ranking`]: a class's ranking: the engine's result, ranks set by hand, placed riders.
//! - [`combined`]: results across classes: rankings over several classes, penalties brought
//!   from a qualifier.
//! - [`startlist`]: a class's start list: order, start times, blocks, team and level headings.
//! - [`master`]: Swiss Equestrian master data (programmes, scoring tables) read into the scoring model.
//! - [`store`]: one SQLite file per show, holding the log.
//! - [`touched`]: what a change sets, thing by thing, and what its author saw of it (R12).
//! - [`export`]: one show in one file (`.equievents`), to keep, hand over or continue elsewhere.

pub mod combined;
pub mod delivered;
pub mod export;
pub mod input;
pub mod master;
pub mod merge;
pub mod model;
pub mod oplog;
pub mod public;
pub mod ranking;
pub mod startlist;
pub mod state;
pub mod store;
pub mod touched;

pub use input::{ScoringInput, TeamConesDetail, TeamConesRound};
pub use state::State;
pub use store::Store;
