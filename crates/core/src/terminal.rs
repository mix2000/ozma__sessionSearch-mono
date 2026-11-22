//! Opening a conversation in a new system terminal window.
//!
//! Launch plans are built as plain data (program + arguments, never a shell
//! string assembled from untrusted input) so they can be unit-tested for every
//! OS on any host. Session ids are validated by [`valid_id`]; work directories
//! are quoted for the target shell, and paths with characters that cannot be
//! quoted safely are refused.

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOs {
    Mac,
    Windows,
    Linux,
}

impl TargetOs {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            TargetOs::Mac
        } else if cfg!(windows) {
            TargetOs::Windows
        } else {
            TargetOs::Linux
        }
    }
}

/// Which macOS terminal app to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MacTerminal {
    #[default]
    Terminal,
    ITerm,
}

impl std::str::FromStr for MacTerminal {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "" | "terminal" | "terminal.app" => Ok(MacTerminal::Terminal),
            "iterm" | "iterm2" | "iterm.app" => Ok(MacTerminal::ITerm),
            other => Err(format!(
                "unknown terminal: {other} (expected terminal|iterm)"
            )),
        }
    }
}

/// Session ids: uuid-like, safe charset only, no leading dash.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && !id.starts_with('-')
        && !id.starts_with('.')
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
}

/// Command tokens (program, flags, ids): a charset that needs no quoting anywhere.
fn safe_token(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
}

/// POSIX shell single-quoting.
pub fn sh_quote(s: &str) -> String {
    if !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_./:@%+=,".contains(&b))
    {
        return s.to_owned();
    }
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// AppleScript string literal.
fn applescript_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn has_control(s: &str) -> bool {
    s.chars().any(char::is_control)
}

/// The command a user can paste into their own terminal.
pub fn copy_command(os: TargetOs, workdir: &str, argv: &[String]) -> String {
    match os {
        TargetOs::Windows => format!("cd /d \"{workdir}\" && {}", argv.join(" ")),
        _ => format!(
            "cd {} && {}",
            sh_quote(workdir),
            argv.iter()
                .map(|a| sh_quote(a))
                .collect::<Vec<_>>()
                .join(" ")
        ),
    }
}

/// One way to open a terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub program: String,
    pub args: Vec<String>,
    /// Windows only: the command line passed verbatim (for `cmd.exe`, whose
    /// parsing differs from the MSVC rules used by `args`).
    pub raw_args: Option<String>,
}

impl LaunchPlan {
    fn new(program: &str, args: Vec<String>) -> Self {
        LaunchPlan {
            program: program.into(),
            args,
            raw_args: None,
        }
    }
}

/// Plans in order of preference. Arguments in `argv` must be validated tokens.
pub fn plans(
    os: TargetOs,
    mac_terminal: MacTerminal,
    workdir: &str,
    argv: &[String],
) -> Result<Vec<LaunchPlan>> {
    if has_control(workdir) {
        return Err(anyhow!("unsupported characters in the folder path"));
    }
    if argv.is_empty() || argv.iter().any(|a| !safe_token(a)) {
        return Err(anyhow!("unsafe command token"));
    }
    Ok(match os {
        TargetOs::Mac => {
            let cmd = copy_command(os, workdir, argv);
            let script: Vec<String> = match mac_terminal {
                MacTerminal::Terminal => vec![
                    "tell application \"Terminal\"".into(),
                    "activate".into(),
                    format!("do script {}", applescript_str(&cmd)),
                    "end tell".into(),
                ],
                MacTerminal::ITerm => vec![
                    "tell application \"iTerm\"".into(),
                    "activate".into(),
                    "set w to (create window with default profile)".into(),
                    format!(
                        "tell current session of w to write text {}",
                        applescript_str(&cmd)
                    ),
                    "end tell".into(),
                ],
            };
            let args = script
                .into_iter()
                .flat_map(|l| ["-e".to_owned(), l])
                .collect();
            vec![LaunchPlan::new("osascript", args)]
        }
        TargetOs::Windows => {
            if workdir.contains('"') {
                return Err(anyhow!("unsupported characters in the folder path"));
            }
            let mut out = Vec::new();
            // Windows Terminal splits its command line on ';' unless escaped.
            let wt_dir = workdir.replace(';', "\\;");
            let mut wt_args = vec!["-d".to_owned(), wt_dir, "cmd".into(), "/k".into()];
            wt_args.extend(argv.iter().cloned());
            out.push(LaunchPlan::new("wt.exe", wt_args));
            if !workdir.contains('%') {
                out.push(LaunchPlan {
                    program: "cmd.exe".into(),
                    args: vec![],
                    raw_args: Some(format!(
                        "/c start \"\" /D \"{workdir}\" cmd /k {}",
                        argv.join(" ")
                    )),
                });
            }
            out
        }
        TargetOs::Linux => {
            let inner = format!(
                "{}; exec \"${{SHELL:-sh}}\"",
                copy_command(os, workdir, argv)
            );
            let sh = |prefix: &[&str]| {
                let mut a: Vec<String> = prefix.iter().map(|s| s.to_string()).collect();
                a.extend(["sh".into(), "-c".into(), inner.clone()]);
                a
            };
            vec![
                LaunchPlan::new("x-terminal-emulator", sh(&["-e"])),
                LaunchPlan::new(
                    "gnome-terminal",
                    sh(&[&format!("--working-directory={workdir}"), "--"]),
                ),
                LaunchPlan::new("konsole", sh(&["--workdir", workdir, "-e"])),
                LaunchPlan::new("xterm", sh(&["-e"])),
            ]
        }
    })
}

