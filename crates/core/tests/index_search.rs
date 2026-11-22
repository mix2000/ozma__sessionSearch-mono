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

#[test]
fn index_search_context_incremental_resume() {
    let f = setup();
    let e = engine(&f);

    let r = e.sync();
    assert_eq!(r.files, 3, "subagent transcripts are not discovered");
    assert_eq!(r.updated, 3);
    // claude: user + 2 answers; headless: 2; codex: user + answer
    assert_eq!(r.messages, 7);

    let st = serde_json::to_value(e.status().unwrap()).unwrap();
    assert_eq!(st["sessions"], 3);
    assert_eq!(st["messages"], 7);
    assert_eq!(st["index"]["running"], false);
    assert!(st["index"]["lastRunAt"].is_string());
    assert_eq!(st["providers"][0]["provider"], "claude");
    assert_eq!(st["providers"][0]["sessions"], 2);
    assert_eq!(st["providers"][2]["exists"], false);

    // Second pass: nothing changed.
    assert_eq!(e.sync().updated, 0);

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
    assert!(claude.get("live").is_none());
    assert_eq!(results[1]["title"], "проверь сборку и nginx");
    assert_eq!(results[1]["gitBranch"], "dev");

    // Cyrillic case-insensitive, substring, all terms in one message.
    let v = search(&e, "деплой ПОЧИНИ", false);
    assert_eq!(v["results"].as_array().unwrap().len(), 1);
    assert_eq!(v["results"][0]["hits"][0]["seq"], 0);
    assert_eq!(
        search(&e, "refused", false)["results"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        search(&e, "\"nginx config\"", false)["results"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        search(&e, "secret", true)["results"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        search(&e, "reasoning", true)["results"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        search(&e, "починка готово", false)["results"]
            .as_array()
            .unwrap()
            .len(),
        0
    );

    // Headless included on demand; provider filter.
    assert_eq!(
        search(&e, "nginx", true)["results"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let only_codex = e
        .search(&SearchParams {
            q: "nginx".into(),
            provider: Some("codex".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(only_codex.results.len(), 1);
    assert_eq!(only_codex.results[0].id, CODEX_ID);

    // Too short query.
    let err = e
        .search(&SearchParams {
            q: "a".into(),
            ..Default::default()
        })
        .unwrap_err();
    assert_eq!(err.status, 400);

    // Context: 3 messages before/after.
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

    // Incremental: append a message (plus an unfinished line).
    {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&f.claude_file)
            .unwrap();
        writeln!(
            file,
            r#"{{"type":"user","entrypoint":"cli","timestamp":"2026-11-12T10:00:00.000Z","message":{{"content":"ещё вопрос про kubernetes"}}}}"#
        )
        .unwrap();
        write!(file, r#"{{"type":"assistant","message":{{"content":"half"#).unwrap();
    }
    let r = e.sync();
    assert_eq!((r.updated, r.messages), (1, 1));
    let v = search(&e, "kubernetes", false);
    assert_eq!(v["results"][0]["messageCount"], 4);
    assert_eq!(v["results"][0]["hits"][0]["seq"], 3);
    assert_eq!(v["results"][0]["lastAt"], "2026-11-12T10:00:00.000Z");
    assert_eq!(v["results"][0]["title"], "Починка деплоя");
    {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&f.claude_file)
            .unwrap();
        writeln!(file, r#" kubernetes done"}}}}"#).unwrap();
    }
    let r = e.sync();
    assert_eq!(
        (r.updated, r.messages),
        (1, 1),
        "the unfinished line is read once complete"
    );

    // A file that got shorter is reindexed from scratch.
    std::fs::write(
        &f.claude_file,
        fixture_text("claude-headless.jsonl", &f.workdir),
    )
    .unwrap();
    let r = e.sync();
    assert_eq!((r.updated, r.messages), (1, 2));
    assert_eq!(
        search(&e, "kubernetes", true)["results"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(e.status().unwrap().messages, 6);

    // Resume: launcher gets the workdir and the agent command.
    let ok = e.resume(CODEX_ID).unwrap();
    assert!(ok.ok);
    assert!(ok.command.ends_with(&format!("codex resume {CODEX_ID}")));
    let launched = f.launched.lock().unwrap().clone();
    assert_eq!(launched.len(), 1);
    assert_eq!(launched[0].0, f.workdir);
    assert_eq!(launched[0].1, ["codex", "resume", CODEX_ID]);

    assert_eq!(e.resume("nope; rm -rf /").unwrap_err().status, 400);
    assert_eq!(
        e.resume("00000000-0000-0000-0000-000000000000")
            .unwrap_err()
            .status,
        404
    );

    // Workdir gone → 409 with the command still returned.
    std::fs::remove_dir_all(&f.workdir).unwrap();
    let err = e.resume(CODEX_ID).unwrap_err();
    assert_eq!(err.status, 409);
    assert!(err.command.unwrap().contains("codex resume"));
    let v = search(&e, "сборку", false);
    assert_eq!(v["results"][0]["workdirExists"], false);
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
    assert_eq!(err.status, 409);
    assert!(err.command.unwrap().contains("claude --resume"));
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
