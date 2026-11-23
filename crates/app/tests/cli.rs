//! The binary end to end (`assert_cmd`): sync, status and search with `--json`
//! on a temporary index; errors and exit codes.

use std::path::Path;

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
