//! The binary end to end (`assert_cmd`): sync, status and search with `--json`
//! on a temporary index; errors and exit codes.

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use serde_json::{json, Value};

const ID: &str = "11111111-2222-3333-4444-555555555555";

fn fixture(root: &Path) {
    let project = root.join("claude/-w");
    std::fs::create_dir_all(&project).unwrap();
    let lines = [
        json!({"type": "user", "entrypoint": "cli", "cwd": root.to_string_lossy(),
               "timestamp": "2026-11-10T08:55:37.856Z", "message": {"content": "Почини ДЕПЛОЙ nginx"}}),
        json!({"type": "assistant", "timestamp": "2026-11-10T08:56:00.000Z",
               "message": {"content": [{"type": "text", "text": "Готово, деплой работает."}]}}),
    ];
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(project.join(format!("{ID}.jsonl")), text).unwrap();
}

/// The binary with isolated roots, data dir and no inherited settings.
fn bin(root: &Path) -> Command {
    let mut cmd = Command::cargo_bin("session-search").unwrap();
    for var in [
        "SESSION_SEARCH_DATA_DIR",
        "SESSION_SEARCH_CLAUDE_ROOT",
        "SESSION_SEARCH_CODEX_ROOT",
        "SESSION_SEARCH_GROK_ROOT",
        "SESSION_SEARCH_PORT",
        "SESSION_SEARCH_TOKEN",
        "SESSION_SEARCH_TERMINAL",
        "SESSION_SEARCH_SKIP_PORTS",
        "RUST_LOG",
    ] {
        cmd.env_remove(var);
    }
    cmd.arg("--data-dir")
        .arg(root.join("data"))
        .arg("--claude-root")
        .arg(root.join("claude"))
        .arg("--codex-root")
        .arg(root.join("codex"))
        .arg("--grok-root")
        .arg(root.join("grok"));
    cmd
}

fn json_out(cmd: &mut Command) -> Value {
    let out = cmd.assert().success().get_output().stdout.clone();
    serde_json::from_slice(&out).unwrap()
}

#[test]
fn sync_status_and_search_json() {
    let tmp = tempfile::tempdir().unwrap();
    fixture(tmp.path());

    let sync = json_out(bin(tmp.path()).args(["sync", "--json"]));
    assert_eq!(sync, json!({"files": 1, "updated": 1, "messages": 2}));

    let status = json_out(bin(tmp.path()).args(["status", "--json"]));
    assert_eq!(
        (status["sessions"].as_i64(), status["messages"].as_i64()),
        (Some(1), Some(2))
    );
    assert_eq!(status["providers"][0]["provider"], "claude");
    assert_eq!(status["providers"][1]["exists"], false);

    let found = json_out(bin(tmp.path()).args(["search", "--no-sync", "--json", "деплой"]));
    assert_eq!(found["terms"], json!(["деплой"]));
    assert_eq!(found["results"][0]["id"], ID);
    assert_eq!(found["results"][0]["matches"], 2);

    let filtered = json_out(bin(tmp.path()).args([
        "search",
        "--no-sync",
        "--json",
        "--provider",
        "codex",
        "деплой",
    ]));
    assert_eq!(filtered["results"], json!([]));

    let only_codex = json_out(bin(tmp.path()).args(["--providers", "codex", "status", "--json"]));
    assert_eq!(only_codex["providers"].as_array().unwrap().len(), 1);
}

fn stdout(cmd: &mut Command) -> String {
    String::from_utf8(cmd.assert().success().get_output().stdout.clone()).unwrap()
}

fn indexed() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    fixture(tmp.path());
    tmp
}

#[test]
fn text_search_output() {
    let tmp = indexed();
    let text = stdout(bin(tmp.path()).args(["search", "nginx"]));
    assert!(
        text.contains("[claude] Почини ДЕПЛОЙ nginx  (2 messages, last 2026-11-10T08:56:00.000Z)"),
        "{text}"
    );
    assert!(text.contains(&format!("claude --resume {ID}")), "{text}");
    let none = stdout(bin(tmp.path()).args(["search", "--no-sync", "zzzz"]));
    assert_eq!(none, "Nothing found.\n");
}

#[test]
fn short_query_exits_with_code_2() {
    let tmp = indexed();
    let out = bin(tmp.path())
        .args(["search", "--no-sync", "--json", "a"])
        .assert()
        .code(2)
        .get_output()
        .stdout
        .clone();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(
        v,
        json!({"statusCode": 400, "message": "Введите хотя бы 2 символа"})
    );
    let err = bin(tmp.path())
        .args(["search", "--no-sync", "a"])
        .assert()
        .code(2)
        .get_output()
        .stderr
        .clone();
    assert_eq!(
        String::from_utf8(err).unwrap(),
        "Введите хотя бы 2 символа\n"
    );
}

#[test]
fn bad_flags_and_text_status() {
    let tmp = indexed();
    bin(tmp.path())
        .args(["--providers", "bogus", "status"])
        .assert()
        .failure()
        .code(2);
    bin(tmp.path()).arg("sync").assert().success();
    let text = stdout(bin(tmp.path()).arg("status"));
    assert!(text.starts_with("sessions: 1, messages: 2\n"), "{text}");
    assert!(text.contains("(not found)"), "{text}");
}

