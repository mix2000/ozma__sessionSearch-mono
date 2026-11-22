//! Grok transcripts. Two independent sources under the grok root:
//!
//! * **Grok Build** (official xAI CLI): `<root>/sessions/<encoded cwd>/<session id>/`
//!   with `summary.json` ([`summary`]) and `updates.jsonl` ([`updates`]).
//!   `chat_history.jsonl` is not indexed.
//! * **grok-cli** (community `grok-dev`): one SQLite database `<root>/grok.db`
//!   ([`cli`]).

pub mod cli;
pub mod discover;
pub mod summary;
pub mod updates;

pub use discover::discover;
pub use updates::UpdatesParser;
