//! Launch plans for opening a conversation in a new system terminal window.
//!
//! Plans are built as plain data (program + arguments, never a shell string
//! assembled from untrusted input) so they can be unit-tested for every OS on
//! any host; `platform` only spawns them. Session ids are validated by
//! [`super::command::valid_id`]; work directories are quoted for the target
//! shell, and paths with characters that cannot be quoted safely are refused.

use std::path::Path;

use anyhow::{anyhow, Result};

use super::command::{applescript_str, copy_command, has_control, safe_token};
use crate::models::{LaunchPlan, MacTerminal, TargetOs};
use crate::platform::TerminalSpawner;

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
    match os {
        TargetOs::Mac => Ok(vec![mac_plan(mac_terminal, workdir, argv)]),
        TargetOs::Windows => windows_plans(workdir, argv),
        TargetOs::Linux => Ok(linux_plans(workdir, argv)),
    }
}

/// `osascript` driving Terminal.app or iTerm.
fn mac_plan(mac_terminal: MacTerminal, workdir: &str, argv: &[String]) -> LaunchPlan {
    let cmd = applescript_str(&copy_command(TargetOs::Mac, workdir, argv));
    let script: Vec<String> = match mac_terminal {
        MacTerminal::Terminal => vec![
            "tell application \"Terminal\"".into(),
            "activate".into(),
            format!("do script {cmd}"),
            "end tell".into(),
        ],
        MacTerminal::ITerm => vec![
            "tell application \"iTerm\"".into(),
            "activate".into(),
            "set w to (create window with default profile)".into(),
            format!("tell current session of w to write text {cmd}"),
            "end tell".into(),
        ],
    };
    let args = script
        .into_iter()
        .flat_map(|l| ["-e".to_owned(), l])
        .collect();
    LaunchPlan::new("osascript", args)
}

/// Windows Terminal, then `cmd.exe start` (skipped when `%` could expand variables).
fn windows_plans(workdir: &str, argv: &[String]) -> Result<Vec<LaunchPlan>> {
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
    Ok(out)
}

/// Common Linux terminal emulators, each running the command then a shell.
fn linux_plans(workdir: &str, argv: &[String]) -> Vec<LaunchPlan> {
    let inner = format!(
        "{}; exec \"${{SHELL:-sh}}\"",
        copy_command(TargetOs::Linux, workdir, argv)
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

/// Opens a new terminal window in `workdir` running `argv`: tries the plans for
/// `os` in order until one spawns.
pub fn launch(
    spawner: &dyn TerminalSpawner,
    os: TargetOs,
    mac_terminal: MacTerminal,
    workdir: &Path,
    argv: &[String],
) -> Result<()> {
    let dir = workdir.to_string_lossy();
    let mut last_err = None;
    for plan in plans(os, mac_terminal, &dir, argv)? {
        match spawner.spawn(&plan, workdir) {
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
        // cmd.exe metacharacters stay inside the quotes, verbatim and unescaped.
        let p = plans(
            TargetOs::Windows,
            MacTerminal::Terminal,
            r"C:\a&b^c!d (x)|y",
            &argv(),
        )
        .unwrap();
        assert_eq!(p[0].args[1], r"C:\a&b^c!d (x)|y");
        assert_eq!(
            p[1].raw_args.as_deref(),
            Some(
                r#"/c start "" /D "C:\a&b^c!d (x)|y" cmd /k claude --resume 0a1b2c3d-1111-2222-3333-444455556666"#
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

    struct Recorder {
        fail: Vec<&'static str>,
        tried: std::sync::Mutex<Vec<String>>,
    }

    impl TerminalSpawner for Recorder {
        fn spawn(&self, plan: &LaunchPlan, _workdir: &Path) -> std::io::Result<()> {
            self.tried.lock().unwrap().push(plan.program.clone());
            if self.fail.contains(&plan.program.as_str()) {
                Err(std::io::Error::other("missing"))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn launch_falls_back_to_the_next_plan() {
        let rec = Recorder {
            fail: vec!["x-terminal-emulator", "gnome-terminal"],
            tried: Default::default(),
        };
        launch(
            &rec,
            TargetOs::Linux,
            MacTerminal::Terminal,
            Path::new("/w"),
            &argv(),
        )
        .unwrap();
        assert_eq!(
            *rec.tried.lock().unwrap(),
            ["x-terminal-emulator", "gnome-terminal", "konsole"]
        );
        let all = Recorder {
            fail: vec!["wt.exe", "cmd.exe"],
            tried: Default::default(),
        };
        let err = launch(
            &all,
            TargetOs::Windows,
            MacTerminal::Terminal,
            Path::new("C:\\w"),
            &argv(),
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "cmd.exe: missing");
        let bad = vec!["claude".into(), "x; rm".into()];
        assert!(launch(
            &all,
            TargetOs::Mac,
            MacTerminal::Terminal,
            Path::new("/"),
            &bad
        )
        .is_err());
    }
}