#[test]
fn open_failure_exits_with_code_1() {
    let tmp = indexed();
    // The data dir is a file: the index cannot be created.
    std::fs::write(tmp.path().join("data"), "").unwrap();
    let err = bin(tmp.path())
        .arg("status")
        .assert()
        .code(1)
        .get_output()
        .stderr
        .clone();
    let text = String::from_utf8(err).unwrap();
    assert!(text.starts_with("Error: open index"), "{text}");
}

#[test]
fn serve_refuses_a_network_address_without_a_token() {
    let tmp = indexed();
    for extra in [&[][..], &["--token", ""][..]] {
        let err = bin(tmp.path())
            .args(["serve", "--no-open", "--host", "0.0.0.0"])
            .args(extra)
            .timeout(std::time::Duration::from_secs(20))
            .assert()
            .code(1)
            .get_output()
            .stderr
            .clone();
        let text = String::from_utf8(err).unwrap();
        assert!(text.contains("set --token"), "{text}");
    }
}

/// An index with one conversation in `cwd`.
fn indexed_in(cwd: &str) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("claude/-w");
    std::fs::create_dir_all(&project).unwrap();
    let line = json!({"type": "user", "entrypoint": "cli", "cwd": cwd,
        "timestamp": "2026-11-10T08:55:37.856Z", "message": {"content": "nginx \u{1b}[31mred"}});
    std::fs::write(project.join(format!("{ID}.jsonl")), format!("{line}\n")).unwrap();
    tmp
}

#[test]
fn escape_sequences_from_transcripts_are_not_printed() {
    let tmp = indexed_in("/gone/\u{1b}[2Jx");
    let text = stdout(bin(tmp.path()).args(["search", "nginx"]));
    assert!(!text.contains('\u{1b}'), "{text:?}");
    assert!(
        text.contains("  /gone/\u{fffd}[2Jx  (folder missing)\n"),
        "{text}"
    );
    // No folder to run it in: no hint either.
    assert!(
        text.contains(&format!("resume: claude --resume {ID}\n")),
        "{text}"
    );
}

/// A folder that exists but cannot be quoted: the hint says where to run.
#[test]
fn folder_hint_for_a_command_without_cd() {
    let dir = tempfile::tempdir().unwrap();
    // `%` is refused on Windows only; a newline is not a valid name there.
    let cwd = dir.path().join(if cfg!(windows) { "100%" } else { "a\nb" });
    std::fs::create_dir(&cwd).unwrap();
    let tmp = indexed_in(&cwd.to_string_lossy());
    let text = stdout(bin(tmp.path()).args(["search", "nginx"]));
    assert!(
        text.contains(&format!(
            "resume: claude --resume {ID}  (run it in the folder above)\n"
        )),
        "{text}"
    );
}

/// Writes a fake `claude` that leaves `ran.txt` with its arguments in the
/// current folder, and returns the folder to put on `PATH`.
fn fake_claude(dir: &Path) -> PathBuf {
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    if cfg!(windows) {
        std::fs::write(bin.join("claude.cmd"), "@>ran.txt echo %*\r\n").unwrap();
    } else {
        let script = bin.join("claude");
        std::fs::write(&script, "#!/bin/sh\necho \"$@\" > ran.txt\n").unwrap();
        let chmod = std::process::Command::new("chmod")
            .arg("+x")
            .arg(&script)
            .status();
        assert!(chmod.unwrap().success());
    }
    bin
}

/// Runs a script file holding the pasted command in `shell`; `None` when the
/// shell is not installed.
fn run_in_shell(shell: &str, dir: &Path, command: &str) -> Option<std::process::Output> {
    let (ext, args): (&str, &[&str]) = match shell {
        "cmd" => ("bat", &["/d", "/c"]),
        "powershell" | "pwsh" => (
            "ps1",
            &["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"],
        ),
        _ => ("sh", &[]),
    };
    let script = dir.join(format!("paste.{ext}"));
    let text = if ext == "bat" {
        format!("@{command}\r\n")
    } else {
        format!("{command}\n")
    };
    std::fs::write(&script, text).unwrap();
    let mut path = vec![dir.join("bin")];
    path.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    std::process::Command::new(shell)
        .args(args)
        .arg(&script)
        .current_dir(dir)
        .env("PATH", std::env::join_paths(path).unwrap())
        .output()
        .ok()
}

/// The copied command, pasted into every shell of this OS, runs the agent in
/// the conversation's folder.
#[test]
fn copied_command_works_in_real_shells() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().join("my dir (x)' a;b,c=d");
    std::fs::create_dir(&cwd).unwrap();
    fake_claude(dir.path());
    let tmp = indexed_in(&cwd.to_string_lossy());
    let found = json_out(bin(tmp.path()).args(["search", "--json", "nginx"]));
    let command = found["results"][0]["resumeCommand"].as_str().unwrap();
    assert!(command.contains("cd "), "{command}");

    let (required, optional): (&[&str], &[&str]) = if cfg!(windows) {
        (&["cmd", "powershell"], &["pwsh"])
    } else {
        (&["sh"], &["bash", "zsh"])
    };
    for shell in required.iter().chain(optional) {
        let Some(out) = run_in_shell(shell, dir.path(), command) else {
            assert!(!required.contains(shell), "{shell} is not available");
            continue;
        };
        let ran = cwd.join("ran.txt");
        let args = std::fs::read_to_string(&ran)
            .unwrap_or_else(|e| panic!("{shell}: {e}; {command}; {out:?}"));
        assert_eq!(args.trim(), format!("--resume {ID}"), "{shell}");
        std::fs::remove_file(ran).unwrap();
    }
}
