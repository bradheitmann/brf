//! brf: re-entry briefs for the projects you own. See README.md and PROTOCOL.md.
//!
//! The library behind the `brf` command line. Each module matches one job: naming files,
//! the registry, the output folder, building, checking, delivering, snapshots and context.

pub mod build;
pub mod check;
pub mod config;
pub mod context;
pub mod deliver;
pub mod error;
pub mod folder;
pub mod html;
pub mod naming;
pub mod registry;
pub mod repo;
pub mod snapshot;
pub mod templates;
pub mod util;
pub mod verify;
