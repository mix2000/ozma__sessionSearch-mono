//! @guard — сторожа правил проекта для всего Rust-кода (`crates/*/src`): обходят
//! дерево и падают на КЛАССЕ нарушения, а не на одном случае. Общий текст для
//! ozma Session Search и ozma AI VPN Closer (scratchpad/shared-guards).
//!
//! - файл рабочего кода не длиннее [`MAX_FILE_LINES`] строк;
//! - функция вне `#[cfg(test)]` не длиннее [`MAX_FN_LINES`] строк;
//! - платформенный `cfg` (`target_os`, `unix`, `windows`, `target_family`) — только в `platform/`;
//! - слои ядра (`crates/core/src`): `models/` без ввода-вывода и чужих слоёв;
//!   `services/` без ОС (fs, process, сокеты, потоки, `platform::Os*`);
//!   `platform/` и `storage/` не знают о `services/`.

use std::path::{Path, PathBuf};

const MAX_FILE_LINES: usize = 500;
const MAX_FN_LINES: usize = 60;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("корень workspace")
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|e| e == "rs") {
            out.push(p);
        }
    }
}

/// Все `crates/*/src/**/*.rs` с текстом.
fn sources() -> Vec<(PathBuf, String)> {
    let mut files = Vec::new();
    for krate in std::fs::read_dir(workspace().join("crates"))
        .expect("crates/")
        .flatten()
    {
        rust_files(&krate.path().join("src"), &mut files);
    }
    files.sort();
    files
        .into_iter()
        .map(|p| {
            let text = std::fs::read_to_string(&p).expect("файл читается");
            (p, text)
        })
        .collect()
}

fn rel(p: &Path) -> String {
    p.strip_prefix(workspace())
        .unwrap_or(p)
        .display()
        .to_string()
        .replace('\\', "/")
}

/// Строки кода без комментариев (`//…`, `//!`, `///`).
fn code_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim_start().starts_with("//"))
}

fn report(what: &str, bad: Vec<String>) {
    assert!(bad.is_empty(), "{what}:\n{}", bad.join("\n"));
}

#[test]
fn guard_self_check_finds_tree() {
    let files = sources();
    assert!(
        files.len() > 20,
        "сторож нашёл всего {} файлов — обход сломан",
        files.len()
    );
    for layer in ["models", "services", "platform", "storage"] {
        assert!(
            files
                .iter()
                .any(|(p, _)| rel(p).starts_with(&format!("crates/core/src/{layer}/"))),
            "не найден слой crates/core/src/{layer}/ — обход сломан"
        );
    }
}

#[test]
fn guard_file_length_budget() {
    let bad = sources()
        .iter()
        .filter(|(_, t)| t.lines().count() > MAX_FILE_LINES)
        .map(|(p, t)| format!("{} → {} строк", rel(p), t.lines().count()))
        .collect();
    report(&format!("файлы длиннее {MAX_FILE_LINES} строк"), bad);
}

#[test]
fn guard_platform_cfg_only_in_platform() {
    let bad = sources()
        .iter()
        .filter(|(p, _)| !rel(p).split('/').any(|c| c == "platform"))
        .flat_map(|(p, t)| {
            code_lines(t)
                .filter(|(_, l)| platform_cfg(l))
                .map(|(n, l)| format!("{}:{} {}", rel(p), n + 1, l.trim()))
                .collect::<Vec<_>>()
        })
        .collect();
    report("платформенный cfg вне platform/", bad);
}

/// Условие `cfg(…)` / `cfg!(…)` / `cfg_attr(условие, …)` про платформу.
/// У `cfg_attr` смотрим только условие: `windows_subsystem = "windows"` — не оно.
fn platform_cfg(line: &str) -> bool {
    let l = line.replace(' ', "");
    let predicates: Vec<&str> = ["cfg(", "cfg!("]
        .iter()
        .flat_map(|k| l.match_indices(k).map(|(i, k)| &l[i + k.len()..]))
        .chain(
            l.match_indices("cfg_attr(")
                .map(|(i, k)| l[i + k.len()..].split(',').next().unwrap_or("")),
        )
        .collect();
    predicates.iter().any(|p| {
        ["target_os", "unix", "windows", "target_family"]
            .iter()
            .any(|m| p.contains(m))
    })
}

/// Запрещённые подстроки для файлов слоя.
fn layer_rules(layer: &str) -> &'static [&'static str] {
    const IO: &[&str] = &[
        "std::fs",
        "std::process",
        "UdpSocket",
        "TcpStream",
        "TcpListener",
        "ToSocketAddrs",
    ];
    match layer {
        "models" => &[
            "crate::services",
            "crate::storage",
            "crate::platform",
            "crate::engine",
            "std::fs",
            "std::process",
            "UdpSocket",
            "TcpStream",
            "TcpListener",
            "ToSocketAddrs",
            "Utc::now",
            "Local::now",
            "SystemTime::now",
            "Instant::now",
        ],
        "services" => &[
            "std::fs",
            "std::process",
            "UdpSocket",
            "TcpStream",
            "TcpListener",
            "ToSocketAddrs",
            "thread::spawn",
            "platform::Os",
            "platform::unix",
            "platform::macos",
            "platform::windows",
            "SysProcSource",
        ],
        "platform" | "storage" => &["crate::services"],
        _ => IO,
    }
}

