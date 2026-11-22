//! Linux and other Unix: the first terminal emulator that starts wins.

use std::process::Command;

use crate::models::LaunchPlan;
use crate::models::TargetOs;

pub const OS: TargetOs = TargetOs::Linux;

pub fn spawn(plan: &LaunchPlan, mut cmd: Command) -> std::io::Result<()> {
    if plan.raw_args.is_some() {
        return Err(std::io::Error::other("raw arguments are Windows-only"));
    }
    cmd.spawn().map(drop)
}
