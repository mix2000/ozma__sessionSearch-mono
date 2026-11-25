//! Grok sources: Grok Build (`sessions/<encoded cwd>/<id>/updates.jsonl`) and
//! grok-cli (`grok.db`).

use std::io::Write;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
use serde_json::{json, Value};
use session_search_core::{Engine, EngineConfig, MacTerminal, Provider, Roots, SearchParams};

const BUILD_ID: &str = "019a1b2c-3d4e-7f00-8a1b-2c3d4e5f6a7b";
const HASHED_ID: &str = "019a1b2c-3d4e-7f00-8a1b-000000000002";
const HEADLESS_ID: &str = "019a1b2c-3d4e-7f00-8a1b-000000000003";
const SUBAGENT_ID: &str = "019a1b2c-3d4e-7f00-8a1b-000000000004";
const HIDDEN_ID: &str = "019a1b2c-3d4e-7f00-8a1b-000000000005";
const CLI_ID: &str = "cli-session-1";

fn encode(path: &Path) -> String {
    path.to_string_lossy()
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

fn upd(ts: i64, tag: &str, content: Value, meta: Value) -> String {
    json!({"timestamp": ts, "method": "session/update",
        "params": {"sessionId": "s", "update": {"sessionUpdate": tag, "content": content, "_meta": meta}}})
    .to_string()
}

fn text(s: &str) -> Value {
    json!({"type": "text", "text": s})
}

fn x_upd(ts: i64, update: Value) -> String {
    json!({"timestamp": ts, "method": "_x.ai/session/update", "params": {"sessionId": "s", "update": update}})
        .to_string()
}

fn write_session(dir: &Path, summary: Value, lines: &[String]) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("summary.json"), summary.to_string()).unwrap();
    std::fs::write(
        dir.join("chat_history.jsonl"),
        "{\"text\":\"chat history must not be indexed\"}\n",
    )
    .unwrap();
    let p = dir.join("updates.jsonl");
    std::fs::write(
        &p,
        lines.iter().map(|l| format!("{l}\n")).collect::<String>(),
    )
    .unwrap();
    p
}

fn append(p: &Path, lines: &[String]) {
    let mut f = std::fs::OpenOptions::new().append(true).open(p).unwrap();
    for l in lines {
        writeln!(f, "{l}").unwrap();
    }
}

fn engine(root: &Path) -> Engine {
    Engine::open(EngineConfig {
        db_path: root.join("data/index.sqlite"),
        roots: Roots {
            claude: root.join("none/projects"),
            codex: root.join("none/sessions"),
            grok: root.join("grok"),
        },
        providers: vec![Provider::Grok],
        mac_terminal: MacTerminal::Terminal,
    })
    .unwrap()
    .with_launcher(Box::new(|_, _| Ok(())))
}

fn search(e: &Engine, q: &str, headless: bool) -> Value {
    serde_json::to_value(
        e.search(&SearchParams {
            q: q.into(),
            include_headless: headless,
            ..Default::default()
        })
        .unwrap(),
    )
    .unwrap()
}

fn count(v: &Value) -> usize {
    v["results"].as_array().unwrap().len()
}

/// Grok Build sessions: a main one (streaming reply at EOF), one in a hashed
/// folder, a headless, a subagent and a hidden one.
struct Build {
    tmp: tempfile::TempDir,
    workdir: PathBuf,
    main: PathBuf,
}

fn build_fixture() -> Build {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let workdir = root.join("my proj");
    std::fs::create_dir_all(&workdir).unwrap();
    let sessions = root.join("grok/sessions");
    let group = sessions.join(encode(&workdir));
    let main = write_main_session(&group);
    write_other_sessions(&sessions, &group, &workdir);
    Build { workdir, main, tmp }
}

