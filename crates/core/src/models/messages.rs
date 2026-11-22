//! User-facing messages of the core (one place; the UI shows them as is).

/// Minimum characters in at least one query term (also sent to the UI in `/status`).
pub const MIN_QUERY_CHARS: usize = 2;

pub fn query_too_short() -> String {
    format!("Введите хотя бы {MIN_QUERY_CHARS} символа")
}

pub const BAD_SESSION_ID: &str = "Некорректный id разговора";
pub const SESSION_NOT_FOUND: &str = "Разговор не найден";
pub const WORKDIR_UNKNOWN: &str = "Папка разговора неизвестна";
pub const FILE_DELETED: &str =
    "Агент уже удалил файл этого разговора — продолжить нельзя, текст остался только в поиске";

pub fn already_open(pid: u32) -> String {
    format!("Разговор уже открыт в терминале (pid {pid})")
}

pub fn workdir_missing(workdir: &str) -> String {
    format!("Папка разговора не найдена: {workdir}")
}

pub fn launch_failed(e: &anyhow::Error) -> String {
    format!("Не удалось открыть терминал: {e:#}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts_are_unchanged() {
        assert_eq!(query_too_short(), "Введите хотя бы 2 символа");
        assert_eq!(already_open(7), "Разговор уже открыт в терминале (pid 7)");
        assert_eq!(workdir_missing("/w"), "Папка разговора не найдена: /w");
        assert_eq!(
            launch_failed(&anyhow::anyhow!("x")),
            "Не удалось открыть терминал: x"
        );
    }
}
