//! macOS: terminals are opened through `osascript`.

use std::process::Command;

use crate::models::LaunchPlan;
use crate::models::TargetOs;

pub const OS: TargetOs = TargetOs::Mac;

/// `osascript` returns quickly; a non-zero status means the script failed.
pub fn spawn(plan: &LaunchPlan, mut cmd: Command) -> std::io::Result<()> {
    if plan.raw_args.is_some() {
        return Err(std::io::Error::other("raw arguments are Windows-only"));
    }
    let status = cmd.status()?;
    if !status.success() {
        return Err(std::io::Error::other(format!(
            "{} exited with {status}",
            plan.program
        )));
    }
    Ok(())
}