#[test]
fn guard_core_layers() {
    let core = "crates/core/src/";
    let bad = sources()
        .iter()
        .filter_map(|(p, t)| {
            let r = rel(p);
            let layer = r.strip_prefix(core)?.split('/').next()?.to_string();
            ["models", "services", "platform", "storage"]
                .contains(&layer.as_str())
                .then_some((r, layer, t))
        })
        .flat_map(|(r, layer, t)| {
            let rules = layer_rules(&layer);
            let mut out = Vec::new();
            for (n, l) in production_lines(t) {
                for rule in rules.iter().filter(|rule| l.contains(*rule)) {
                    out.push(format!("{r}:{} [{layer}] «{rule}»: {}", n + 1, l.trim()));
                }
                // Конкретные реализации платформы (`OsNet`, `OsDns` …) — только в engine.
                if layer == "services" && has_os_type(l) {
                    out.push(format!("{r}:{} [services] Os*: {}", n + 1, l.trim()));
                }
            }
            out
        })
        .collect();
    report("нарушение слоёв ядра", bad);
}

/// Строки кода вне `#[cfg(test)]`-блоков: тестам сервиса можно писать временные файлы.
fn production_lines(text: &str) -> Vec<(usize, &str)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        if l.trim_start().starts_with("#[cfg(test)]") {
            i = skip_block(&lines, i + 1);
            continue;
        }
        if !l.trim_start().starts_with("//") {
            out.push((i, l));
        }
        i += 1;
    }
    out
}

/// Идентификатор вида `OsNet`, `OsProcessControl` (Os + заглавная буква).
fn has_os_type(line: &str) -> bool {
    line.match_indices("Os").any(|(i, _)| {
        let before = line[..i].chars().next_back();
        let after = line[i + 2..].chars().next();
        !before.is_some_and(|c| c.is_alphanumeric() || c == '_')
            && after.is_some_and(|c| c.is_ascii_uppercase())
    })
}

/// Функции вне тестовых модулей: (строка начала, длина в строках).
fn functions(text: &str) -> Vec<(usize, usize, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i].trim_start();
        if l.starts_with("#[cfg(test)]") {
            i = skip_block(&lines, i + 1);
            continue;
        }
        let is_fn = (l.starts_with("fn ") || l.contains(" fn ") || l.starts_with("pub fn "))
            && !l.starts_with("//")
            && l.contains('(');
        if is_fn && !l.trim_end().ends_with(';') {
            let end = skip_block(&lines, i);
            let name = l
                .split("fn ")
                .nth(1)
                .unwrap_or("")
                .split('(')
                .next()
                .unwrap_or("")
                .to_string();
            out.push((i + 1, end - i, name));
            i = end;
            continue;
        }
        i += 1;
    }
    out
}

/// Индекс строки после блока `{…}`, который начинается на строке `from` или позже.
fn skip_block(lines: &[&str], from: usize) -> usize {
    let mut depth = 0i32;
    let mut opened = false;
    for (k, line) in lines.iter().enumerate().skip(from) {
        let code = line.split("//").next().unwrap_or("");
        for c in code.chars() {
            match c {
                '{' => {
                    depth += 1;
                    opened = true;
                }
                '}' => depth -= 1,
                _ => {}
            }
        }
        if opened && depth <= 0 {
            return k + 1;
        }
        if !opened && code.trim_end().ends_with(';') {
            return k + 1;
        }
    }
    lines.len()
}

#[test]
fn guard_function_length() {
    let bad = sources()
        .iter()
        .flat_map(|(p, t)| {
            functions(t)
                .into_iter()
                .filter(|(_, len, _)| *len > MAX_FN_LINES)
                .map(|(line, len, name)| format!("{}:{line} fn {name} → {len} строк", rel(p)))
                .collect::<Vec<_>>()
        })
        .collect();
    report(&format!("функции длиннее {MAX_FN_LINES} строк"), bad);
}

#[test]
fn guard_detectors_work() {
    // Сторож без проверки своих детекторов ничего не стоит: прогон на образцах.
    let long = format!("fn a() {{\n{}}}\n", "    x();\n".repeat(MAX_FN_LINES));
    assert_eq!(functions(&long)[0].1, MAX_FN_LINES + 2);
    let tests = "#[cfg(test)]\nmod tests {\n    fn big() {\n    }\n}\nfn small() {\n}\n";
    let io_in_tests =
        "use a::b;\n#[cfg(test)]\nmod tests {\n    use std::fs;\n}\nuse std::process;\n";
    let prod: Vec<&str> = production_lines(io_in_tests)
        .into_iter()
        .map(|(_, l)| l)
        .collect();
    assert_eq!(prod, ["use a::b;", "use std::process;"]);
    let f = functions(tests);
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].2, "small");
    assert!(has_os_type("use crate::platform::{OsNet, NetProbe};"));
    assert!(!has_os_type("let pos = 1; // Ostrich"));
    assert!(!has_os_type("fn os_apply()"));
    assert!(platform_cfg("#[cfg(target_os = \"macos\")]"));
    assert!(platform_cfg("if cfg!(windows) {"));
    assert!(platform_cfg("#[cfg_attr(unix, allow(dead_code))]"));
    assert!(!platform_cfg(
        "#![cfg_attr(not(debug_assertions), windows_subsystem = \"windows\")]"
    ));
    assert!(!platform_cfg("#[cfg(test)]"));
}
