//! Windows: Windows Terminal or `cmd.exe start`, the latter with a verbatim
//! command line.

use std::os::windows::process::CommandExt;
use std::process::Command;

use crate::models::LaunchPlan;
use crate::models::TargetOs;

pub const OS: TargetOs = TargetOs::Windows;

pub fn spawn(plan: &LaunchPlan, mut cmd: Command) -> std::io::Result<()> {
    if let Some(raw) = &plan.raw_args {
        cmd.raw_arg(raw);
    }
    cmd.spawn().map(drop)
}
