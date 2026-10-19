//! brf: re-entry briefs for the projects you own. See README.md and PROTOCOL.md.
//!
//! The library behind the `brf` command line. Each module matches one job: naming files,
//! the registry, the output folder, building, checking, delivering, snapshots and context.

#[cfg(feature = "full")]
pub mod build;
#[cfg(feature = "full")]
pub mod check;
#[cfg(feature = "full")]
pub mod config;
#[cfg(feature = "full")]
pub mod context;
#[cfg(feature = "full")]
pub mod deliver;
#[cfg(feature = "full")]
pub mod error;
#[cfg(feature = "full")]
pub mod folder;
pub mod html;
pub mod naming;
#[cfg(feature = "full")]
pub mod registry;
#[cfg(feature = "full")]
pub mod repo;
#[cfg(feature = "full")]
pub mod snapshot;
#[cfg(feature = "full")]
pub mod templates;
pub mod util;
#[cfg(feature = "full")]
pub mod verify;

/// Content validation without CLI, template assets, filesystem or Git operations.
pub mod validation;
