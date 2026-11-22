//! JSON helpers shared by the transcript parsers.

use chrono::{DateTime, Utc};
use serde_json::Value;

/// Same semantics as `new Date(value)` for the ISO strings agents write.
pub(crate) fn to_date(value: Option<&Value>) -> Option<DateTime<Utc>> {
    let s = value?.as_str()?;
    if let Ok(d) = DateTime::parse_from_rfc3339(s) {
        return Some(d.with_timezone(&Utc));
    }
    // ISO without zone: JS treats it as local time; we treat it as UTC.
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f")
        .ok()
        .map(|n| n.and_utc())
}

/// JavaScript truthiness of a JSON value.
pub(crate) fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

pub(crate) fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn dates_like_js() {
        let d = to_date(Some(&json!("2026-11-10T08:55:37.856Z"))).unwrap();
        assert_eq!(d.timestamp_millis(), 1_794_300_937_856);
        assert_eq!(
            to_date(Some(&json!("2026-11-10T08:55:37"))),
            "2026-11-10T08:55:37Z".parse().ok()
        );
        assert_eq!(to_date(Some(&json!(5))), None);
        assert_eq!(to_date(None), None);
    }

    #[test]
    fn js_truthiness() {
        for (v, t) in [
            (json!(null), false),
            (json!(false), false),
            (json!(0), false),
            (json!(""), false),
            (json!(1), true),
            (json!("x"), true),
            (json!([]), true),
            (json!({}), true),
        ] {
            assert_eq!(truthy(Some(&v)), t, "{v}");
        }
        assert!(!truthy(None));
        assert_eq!(str_field(&json!({"a": "b"}), "a").as_deref(), Some("b"));
        assert_eq!(str_field(&json!({"a": 1}), "a"), None);
    }
}
