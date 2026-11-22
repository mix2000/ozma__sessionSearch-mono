//! Indexes fixture transcripts of Claude and Codex in a temp dir and checks
//! search, context, incremental sync and resume.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use session_search_core::{Engine, EngineConfig, MacTerminal, Provider, Roots, SearchParams};

const CLAUDE_ID: &str = "11111111-2222-3333-4444-555555555555";
const HEADLESS_ID: &str = "99999999-2222-3333-4444-555555555555";
const CODEX_ID: &str = "01a09d3b-58db-7550-bf6b-93f273e29e1a";

type Launched = Arc<Mutex<Vec<(PathBuf, Vec<String>)>>>;

struct Fixture {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    workdir: PathBuf,
    claude_file: PathBuf,
    launched: Launched,
}

fn fixture_text(name: &str, workdir: &Path) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let workdir_json = serde_json::to_string(&workdir.to_string_lossy()).unwrap();
    std::fs::read_to_string(p)
        .unwrap()
        .replace("\"__WORKDIR__\"", &workdir_json)
}

fn setup() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    let workdir = root.join("work dir");
    std::fs::create_dir_all(&workdir).unwrap();
    let project = root.join("claude/projects/-work-dir");
    std::fs::create_dir_all(project.join(CLAUDE_ID).join("subagents")).unwrap();
    let claude_file = project.join(format!("{CLAUDE_ID}.jsonl"));
    std::fs::write(&claude_file, fixture_text("claude-session.jsonl", &workdir)).unwrap();
    std::fs::write(
        project.join(format!("{HEADLESS_ID}.jsonl")),
        fixture_text("claude-headless.jsonl", &workdir),
    )
    .unwrap();
    std::fs::write(
        project.join(CLAUDE_ID).join("subagents/agent-a1.jsonl"),
        fixture_text("claude-subagent.jsonl", &workdir),
    )
    .unwrap();
    let codex_dir = root.join("codex/sessions/2026/09/14");
    std::fs::create_dir_all(&codex_dir).unwrap();
    std::fs::write(
        codex_dir.join(format!("rollout-2026-09-14T03-05-07-{CODEX_ID}.jsonl")),
        fixture_text("codex-session.jsonl", &workdir),
    )
    .unwrap();
    Fixture {
        _tmp: tmp,
        workdir,
        claude_file,
        launched: Arc::new(Mutex::new(Vec::new())),
        root,
    }
}

fn engine(f: &Fixture) -> Engine {
    let launched = f.launched.clone();
    Engine::open(EngineConfig {
        db_path: f.root.join("data/index.sqlite"),
        roots: Roots {
            claude: f.root.join("claude/projects"),
            codex: f.root.join("codex/sessions"),
            grok: f.root.join("grok"),
        },
        providers: Provider::ALL.to_vec(),
        mac_terminal: MacTerminal::Terminal,
    })
    .unwrap()
    .with_launcher(Box::new(move |dir, argv| {
        launched
            .lock()
            .unwrap()
            .push((dir.to_path_buf(), argv.to_vec()));
        Ok(())
    }))
}

fn search(e: &Engine, q: &str, headless: bool) -> serde_json::Value {
    let r = e
        .search(&SearchParams {
            q: q.into(),
            include_headless: headless,
            ..Default::default()
        })
        .unwrap();
    serde_json::to_value(r).unwrap()
}

/// Fixture indexed once.
fn synced() -> (Fixture, Engine) {
    let f = setup();
    let e = engine(&f);
    e.sync();
    (f, e)
}

fn count(e: &Engine, q: &str, headless: bool) -> usize {
    search(e, q, headless)["results"].as_array().unwrap().len()
}

fn append(f: &Fixture, text: &str) {
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&f.claude_file)
        .unwrap();
    write!(file, "{text}").unwrap();
}

#[test]
fn first_sync_indexes_main_transcripts_only() {
    let f = setup();
    let e = engine(&f);
    let r = e.sync();
    assert_eq!(r.files, 3, "subagent transcripts are not discovered");
    assert_eq!(r.updated, 3);
    // claude: user + 2 answers; headless: 2; codex: user + answer
    assert_eq!(r.messages, 7);
    // Second pass: nothing changed.
    assert_eq!(e.sync().updated, 0);
}