/// The main session: joined prompt chunks, a thought, a tool call, a shell
/// command and a reply still being streamed at EOF.
fn write_main_session(group: &Path) -> PathBuf {
    write_session(
        &group.join(BUILD_ID),
        json!({"info": {"id": BUILD_ID}, "generated_title": "Деплой nginx",
               "created_at": "2026-11-01T10:00:00Z", "last_active_at": "2026-11-01T10:05:00Z",
               "head_branch": "main"}),
        &[
            upd(1_793_000_000, "user_message_chunk", text("почини"), json!({"promptIndex": 0})),
            upd(1_793_000_001, "user_message_chunk", text("деплой nginx"), json!({"promptIndex": 0})),
            upd(1_793_000_002, "agent_thought_chunk", text("secret thought"), json!({})),
            upd(1_793_000_003, "agent_message_chunk", text("Смотрю "), json!({})),
            upd(1_793_000_004, "agent_message_chunk", text("конфиг nginx."), json!({})),
            json!({"timestamp": 1_793_000_005, "method": "session/update",
                   "params": {"update": {"sessionUpdate": "tool_call", "title": "Read nginx.conf"}}})
            .to_string(),
            upd(1_793_000_006, "user_message_chunk",
                json!({"type": "text", "text": "! cat nginx", "_meta": {"bash_command": "cat nginx"}}), json!({})),
            // Still being streamed at EOF.
            upd(1_793_000_007, "agent_message_chunk", text("Начинаю"), json!({})),
        ],
    )
}

/// A hashed-folder session (cwd in `.cwd`), a headless, a subagent and a hidden one.
fn write_other_sessions(sessions: &Path, group: &Path, workdir: &Path) {
    // Long cwd: hashed folder name + .cwd file.
    let hashed = sessions.join("my-proj-0123456789abcdef");
    std::fs::create_dir_all(&hashed).unwrap();
    std::fs::write(hashed.join(".cwd"), workdir.to_string_lossy().as_bytes()).unwrap();
    write_session(
        &hashed.join(HASHED_ID),
        json!({"session_summary": "Hashed"}),
        &[upd(
            1_792_000_000,
            "user_message_chunk",
            text("hashed nginx"),
            json!({"promptIndex": 0}),
        )],
    );
    write_session(
        &group.join(HEADLESS_ID),
        json!({"session_kind": "headless"}),
        &[upd(
            1_791_000_000,
            "user_message_chunk",
            text("nightly nginx"),
            json!({}),
        )],
    );
    write_session(
        &group.join(SUBAGENT_ID),
        json!({"session_kind": "subagent"}),
        &[upd(
            1,
            "user_message_chunk",
            text("subagent nginx"),
            json!({}),
        )],
    );
    write_session(
        &group.join(HIDDEN_ID),
        json!({"hidden": true}),
        &[upd(
            1,
            "user_message_chunk",
            text("hidden nginx"),
            json!({}),
        )],
    );
}

/// The Build fixture indexed once.
fn build_synced() -> (Build, Engine) {
    let b = build_fixture();
    let e = engine(b.tmp.path());
    e.sync();
    (b, e)
}

#[test]
fn build_skips_subagent_and_hidden_sessions() {
    let b = build_fixture();
    let r = engine(b.tmp.path()).sync();
    assert_eq!(r.files, 3, "subagent and hidden sessions are skipped");
    assert_eq!(r.messages, 3 + 1 + 1);
}

#[test]
fn build_session_cards() {
    let (b, e) = build_synced();
    let v = search(&e, "nginx", false);
    let ids: Vec<&str> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [BUILD_ID, HASHED_ID], "headless excluded by default");
    let main_r = &v["results"][0];
    assert_eq!(main_r["provider"], "grok");
    assert_eq!(main_r["title"], "Деплой nginx");
    assert_eq!(main_r["gitBranch"], "main");
    assert_eq!(main_r["kind"], "interactive");
    assert_eq!(main_r["workdir"], b.workdir.to_string_lossy().as_ref());
    assert_eq!(main_r["workdirExists"], true);
    assert_eq!(main_r["startedAt"], "2026-11-01T10:00:00.000Z");
    assert!(main_r["resumeCommand"]
        .as_str()
        .unwrap()
        .contains(&format!("grok --resume {BUILD_ID}")));
    assert_eq!(main_r["matches"], 2);
    let hashed = &v["results"][1];
    assert_eq!(hashed["workdir"], b.workdir.to_string_lossy().as_ref());
    assert_eq!(hashed["title"], "Hashed");
    assert_eq!(count(&search(&e, "nginx", true)), 3);
}

