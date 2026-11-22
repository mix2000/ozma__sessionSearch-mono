//! A launch plan: one way to open a terminal, as plain data. Built by
//! `services::launch_plan`, executed by `platform`.

/// Program and arguments to spawn in the conversation's workdir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub program: String,
    pub args: Vec<String>,
    /// Windows only: the command line passed verbatim (for `cmd.exe`, whose
    /// parsing differs from the MSVC rules used by `args`).
    pub raw_args: Option<String>,
}

impl LaunchPlan {
    pub fn new(program: &str, args: Vec<String>) -> Self {
        LaunchPlan {
            program: program.into(),
            args,
            raw_args: None,
        }
    }
}