#[test]
fn status_counts_sessions_per_provider() {
    let (_f, e) = synced();
    let st = serde_json::to_value(e.status().unwrap()).unwrap();
    assert_eq!(st["sessions"], 3);
    assert_eq!(st["messages"], 7);
    assert_eq!(st["minQueryLength"], 2);
    assert_eq!(st["index"]["running"], false);
    assert!(st["index"]["lastRunAt"].is_string());
    assert_eq!(st["providers"][0]["provider"], "claude");
    assert_eq!(st["providers"][0]["sessions"], 2);
    assert_eq!(st["providers"][2]["exists"], false);
}

#[test]
fn search_returns_fresh_sessions_with_card_fields() {
    let (_f, e) = synced();
    // Interactive only by default; Codex message is older → after Claude.
    let v = search(&e, "nginx", false);
    assert_eq!(v["terms"], serde_json::json!(["nginx"]));
    assert_eq!(v["truncated"], false);
    let results = v["results"].as_array().unwrap();
    let ids: Vec<&str> = results.iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert_eq!(ids, [CLAUDE_ID, CODEX_ID]);
    let claude = &results[0];
    assert_eq!(claude["provider"], "claude");
    assert_eq!(claude["title"], "Починка деплоя");
    assert_eq!(claude["kind"], "interactive");
    assert_eq!(claude["gitBranch"], "master");
    assert_eq!(claude["startedAt"], "2026-11-10T08:55:37.856Z");
    assert_eq!(claude["lastAt"], "2026-11-10T08:56:00.000Z");
    assert_eq!(claude["messageCount"], 3);
    assert_eq!(claude["workdirExists"], true);
    assert!(claude.get("live").is_none());
    assert_eq!(results[1]["title"], "проверь сборку и nginx");
    assert_eq!(results[1]["gitBranch"], "dev");
}

#[test]
fn hits_carry_a_paragraph_snippet() {
    let (_f, e) = synced();
    let claude = &search(&e, "nginx", false)["results"][0];
    assert_eq!(
        claude["matches"], 2,
        "tool output, thinking, sidechain, compact summary skipped"
    );
    let hit = &claude["hits"][0];
    assert_eq!(hit["seq"], 1);
    assert_eq!(hit["role"], "assistant");
    assert!(hit["messageId"].is_i64());
    assert_eq!(hit["snippet"]["before"], "Смотрю конфиг.");
    assert_eq!(
        hit["snippet"]["match"],
        "Нашёл ошибку в nginx config: ECONNREFUSED на upstream."
    );
    assert_eq!(hit["snippet"]["after"], "Исправляю.");
}

#[test]
fn all_terms_must_occur_in_one_indexed_message() {
    let (_f, e) = synced();
    // Cyrillic case-insensitive, substring, all terms in one message.
    let v = search(&e, "деплой ПОЧИНИ", false);
    assert_eq!(v["results"].as_array().unwrap().len(), 1);
    assert_eq!(v["results"][0]["hits"][0]["seq"], 0);
    assert_eq!(count(&e, "refused", false), 1);
    assert_eq!(count(&e, "\"nginx config\"", false), 1);
    assert_eq!(count(&e, "починка готово", false), 0);
    // Tool output and thinking are never indexed.
    assert_eq!(count(&e, "secret", true), 0);
    assert_eq!(count(&e, "reasoning", true), 0);
}

