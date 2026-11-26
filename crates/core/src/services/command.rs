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

/// A character that must not reach a terminal from a transcript: a control
/// character, or a bidirectional one that makes text read as something else
/// (embeddings, overrides, isolates and the marks LRM, RLM, ALM).
pub fn unprintable(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{200e}' | '\u{200f}' | '\u{061c}'
        )
}

/// Whether a pasted command can change to `workdir`: no [`unprintable`]
/// characters (controls act even inside quotes when pasted), and on Windows
/// nothing from [`ACTIVE_ON_WINDOWS`] and no network path.
pub fn can_cd(os: TargetOs, workdir: &str) -> bool {
    !workdir.contains(unprintable)
        && !(os == TargetOs::Windows && (workdir.contains(ACTIVE_ON_WINDOWS) || is_unc(workdir)))
}

/// `\\server\share`, `\\?\…`: cmd cannot have such a current folder.
pub fn is_unc(workdir: &str) -> bool {
    let mut chars = workdir.chars();
    (0..2).all(|_| matches!(chars.next(), Some('\\' | '/')))
}

/// Inside double quotes cmd expands `%…%` (which can yield a quote), PowerShell
/// runs `$(…)`, escapes with a backtick and takes typographic double quotes for
/// `"`. `|`, `<` and `>` are not valid in a Windows file name anyway.
const ACTIVE_ON_WINDOWS: [char; 10] = [
    '"', '%', '$', '`', '\u{201c}', '\u{201d}', '\u{201e}', '|', '<', '>',
];

/// The command a user can paste into their own terminal; without the `cd` part
/// when there is no safe way to write it.
pub fn copy_command(os: TargetOs, workdir: &str, argv: &[String]) -> String {
    if !can_cd(os, workdir) {
        return argv.join(" ");
    }
    match os {
        TargetOs::Windows => windows_command(workdir, argv),
        _ => posix_command(workdir, argv),
    }
}

/// `cd <workdir> && <argv>` for a POSIX shell.
pub fn posix_command(workdir: &str, argv: &[String]) -> String {
    let argv: Vec<_> = argv.iter().map(|a| sh_quote(a)).collect();
    format!("cd {} && {}", sh_quote(workdir), argv.join(" "))
}

/// Has to mean the same in cmd and in PowerShell (5.1 has no `&&`, none has
/// `cd /d`), hence `cmd /c`. Only that inner cmd sees the folder without quotes,
/// so `^` escapes its `&` and `^`; no space before `&&`, or it would become part
/// of the folder.
fn windows_command(workdir: &str, argv: &[String]) -> String {
    let dir = workdir.replace('^', "^^").replace('&', "^&");
    format!("cmd /c \"cd /d {dir}&& {}\"", argv.join(" "))
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
            r#"cmd /c "cd /d C:\w&& claude --resume 0a1b2c3d-1111-2222-3333-444455556666""#
        );
    }

    #[test]
    fn copy_command_drops_a_folder_that_cannot_be_quoted() {
        let plain = argv().join(" ");
        for os in [TargetOs::Mac, TargetOs::Linux, TargetOs::Windows] {
            assert_eq!(copy_command(os, "/a\n/b", &argv()), plain);
            assert_eq!(copy_command(os, "/a\u{1b}[201~b", &argv()), plain);
            assert_eq!(copy_command(os, "/a\u{202e}b", &argv()), plain);
        }
        for breakout in [
            r#"C:\w" & calc & ""#,
            r"C:\%CMDCMDLINE:~0,1% & calc",
            r"C:\$(calc)",
            r"C:\w`",
            "C:\\w\u{201d}; calc; \u{201c}",
            "C:\\w\u{201e}; calc",
            r"C:\w|calc",
            r"C:\w>x",
            r"C:\w<x",
            r"\\server\share",
            r"\\?\C:\w",
            "//server/share",
        ] {
            assert_eq!(copy_command(TargetOs::Windows, breakout, &argv()), plain);
        }
        // Inert in both shells: the folder stays.
        let inert = r"C:\Мои проекты\a;b,c=d!e (x)'";
        assert!(copy_command(TargetOs::Windows, inert, &argv())
            .starts_with(&format!("cmd /c \"cd /d {inert}&& ")));
        // Operators of the inner cmd are escaped, the escape itself included.
        assert!(
            copy_command(TargetOs::Windows, r"C:\R&D\a^& calc^", &argv())
                .starts_with(r#"cmd /c "cd /d C:\R^&D\a^^^& calc^^&& claude "#)
        );
        // Two leading slashes are just the root elsewhere.
        assert!(copy_command(TargetOs::Linux, "//w", &argv()).starts_with("cd //w && "));
        // Single quotes keep all of it literal elsewhere.
        assert!(
            copy_command(TargetOs::Linux, "/a%b$(c)`", &argv()).starts_with("cd '/a%b$(c)`' && ")
        );
        assert!(copy_command(TargetOs::Mac, "/a \"b\"", &argv()).starts_with("cd '/a \"b\"' && "));
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
        assert!("\u{1b}\u{9b}\u{202e}\u{2066}\u{200e}\u{200f}\u{61c}"
            .chars()
            .all(unprintable));
        assert!(!"яé 👩\u{200d}💻\u{fffd}".contains(unprintable));
        assert_eq!(applescript_str(r#"a"b\c"#), r#""a\"b\\c""#);
    }
}
