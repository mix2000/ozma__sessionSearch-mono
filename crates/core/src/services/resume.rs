//! Rules for resuming a conversation: the one place that builds the resume
//! command, and the checks before opening a terminal.

use crate::error::{Error, Result};
use crate::models::messages;
use crate::models::{Provider, TargetOs};
use crate::services::agents::resume_argv;
use crate::services::command::{copy_command, quotable};
use crate::storage::sessions::SessionRow;

/// `cd <workdir> && <agent resume command>` for `os` (just the agent command
/// when the workdir is unknown); `None` for an unknown provider.
pub fn command(s: &SessionRow, os: TargetOs) -> Option<String> {
    let provider: Provider = s.provider.parse().ok()?;
    let argv = resume_argv(provider, &s.id, &s.file_path);
    Some(match s.workdir.as_deref() {
        Some(w) => copy_command(os, w, &argv),
        None => argv.join(" "),
    })
}

/// Whether [`command`] had to leave the known folder out: run it there.
pub fn needs_folder(s: &SessionRow, os: TargetOs) -> bool {
    s.workdir.as_deref().is_some_and(|w| !quotable(os, w))
}

/// What to run and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeTarget {
    pub argv: Vec<String>,
    /// The same command for one's own terminal (returned with every error).
    pub command: String,
    pub workdir: String,
}

/// The agent command for a stored session.
pub fn target(session: &SessionRow, os: TargetOs) -> Result<ResumeTarget> {
    let provider: Provider = session
        .provider
        .parse()
        .map_err(|e: String| Error::internal(e))?;
    let workdir = session
        .workdir
        .clone()
        .ok_or_else(|| Error::bad_request(messages::WORKDIR_UNKNOWN))?;
    let argv = resume_argv(provider, &session.id, &session.file_path);
    let command = copy_command(os, &workdir, &argv);
    Ok(ResumeTarget {
        argv,
        command,
        workdir,
    })
}

/// What is known about the session right now.
#[derive(Debug, Clone, Copy, Default)]
pub struct ResumeChecks {
    /// Pid of a process that already has the conversation open.
    pub live_pid: Option<u32>,
    pub workdir_exists: bool,
    pub file_exists: bool,
}

/// Refuses to resume a conversation that is open, lost its folder or file.
pub fn check(t: &ResumeTarget, c: ResumeChecks) -> Result<()> {
    if let Some(pid) = c.live_pid {
        return Err(Error::conflict(messages::already_open(pid), &t.command));
    }
    if !c.workdir_exists {
        return Err(Error::conflict(
            messages::workdir_missing(&t.workdir),
            &t.command,
        ));
    }
    if !c.file_exists {
        return Err(Error::conflict(messages::FILE_DELETED, &t.command));
    }
    Ok(())
}

/// A terminal that failed to open.
pub fn launch_failed(t: &ResumeTarget, e: &anyhow::Error) -> Error {
    Error::Internal {
        message: messages::launch_failed(e),
        command: Some(t.command.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(provider: &str, workdir: Option<&str>) -> SessionRow {
        SessionRow {
            id: "abc".into(),
            provider: provider.into(),
            file_path: "/f.jsonl".into(),
            workdir: workdir.map(str::to_owned),
            title: None,
            kind: None,
            git_branch: None,
            started_at: None,
            last_at: None,
            message_count: 0,
            next_seq: 0,
        }
    }

    #[test]
    fn target_needs_known_provider_and_workdir() {
        let t = target(&session("codex", Some("/w")), TargetOs::Linux).unwrap();
        assert_eq!(t.argv, ["codex", "resume", "abc"]);
        assert_eq!(t.command, "cd /w && codex resume abc");
        assert_eq!(
            target(&session("codex", None), TargetOs::Linux)
                .unwrap_err()
                .status(),
            400
        );
        assert_eq!(
            target(&session("x", Some("/w")), TargetOs::Linux)
                .unwrap_err()
                .status(),
            500
        );
    }

    #[test]
    fn checks_in_order_with_command() {
        let t = target(&session("claude", Some("/w")), TargetOs::Mac).unwrap();
        let ok = ResumeChecks {
            live_pid: None,
            workdir_exists: true,
            file_exists: true,
        };
        assert!(check(&t, ok).is_ok());
        let live = check(
            &t,
            ResumeChecks {
                live_pid: Some(7),
                workdir_exists: false,
                ..ok
            },
        )
        .unwrap_err();
        assert_eq!(live.message(), "Разговор уже открыт в терминале (pid 7)");
        assert_eq!(live.command(), Some("cd /w && claude --resume abc"));
        let dir = check(
            &t,
            ResumeChecks {
                workdir_exists: false,
                file_exists: false,
                ..ok
            },
        )
        .unwrap_err();
        assert_eq!(dir.message(), "Папка разговора не найдена: /w");
        let file = check(
            &t,
            ResumeChecks {
                file_exists: false,
                ..ok
            },
        )
        .unwrap_err();
        assert_eq!(file.status(), 409);
        let e = launch_failed(&t, &anyhow::anyhow!("no osascript"));
        assert_eq!(
            (e.status(), e.message()),
            (500, "Не удалось открыть терминал: no osascript")
        );
        assert_eq!(e.command(), Some("cd /w && claude --resume abc"));
    }

    #[test]
    fn command_for_cards_and_cli() {
        let s = session("claude", Some("/my dir"));
        assert_eq!(
            command(&s, TargetOs::Mac).as_deref(),
            Some("cd '/my dir' && claude --resume abc")
        );
        assert_eq!(
            command(&session("codex", None), TargetOs::Windows).as_deref(),
            Some("codex resume abc")
        );
        assert_eq!(command(&session("other", None), TargetOs::Linux), None);
    }

    #[test]
    fn folder_hint_follows_the_command() {
        let s = session("claude", Some(r"C:\100%"));
        assert_eq!(
            command(&s, TargetOs::Windows).as_deref(),
            Some("claude --resume abc")
        );
        assert!(needs_folder(&s, TargetOs::Windows));
        assert!(!needs_folder(&s, TargetOs::Mac));
        assert!(!needs_folder(&session("claude", None), TargetOs::Windows));
    }
}