#[test]
fn build_chunks_are_joined_and_service_parts_skipped() {
    let (_b, e) = build_synced();
    assert_eq!(
        count(&search(&e, "\"почини деплой\"", false)),
        0,
        "prompt chunks are joined by a paragraph break"
    );
    assert_eq!(count(&search(&e, "почини деплой", false)), 1);
    assert_eq!(
        count(&search(&e, "\"Смотрю конфиг\"", false)),
        1,
        "agent chunks are concatenated"
    );
    for hidden in [
        "thought",
        "cat nginx",
        "Read nginx.conf",
        "history",
        "subagent",
    ] {
        assert_eq!(count(&search(&e, hidden, true)), 0, "{hidden}");
    }
    assert_eq!(
        count(&search(&e, "Начинаю", false)),
        1,
        "the open reply is searchable"
    );
}

#[test]
fn build_streamed_reply_stays_one_message() {
    let (b, e) = build_synced();
    append(
        &b.main,
        &[
            upd(
                1_793_000_008,
                "agent_message_chunk",
                text(" проверку."),
                json!({}),
            ),
            x_upd(1_793_000_009, json!({"sessionUpdate": "turn_completed"})),
        ],
    );
    let r = e.sync();
    assert_eq!((r.updated, r.messages), (1, 0));
    let v = search(&e, "\"Начинаю проверку\"", false);
    assert_eq!(v["results"][0]["messageCount"], 3);
    assert_eq!(v["results"][0]["hits"][0]["seq"], 2);
    assert_eq!(count(&search(&e, "\"Начинаю проверку.\"", false)), 1);
}

#[test]
fn build_rewind_drops_the_dead_branch() {
    let (b, e) = build_synced();
    let turn = |ts| x_upd(ts, json!({"sessionUpdate": "turn_completed"}));
    append(
        &b.main,
        &[
            upd(
                1_793_000_010,
                "user_message_chunk",
                text("kubernetes вопрос"),
                json!({"promptIndex": 1}),
            ),
            upd(
                1_793_000_011,
                "agent_message_chunk",
                text("kubernetes ответ"),
                json!({}),
            ),
            turn(1_793_000_012),
        ],
    );
    assert_eq!(e.sync().messages, 2);
    assert_eq!(count(&search(&e, "kubernetes", false)), 1);
    append(
        &b.main,
        &[
            x_upd(
                1_793_000_013,
                json!({"sessionUpdate": "rewind_marker", "target_prompt_index": 0}),
            ),
            upd(
                1_793_000_014,
                "user_message_chunk",
                text("docker вместо"),
                json!({"promptIndex": 1}),
            ),
            turn(1_793_000_015),
        ],
    );
    e.sync();
    assert_eq!(
        count(&search(&e, "kubernetes", false)),
        0,
        "rewound branch removed"
    );
    assert_eq!(search(&e, "docker", false)["results"][0]["messageCount"], 4);
}

#[test]
fn build_context_and_resume() {
    let (_b, e) = build_synced();
    let ctx = e.context(BUILD_ID, 0).unwrap();
    assert_eq!(ctx[0].text, "почини\n\nдеплой nginx");
    assert!(e
        .resume(BUILD_ID)
        .unwrap()
        .command
        .contains("grok --resume"));
}

fn make_cli_db(path: &Path) -> Connection {
    let c = Connection::open(path).unwrap();
    c.pragma_update(None, "journal_mode", "WAL").unwrap();
    c.execute_batch(
        "CREATE TABLE workspaces (id TEXT PRIMARY KEY, canonical_path TEXT, git_root TEXT, display_name TEXT);
         CREATE TABLE sessions (id TEXT PRIMARY KEY, workspace_id TEXT, title TEXT, recap_text TEXT,
             model TEXT, mode TEXT, cwd_at_start TEXT, cwd_last TEXT, status TEXT,
             created_at TEXT, updated_at TEXT);
         CREATE TABLE messages (session_id TEXT, seq INTEGER, role TEXT, message_json TEXT, created_at TEXT,
             PRIMARY KEY (session_id, seq));",
    )
    .unwrap();
    c
}

/// A grok-cli database with one session; the connection stays open (WAL)
/// like grok-cli's own while we read.
struct Cli {
    tmp: tempfile::TempDir,
    workdir: PathBuf,
    src: Connection,
}

