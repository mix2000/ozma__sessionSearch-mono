//! Query parsing and snippet building.

use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

static TERM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""([^"]+)"|(\S+)"#).expect("valid regex"));
static PARA_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n\s*\n").expect("valid regex"));

pub const MAX_TERMS: usize = 8;

/// Words and "quoted phrases" of a query (deduplicated, at most 8).
pub fn parse_query(q: &str) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();
    for c in TERM_RE.captures_iter(q) {
        let term = c
            .get(1)
            .or_else(|| c.get(2))
            .map_or("", |m| m.as_str())
            .trim();
        if !term.is_empty() && !terms.iter().any(|t| t == term) {
            terms.push(term.to_owned());
        }
    }
    terms.truncate(MAX_TERMS);
    terms
}

/// At least one term with 2+ characters is required.
pub fn query_is_valid(terms: &[String]) -> bool {
    !terms.is_empty() && !terms.iter().all(|t| t.chars().count() < 2)
}

/// Lowercase used both for the stored search copy and for terms.
pub fn fold(s: &str) -> String {
    s.to_lowercase()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Snippet {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(rename = "match")]
    pub matched: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

const PARA_LIMIT: usize = 700;

fn chars_slice(chars: &[char], start: usize, end: usize) -> String {
    chars[start.min(chars.len())..end.min(chars.len())]
        .iter()
        .collect()
}

fn clip(text: &str, limit: usize, around: Option<usize>) -> String {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    if len <= limit {
        return text.to_owned();
    }
    let Some(around) = around else {
        return chars_slice(&chars, 0, limit).trim_end().to_owned() + "…";
    };
    let start = around.saturating_sub(limit / 2).min(len - limit);
    let end = start + limit;
    let mut out = String::new();
    if start > 0 {
        out.push('…');
    }
    out.push_str(chars_slice(&chars, start, end).trim());
    if end < len {
        out.push('…');
    }
    out
}

/// Char index of the first case-insensitive occurrence of `needle_lc`
/// (already lowercased) in `hay`, in characters of the original `hay`.
fn find_ci(hay: &str, needle_lc: &str) -> Option<usize> {
    if needle_lc.is_empty() {
        return Some(0);
    }
    let mut lowered = String::with_capacity(hay.len());
    // For each byte offset in `lowered` where a source char starts: its char index.
    let mut starts: Vec<(usize, usize)> = Vec::with_capacity(hay.len());
    for (ci, ch) in hay.chars().enumerate() {
        starts.push((lowered.len(), ci));
        lowered.extend(ch.to_lowercase());
    }
    let byte = lowered.find(needle_lc)?;
    let idx = starts.partition_point(|(b, _)| *b <= byte);
    Some(starts[idx.saturating_sub(1)].1)
}

/// The paragraph with the match plus one paragraph before and after
/// (paragraphs are separated by an empty line; without empty lines — by lines).
pub fn build_snippet(text: &str, terms: &[String]) -> Snippet {
    let mut paras: Vec<&str> = PARA_RE
        .split(text)
        .filter(|p| !p.trim().is_empty())
        .collect();
    if paras.len() <= 1 {
        paras = text.split('\n').filter(|p| !p.trim().is_empty()).collect();
    }
    if paras.is_empty() {
        paras = vec![text];
    }
    let lowered: Vec<String> = terms.iter().map(|t| fold(t)).collect();
    let idx = paras
        .iter()
        .position(|p| {
            let pl = fold(p);
            lowered.iter().any(|t| pl.contains(t.as_str()))
        })
        .unwrap_or(0);
    let para = paras[idx].trim();
    let para_len = para.chars().count();
    let pos = lowered
        .iter()
        .filter_map(|t| find_ci(para, t))
        .min()
        .unwrap_or(para_len)
        .min(para_len);
    let before = (idx > 0).then(|| {
        let prev = paras[idx - 1].trim();
        let chars: Vec<char> = prev.chars().collect();
        if chars.len() > PARA_LIMIT / 2 {
            "…".to_owned() + &chars_slice(&chars, chars.len() - PARA_LIMIT / 2, chars.len())
        } else {
            prev.to_owned()
        }
    });
    let after = (idx + 1 < paras.len()).then(|| clip(paras[idx + 1].trim(), PARA_LIMIT / 2, None));
    Snippet {
        before,
        matched: clip(para, PARA_LIMIT, Some(pos)),
        after,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn words_and_quoted_phrases() {
        assert_eq!(
            parse_query(r#"deploy "nginx config" deploy"#),
            s(&["deploy", "nginx config"])
        );
        assert_eq!(parse_query(r#"  "  " a"#), s(&["a"]));
        assert_eq!(parse_query("1 2 3 4 5 6 7 8 9 10").len(), 8);
        assert_eq!(parse_query(r#""unterminated"#), s(&[r#""unterminated"#]));
    }

    #[test]
    fn validity() {
        assert!(!query_is_valid(&s(&[])));
        assert!(!query_is_valid(&s(&["a", "б"])));
        assert!(query_is_valid(&s(&["a", "бв"])));
    }

    #[test]
    fn snippet_paragraph_with_neighbours() {
        let text = "Первый абзац.\n\nВторой про Nginx конфиг.\n\nТретий.\n\nЧетвёртый.";
        assert_eq!(
            build_snippet(text, &s(&["nginx"])),
            Snippet {
                before: Some("Первый абзац.".into()),
                matched: "Второй про Nginx конфиг.".into(),
                after: Some("Третий.".into()),
            }
        );
    }

    #[test]
    fn snippet_long_paragraph_is_clipped_around_match() {
        let long = "a".repeat(2000) + " нужное " + &"b".repeat(2000);
        let sn = build_snippet(&long, &s(&["нужное"]));
        assert!(sn.matched.contains("нужное"));
        assert!(sn.matched.chars().count() < 720);
        assert!(sn.matched.starts_with('…') && sn.matched.ends_with('…'));
        assert_eq!(sn.before, None);
    }

    #[test]
    fn snippet_cyrillic_case_insensitive_and_line_split() {
        let text = "строка один\nПОЧИНИ деплой\nтри";
        let sn = build_snippet(text, &s(&["почини"]));
        assert_eq!(sn.matched, "ПОЧИНИ деплой");
        assert_eq!(sn.before.as_deref(), Some("строка один"));
        assert_eq!(sn.after.as_deref(), Some("три"));
    }

    #[test]
    fn snippet_neighbours_are_clipped() {
        let text = format!("{}\n\nmatch here\n\n{}", "x".repeat(1000), "y".repeat(1000));
        let sn = build_snippet(&text, &s(&["match"]));
        let before = sn.before.unwrap();
        assert!(before.starts_with('…') && before.chars().count() == 351);
        let after = sn.after.unwrap();
        assert!(after.ends_with('…') && after.chars().count() == 351);
    }

    #[test]
    fn find_ci_maps_to_original_chars() {
        assert_eq!(find_ci("АбВ где", "где"), Some(4));
        assert_eq!(find_ci("İx nginx", "nginx"), Some(3));
        assert_eq!(find_ci("abc", "zz"), None);
    }

    #[test]
    fn snippet_serializes_like_original() {
        let v = serde_json::to_value(Snippet {
            before: None,
            matched: "m".into(),
            after: None,
        })
        .unwrap();
        assert_eq!(v, serde_json::json!({ "match": "m" }));
    }
}
