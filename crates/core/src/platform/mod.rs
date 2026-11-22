//! Everything that depends on the operating system. `#[cfg]` lives only here:
//! the rest of the crate talks to the OS through these traits, so it can be
//! tested by substitution. This layer only executes — it builds no plans and
//! knows nothing about services.

use std::collections::HashSet;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::models::{LaunchPlan, TargetOs};

mod fs;
pub mod paths;
mod processes;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as os;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as os;

#[cfg(all(unix, not(target_os = "macos")))]
mod unix;
#[cfg(all(unix, not(target_os = "macos")))]
use unix as os;

pub use fs::OsFs;
pub use processes::OsProcesses;

/// The OS this binary runs on.
pub fn current_os() -> TargetOs {
    os::OS
}

/// Spawns one launch plan (replaceable in tests).
pub trait TerminalSpawner: Send + Sync {
    fn spawn(&self, plan: &LaunchPlan, workdir: &Path) -> std::io::Result<()>;
}

/// Spawns plans as real processes, detached from our stdio.
pub struct OsTerminal;

impl TerminalSpawner for OsTerminal {
    fn spawn(&self, plan: &LaunchPlan, workdir: &Path) -> std::io::Result<()> {
        let mut cmd = Command::new(&plan.program);
        cmd.args(&plan.args)
            .current_dir(workdir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        os::spawn(plan, cmd)
    }
}

/// Running processes at one moment.
#[derive(Debug, Clone, Default)]
pub struct ProcessSnapshot {
    /// Pid and command line (or just the name) of every process but ours.
    pub processes: Vec<(u32, Vec<String>)>,
    /// All running pids.
    pub alive: HashSet<u32>,
}

/// Lists running processes (replaceable in tests).
pub trait ProcessSource: Send + Sync {
    /// `None` when the process list is unavailable.
    fn snapshot(&self) -> Option<ProcessSnapshot>;
}

/// Questions about paths the engine asks (replaceable in tests).
pub trait FsProbe: Send + Sync {
    fn is_dir(&self, path: &Path) -> bool;
    fn is_file(&self, path: &Path) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_os_matches_cfg() {
        let expected = if cfg!(target_os = "macos") {
            TargetOs::Mac
        } else if cfg!(windows) {
            TargetOs::Windows
        } else {
            TargetOs::Linux
        };
        assert_eq!(current_os(), expected);
    }

    #[test]
    fn missing_program_is_an_error() {
        let plan = LaunchPlan::new("definitely-not-a-terminal-xyz", vec![]);
        assert!(OsTerminal.spawn(&plan, Path::new(".")).is_err());
    }
}
