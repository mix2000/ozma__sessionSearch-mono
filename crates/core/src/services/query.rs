//! Query parsing: words and "quoted phrases".

use std::sync::LazyLock;

use regex::Regex;

pub use crate::models::messages::MIN_QUERY_CHARS;
pub use crate::models::search::fold;

static TERM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""([^"]+)"|(\S+)"#).expect("valid regex"));

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

/// At least one term with [`MIN_QUERY_CHARS`]+ characters is required.
pub fn query_is_valid(terms: &[String]) -> bool {
    !terms.is_empty() && !terms.iter().all(|t| t.chars().count() < MIN_QUERY_CHARS)
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
}
