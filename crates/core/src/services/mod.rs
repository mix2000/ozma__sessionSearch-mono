//! Business logic: query parsing, snippets, search assembly, resume rules,
//! live-session detection and incremental indexing. Pure functions are tested
//! without the OS; OS access goes through `platform` traits.

pub mod agents;
pub mod command;
pub mod indexer;
pub mod launch_plan;
pub mod live;
pub mod query;
pub mod resume;
pub mod search;
pub mod snippet;
