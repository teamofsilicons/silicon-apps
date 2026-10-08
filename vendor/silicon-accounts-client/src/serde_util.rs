//! Lenient serde helpers. The service may add fields or send `null` where a value is
//! usually present; a client should keep working instead of failing a whole command.

use serde::Deserialize;
use serde::de::{self, Deserializer};
use serde_json::Value;

time::serde::format_description!(pub(crate) ymd, Date, "[year]-[month]-[day]");

/// `null`/missing → `""`; numbers and booleans → their text.
pub(crate) fn lenient_string<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<String, D::Error> {
    Ok(lenient_opt_string(deserializer)?.unwrap_or_default())
}

/// `null`/missing → `None`; numbers and booleans → their text.
pub(crate) fn lenient_opt_string<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    match Option::<Value>::deserialize(deserializer)? {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(Value::Number(n)) => Ok(Some(n.to_string())),
        Some(Value::Bool(b)) => Ok(Some(b.to_string())),
        Some(other) => Err(de::Error::custom(format!(
            "expected a string, found {}",
            describe(&other)
        ))),
    }
}

/// `null`/missing → empty list.
pub(crate) fn lenient_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}

/// `null`/missing → `false`.
pub(crate) fn lenient_bool<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    Ok(Option::<bool>::deserialize(deserializer)?.unwrap_or_default())
}

/// `null`/missing → 0.
pub(crate) fn lenient_u64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    Ok(Option::<u64>::deserialize(deserializer)?.unwrap_or_default())
}

/// `null`/missing → 0.
pub(crate) fn lenient_i64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    Ok(Option::<i64>::deserialize(deserializer)?.unwrap_or_default())
}

/// Timestamps the way the API writes them: RFC 3339 in UTC with exactly three fractional
/// digits (`2026-10-07T05:17:55.590Z`, `2026-10-07T05:17:55.000Z`). The `time` crate's own
/// RFC 3339 formatter trims trailing zeros (`…55.59Z`, `…55Z`), so a value that went through
/// this package would no longer match the service's answers byte for byte. Any RFC 3339 time
/// is accepted on the way in.
pub(crate) mod rfc3339_ms {
    use serde::{Deserialize, Deserializer, Serializer, de};
    use time::OffsetDateTime;
    use time::format_description::well_known::Rfc3339;

    /// `2026-10-07T05:17:55.590Z`.
    pub(crate) fn format(t: OffsetDateTime) -> String {
        let t = t.to_offset(time::UtcOffset::UTC);
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
            t.year(),
            u8::from(t.month()),
            t.day(),
            t.hour(),
            t.minute(),
            t.second(),
            t.millisecond()
        )
    }

    fn parse<E: de::Error>(text: &str) -> Result<OffsetDateTime, E> {
        OffsetDateTime::parse(text.trim(), &Rfc3339)
            .map_err(|e| E::custom(format!("`{text}` is not an RFC 3339 time: {e}")))
    }

    pub(crate) fn serialize<S: Serializer>(
        t: &OffsetDateTime,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format(*t))
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<OffsetDateTime, D::Error> {
        parse(&String::deserialize(deserializer)?)
    }

    /// The same for `Option<OffsetDateTime>`: `None` is written as `null`, and `null` (or a
    /// missing field, with `#[serde(default)]`) reads as `None`.
    pub(crate) mod option {
        use serde::{Deserialize, Deserializer, Serializer};
        use time::OffsetDateTime;

        // serde's `with` hands the field by reference.
        #[allow(clippy::ref_option)]
        pub(crate) fn serialize<S: Serializer>(
            t: &Option<OffsetDateTime>,
            serializer: S,
        ) -> Result<S::Ok, S::Error> {
            match t {
                Some(t) => serializer.serialize_some(&super::format(*t)),
                None => serializer.serialize_none(),
            }
        }

        pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
            deserializer: D,
        ) -> Result<Option<OffsetDateTime>, D::Error> {
            Option::<String>::deserialize(deserializer)?
                .map(|text| super::parse(&text))
                .transpose()
        }
    }
}

/// A short human description of a JSON value's type, for error messages.
pub(crate) fn describe(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "a list",
        Value::Object(_) => "an object",
    }
}

/// If `value` is an object whose only meaningful content is `{key: {...}}`, returns the
/// inner object; otherwise returns `value` unchanged. Lets the client accept both
/// `{"job": {...}}` and a bare `{...}`.
pub(crate) fn unwrap_key(value: Value, key: &str) -> Value {
    match value {
        Value::Object(mut map) if map.get(key).is_some_and(Value::is_object) => {
            map.remove(key).unwrap_or(Value::Null)
        }
        other => other,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use time::OffsetDateTime;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Stamped {
        #[serde(with = "super::rfc3339_ms")]
        at: OffsetDateTime,
        #[serde(default, with = "super::rfc3339_ms::option")]
        maybe: Option<OffsetDateTime>,
    }

    #[test]
    fn timestamps_keep_the_apis_three_fractional_digits() {
        for (input, output) in [
            // Trailing zeros stay: the `time` crate's formatter would print `.59Z` and `Z`.
            ("2026-10-07T05:17:55.590Z", "2026-10-07T05:17:55.590Z"),
            ("2026-10-07T05:17:55.000Z", "2026-10-07T05:17:55.000Z"),
            ("2026-10-07T05:17:55Z", "2026-10-07T05:17:55.000Z"),
            // Other offsets and finer precision come out the way the API writes them.
            (
                "2026-10-07T10:47:55.123456+05:30",
                "2026-10-07T05:17:55.123Z",
            ),
        ] {
            let value: Stamped =
                serde_json::from_value(json!({ "at": input, "maybe": input })).unwrap();
            assert_eq!(
                serde_json::to_value(&value).unwrap(),
                json!({ "at": output, "maybe": output }),
                "{input}"
            );
        }
        // A null or missing optional time reads as None and is written as null.
        let value: Stamped =
            serde_json::from_value(json!({ "at": "2026-10-07T05:17:55.590Z" })).unwrap();
        assert_eq!(value.maybe, None);
        assert_eq!(
            serde_json::to_value(&value).unwrap()["maybe"],
            serde_json::Value::Null
        );
        let value: Stamped =
            serde_json::from_value(json!({ "at": "2026-10-07T05:17:55.590Z", "maybe": null }))
                .unwrap();
        assert_eq!(value.maybe, None);
        // Not a time: a precise error.
        let err = serde_json::from_value::<Stamped>(json!({ "at": "yesterday" })).unwrap_err();
        assert!(
            err.to_string()
                .contains("`yesterday` is not an RFC 3339 time"),
            "{err}"
        );
    }
}
