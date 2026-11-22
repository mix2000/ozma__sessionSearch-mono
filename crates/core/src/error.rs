//! Errors of the core crate. Every variant carries the HTTP status it maps to;
//! the app renders them as `{statusCode, message, error}` in one place.

/// An error returned by [`crate::Engine`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("400 {0}")]
    BadRequest(String),
    #[error("404 {0}")]
    NotFound(String),
    /// The conversation cannot be resumed right now; `command` is what to run by hand.
    #[error("409 {message}")]
    Conflict { message: String, command: String },
    #[error("500 {message}")]
    Internal {
        message: String,
        /// For resume errors: the command to run by hand.
        command: Option<String>,
    },
}

impl Error {
    pub fn bad_request(m: impl Into<String>) -> Self {
        Error::BadRequest(m.into())
    }

    pub fn not_found(m: impl Into<String>) -> Self {
        Error::NotFound(m.into())
    }

    pub fn conflict(m: impl Into<String>, command: &str) -> Self {
        Error::Conflict {
            message: m.into(),
            command: command.to_owned(),
        }
    }

    pub fn internal(e: impl std::fmt::Display) -> Self {
        Error::Internal {
            message: e.to_string(),
            command: None,
        }
    }

    /// HTTP status code of this error.
    pub fn status(&self) -> u16 {
        match self {
            Error::BadRequest(_) => 400,
            Error::NotFound(_) => 404,
            Error::Conflict { .. } => 409,
            Error::Internal { .. } => 500,
        }
    }

    /// Message for the user.
    pub fn message(&self) -> &str {
        match self {
            Error::BadRequest(m) | Error::NotFound(m) => m,
            Error::Conflict { message, .. } | Error::Internal { message, .. } => message,
        }
    }

    /// Command to run by hand (resume errors).
    pub fn command(&self) -> Option<&str> {
        match self {
            Error::Conflict { command, .. } => Some(command),
            Error::Internal { command, .. } => command.as_deref(),
            _ => None,
        }
    }
}

impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        Error::internal(format!("{e:#}"))
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_message_and_display() {
        let e = Error::conflict("busy", "cd /w && claude --resume x");
        assert_eq!(e.status(), 409);
        assert_eq!(e.message(), "busy");
        assert_eq!(e.command(), Some("cd /w && claude --resume x"));
        assert_eq!(e.to_string(), "409 busy");
        assert_eq!(Error::bad_request("x").to_string(), "400 x");
        assert_eq!(Error::not_found("y").status(), 404);
        assert_eq!(Error::internal("z").command(), None);
        let from: Error = anyhow::anyhow!("inner").context("outer").into();
        assert_eq!(from.message(), "outer: inner");
        assert_eq!(from.status(), 500);
    }
}
