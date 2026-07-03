//! Pure hygiene rule engine (`linear hygiene`).
//!
//! Detects workflow-hygiene problems across issues, projects, and initiatives
//! from declarative TOML rules and turns them into findings with executable
//! fixes. See `docs/hygiene.md` for the full design (requirements R1–R40).
//!
//! Layout:
//! - [`config`] — `hygiene.toml` types, load/merge, validation (R1–R9, R30).
//! - [`model`] — flattened entity field model + GraphQL JSON conversion (§4.1).
//! - [`engine`] — predicate evaluation, builtins, finding construction,
//!   fix derivation, deterministic sorting (§4.2–§4.5). Pure: all inputs
//!   (including `now` and label-group/field candidates) are passed in.
//! - [`state`] — last-run artifact (R32–R34) and snooze state (R26) with
//!   atomic, profile/auth-scoped persistence.

pub mod config;
pub mod engine;
pub mod model;
pub mod state;