fn spawn(plan: &LaunchPlan, workdir: &Path) -> std::io::Result<()> {
    let mut cmd = Command::new(&plan.program);
    cmd.args(&plan.args)
        .current_dir(workdir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    if let Some(raw) = &plan.raw_args {
        use std::os::windows::process::CommandExt;
        cmd.raw_arg(raw);
    }
    #[cfg(not(windows))]
    if plan.raw_args.is_some() {
        return Err(std::io::Error::other("raw arguments are Windows-only"));
    }
    if cfg!(target_os = "macos") {
        // osascript returns quickly; a non-zero status means the script failed.
        let status = cmd.status()?;
        if !status.success() {
            return Err(std::io::Error::other(format!(
                "{} exited with {status}",
                plan.program
            )));
        }
        return Ok(());
    }
    cmd.spawn().map(drop)
}

/// Opens a new terminal window in `workdir` running `argv`.
pub fn launch(mac_terminal: MacTerminal, workdir: &Path, argv: &[String]) -> Result<()> {
    let dir = workdir.to_string_lossy();
    let mut last_err = None;
    for plan in plans(TargetOs::current(), mac_terminal, &dir, argv)? {
        match spawn(&plan, workdir) {
            Ok(()) => return Ok(()),
            Err(e) => {
                tracing::debug!("terminal {} failed: {e}", plan.program);
                last_err = Some(anyhow!("{}: {e}", plan.program));
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow!("no terminal available")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv() -> Vec<String> {
        vec![
            "claude".into(),
            "--resume".into(),
            "0a1b2c3d-1111-2222-3333-444455556666".into(),
        ]
    }

    #[test]
    fn id_validation() {
        assert!(valid_id("0a1b2c3d-1111-2222-3333-444455556666"));
        assert!(valid_id("abc_DEF.1"));
        for bad in [
            "",
            "-rf",
            "a b",
            "a;b",
            "a$(x)",
            "a/b",
            "..",
            "a\"b",
            "a'b",
            &"x".repeat(129),
        ] {
            assert!(!valid_id(bad), "{bad:?}");
        }
    }

    #[test]
    fn posix_quoting() {
        assert_eq!(sh_quote("/Users/me/proj"), "/Users/me/proj");
        assert_eq!(sh_quote("/a b/it's $(rm)"), r"'/a b/it'\''s $(rm)'");
    }

    #[test]
    fn mac_terminal_plan_escapes_applescript() {
        let p = plans(
            TargetOs::Mac,
            MacTerminal::Terminal,
            "/a \"b\"/c\\d's",
            &argv(),
        )
        .unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].program, "osascript");
        let script = p[0].args.join("\n");
        assert!(script.contains("tell application \"Terminal\""));
        assert!(script.contains(
            r#"do script "cd '/a \"b\"/c\\d'\\''s' && claude --resume 0a1b2c3d-1111-2222-3333-444455556666""#
        ));
        let p = plans(TargetOs::Mac, MacTerminal::ITerm, "/w", &argv()).unwrap();
        assert!(p[0].args.join("\n").contains("tell application \"iTerm\""));
    }

    #[test]
    fn windows_plans() {
        let p = plans(
            TargetOs::Windows,
            MacTerminal::Terminal,
            r"C:\Users\me\my proj;x",
            &argv(),
        )
        .unwrap();
        assert_eq!(p[0].program, "wt.exe");
        assert_eq!(
            p[0].args,
            [
                "-d",
                r"C:\Users\me\my proj\;x",
                "cmd",
                "/k",
                "claude",
                "--resume",
                "0a1b2c3d-1111-2222-3333-444455556666"
            ]
        );
        assert_eq!(p[1].program, "cmd.exe");
        assert_eq!(
            p[1].raw_args.as_deref(),
            Some(
                r#"/c start "" /D "C:\Users\me\my proj;x" cmd /k claude --resume 0a1b2c3d-1111-2222-3333-444455556666"#
            )
        );
        // '%' could expand variables in cmd.exe: only the wt plan remains.
        let p = plans(
            TargetOs::Windows,
            MacTerminal::Terminal,
            r"C:\%PATH%",
            &argv(),
        )
        .unwrap();
        assert_eq!(p.len(), 1);
        assert!(plans(
            TargetOs::Windows,
            MacTerminal::Terminal,
            "C:\\a\"b",
            &argv()
        )
        .is_err());
    }

    #[test]
    fn linux_plans_quote_workdir() {
        let p = plans(
            TargetOs::Linux,
            MacTerminal::Terminal,
            "/home/me/it's",
            &argv(),
        )
        .unwrap();
        assert_eq!(p[0].program, "x-terminal-emulator");
        assert_eq!(p[0].args[..3], ["-e", "sh", "-c"]);
        assert!(p[0].args[3].starts_with(r"cd '/home/me/it'\''s' && claude --resume"));
    }

    #[test]
    fn rejects_unsafe_input() {
        assert!(plans(TargetOs::Mac, MacTerminal::Terminal, "/a\nb", &argv()).is_err());
        let bad = vec!["claude".into(), "--resume".into(), "x; rm -rf /".into()];
        assert!(plans(TargetOs::Linux, MacTerminal::Terminal, "/w", &bad).is_err());
    }

    #[test]
    fn copy_commands() {
        assert_eq!(
            copy_command(TargetOs::Mac, "/my dir", &argv()),
            "cd '/my dir' && claude --resume 0a1b2c3d-1111-2222-3333-444455556666"
        );
        assert_eq!(
            copy_command(TargetOs::Windows, r"C:\w", &argv()),
            r#"cd /d "C:\w" && claude --resume 0a1b2c3d-1111-2222-3333-444455556666"#
        );
    }
}