fn cli_fixture() -> Cli {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let workdir = root.join("ws");
    std::fs::create_dir_all(&workdir).unwrap();
    std::fs::create_dir_all(root.join("grok")).unwrap();
    let db_path = root.join("grok/grok.db");
    let src = make_cli_db(&db_path);
    src.execute(
        "INSERT INTO workspaces (id, canonical_path) VALUES ('w1', ?1)",
        [workdir.to_string_lossy()],
    )
    .unwrap();
    src.execute(
        "INSERT INTO sessions (id, workspace_id, title, created_at, updated_at)
         VALUES (?1, 'w1', NULL, '2026-11-01T10:00:00.000Z', '2026-11-01T10:01:00.000Z')",
        [CLI_ID],
    )
    .unwrap();
    let msgs = [
        (
            0,
            "system",
            json!({"role": "system", "content": "system prompt nginx"}),
        ),
        (
            1,
            "user",
            json!({"role": "user", "content": "настрой nginx"}),
        ),
        (
            2,
            "assistant",
            json!({"role": "assistant", "content": [
            {"type": "reasoning", "text": "reasoning nginx"},
            {"type": "text", "text": "Готово, nginx настроен."},
            {"type": "tool-call", "toolName": "bash", "input": {"cmd": "nginx -t"}}]}),
        ),
        (
            3,
            "tool",
            json!({"role": "tool", "content": [{"type": "tool-result", "output": "nginx ok"}]}),
        ),
    ];
    for (seq, role, j) in &msgs {
        src.execute(
            "INSERT INTO messages VALUES (?1, ?2, ?3, ?4, '2026-11-01 10:00:30')",
            params![CLI_ID, seq, role, j.to_string()],
        )
        .unwrap();
    }

    Cli { tmp, workdir, src }
}

fn cli_synced() -> (Cli, Engine) {
    let c = cli_fixture();
    let e = engine(c.tmp.path());
    let r = e.sync();
    assert_eq!((r.updated, r.messages), (1, 2));
    (c, e)
}

#[test]
fn cli_session_card() {
    let (c, e) = cli_synced();
    let v = search(&e, "nginx", true);
    assert_eq!(count(&v), 1);
    let res = &v["results"][0];
    assert_eq!(res["id"], CLI_ID);
    assert_eq!(res["provider"], "grok");
    assert_eq!(res["kind"], "interactive");
    assert_eq!(res["title"], "настрой nginx");
    assert_eq!(res["workdir"], c.workdir.to_string_lossy().as_ref());
    assert_eq!(
        res["matches"], 2,
        "system, tool and reasoning parts are skipped"
    );
    assert_eq!(res["lastAt"], "2026-11-01T10:01:00.000Z");
    assert!(res["resumeCommand"]
        .as_str()
        .unwrap()
        .contains(&format!("grok -s {CLI_ID}")));
    // Unchanged database: nothing to do.
    assert_eq!(e.sync().updated, 0);
}

#[test]
fn cli_new_messages_are_imported_incrementally() {
    let (c, e) = cli_synced();
    let msg = json!({"role": "user", "content": [{"type": "text", "text": "и ещё redis"}]});
    c.src
        .execute(
            "INSERT INTO messages VALUES (?1, 4, 'user', ?2, '2026-11-02T09:00:00.000Z')",
            params![CLI_ID, msg.to_string()],
        )
        .unwrap();
    c.src
        .execute(
            "UPDATE sessions SET updated_at = '2026-11-02T09:00:00.000Z', title = 'Nginx и Redis', cwd_last = ?2 WHERE id = ?1",
            params![CLI_ID, c.workdir.to_string_lossy()],
        )
        .unwrap();
    let r = e.sync();
    assert_eq!((r.updated, r.messages), (1, 1));
    let v = search(&e, "redis", false);
    assert_eq!(v["results"][0]["messageCount"], 3);
    assert_eq!(v["results"][0]["title"], "Nginx и Redis");
    assert_eq!(v["results"][0]["hits"][0]["seq"], 2);
}

#[test]
fn cli_resume_uses_dash_s_and_never_writes_the_db() {
    let (c, e) = cli_synced();
    let ok = e.resume(CLI_ID).unwrap();
    assert!(ok.command.contains(&format!("grok -s {CLI_ID}")));
    let n: i64 = c
        .src
        .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 4);
}