#[test]
fn headless_on_demand_provider_filter_and_short_query() {
    let (_f, e) = synced();
    assert_eq!(count(&e, "nginx", true), 3);
    let only_codex = e
        .search(&SearchParams {
            q: "nginx".into(),
            provider: Some("codex".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(only_codex.results.len(), 1);
    assert_eq!(only_codex.results[0].id, CODEX_ID);
    let err = e
        .search(&SearchParams {
            q: "a".into(),
            ..Default::default()
        })
        .unwrap_err();
    assert_eq!(
        (err.status(), err.message()),
        (400, "Введите хотя бы 2 символа")
    );
}

#[test]
fn context_shows_messages_around_a_hit() {
    let (_f, e) = synced();
    let ctx = serde_json::to_value(e.context(CLAUDE_ID, 1).unwrap()).unwrap();
    let seqs: Vec<i64> = ctx
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["seq"].as_i64().unwrap())
        .collect();
    assert_eq!(seqs, [0, 1, 2]);
    assert_eq!(ctx[0]["role"], "user");
    assert_eq!(ctx[0]["text"], "Почини ДЕПЛОЙ nginx");
    assert_eq!(ctx[0]["at"], "2026-11-10T08:55:37.856Z");
}

#[test]
fn appended_lines_are_indexed_incrementally() {
    let (f, e) = synced();
    append(
        &f,
        "{\"type\":\"user\",\"entrypoint\":\"cli\",\"timestamp\":\"2026-11-12T10:00:00.000Z\",\"message\":{\"content\":\"ещё вопрос про kubernetes\"}}\n",
    );
    append(&f, r#"{"type":"assistant","message":{"content":"half"#);
    let r = e.sync();
    assert_eq!((r.updated, r.messages), (1, 1));
    let v = search(&e, "kubernetes", false);
    assert_eq!(v["results"][0]["messageCount"], 4);
    assert_eq!(v["results"][0]["hits"][0]["seq"], 3);
    assert_eq!(v["results"][0]["lastAt"], "2026-11-12T10:00:00.000Z");
    assert_eq!(v["results"][0]["title"], "Починка деплоя");
    append(&f, " kubernetes done\"}}\n");
    let r = e.sync();
    assert_eq!(
        (r.updated, r.messages),
        (1, 1),
        "the unfinished line is read once complete"
    );
}

#[test]
fn a_shorter_file_is_reindexed_from_scratch() {
    let (f, e) = synced();
    std::fs::write(
        &f.claude_file,
        fixture_text("claude-headless.jsonl", &f.workdir),
    )
    .unwrap();
    let r = e.sync();
    assert_eq!((r.updated, r.messages), (1, 2));
    assert_eq!(count(&e, "ДЕПЛОЙ", true), 0);
    assert_eq!(e.status().unwrap().messages, 6);
}

#[test]
fn resume_launches_the_agent_in_its_workdir() {
    let (f, e) = synced();
    let ok = e.resume(CODEX_ID).unwrap();
    assert!(ok.ok);
    assert!(ok.command.ends_with(&format!("codex resume {CODEX_ID}")));
    let launched = f.launched.lock().unwrap().clone();
    assert_eq!(launched.len(), 1);
    assert_eq!(launched[0].0, f.workdir);
    assert_eq!(launched[0].1, ["codex", "resume", CODEX_ID]);
}

#[test]
fn resume_rejects_bad_and_unknown_ids() {
    let (_f, e) = synced();
    assert_eq!(e.resume("nope; rm -rf /").unwrap_err().status(), 400);
    let missing = e
        .resume("00000000-0000-0000-0000-000000000000")
        .unwrap_err();
    assert_eq!(
        (missing.status(), missing.message()),
        (404, "Разговор не найден")
    );
}

#[test]
fn resume_refused_when_the_workdir_is_gone() {
    let (f, e) = synced();
    std::fs::remove_dir_all(&f.workdir).unwrap();
    let err = e.resume(CODEX_ID).unwrap_err();
    assert_eq!(err.status(), 409);
    assert!(err.command().unwrap().contains("codex resume"));
    assert!(f.launched.lock().unwrap().is_empty());
    assert_eq!(
        search(&e, "сборку", false)["results"][0]["workdirExists"],
        false
    );
}

#[test]
fn deleted_transcripts_stay_searchable() {
    let f = setup();
    let e = engine(&f);
    e.sync();
    std::fs::remove_file(&f.claude_file).unwrap();
    e.sync();
    let v = search(&e, "ДЕПЛОЙ", false);
    assert_eq!(v["results"].as_array().unwrap().len(), 1);
    let err = e.resume(CLAUDE_ID).unwrap_err();
    assert_eq!(err.status(), 409);
    assert!(err.command().unwrap().contains("claude --resume"));
}

#[test]
fn reopening_keeps_the_index() {
    let f = setup();
    {
        let e = engine(&f);
        e.sync();
    }
    let e = engine(&f);
    assert_eq!(e.status().unwrap().sessions, 3);
    assert_eq!(e.sync().updated, 0);
}
