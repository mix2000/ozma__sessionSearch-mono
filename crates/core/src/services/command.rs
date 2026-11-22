//! Commands a user can paste into their own terminal, and the quoting rules
//! that keep untrusted paths inert.

use crate::models::TargetOs;

/// Command tokens (program, flags, ids): a charset that needs no quoting anywhere.
pub fn safe_token(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
}

/// Session ids: safe tokens of at most 128 bytes that cannot be mistaken for a
/// flag or a relative path (no leading `-` or `.`).
pub fn valid_id(id: &str) -> bool {
    safe_token(id) && id.len() <= 128 && !id.starts_with('-') && !id.starts_with('.')
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
pub fn applescript_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

pub fn has_control(s: &str) -> bool {
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
    fn posix_quoting() {
        assert_eq!(sh_quote("/Users/me/proj"), "/Users/me/proj");
        assert_eq!(sh_quote("/a b/it's $(rm)"), r"'/a b/it'\''s $(rm)'");
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
    fn tokens_and_control_chars() {
        assert!(safe_token("--resume"));
        assert!(!safe_token("a b") && !safe_token(""));
        assert!(has_control("a\nb") && !has_control("a b"));
        assert_eq!(applescript_str(r#"a"b\c"#), r#""a\"b\\c""#);
    }
}
