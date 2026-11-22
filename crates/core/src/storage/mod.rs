//! Storage: our SQLite index — sessions, messages, per-file progress.

pub mod files;
pub mod messages;
pub mod schema;
pub mod sessions;

pub use schema::open;
