//! Bug report bodies and their optional screenshot.
//!
//! The report mail goes out as plain text through the Accounts private mail
//! bridge (or an `APPS_MAIL_URL` adapter that receives JSON), and neither
//! carries attachments. A screenshot is therefore kept in this database and
//! the mail carries a link with a random key that only the recipients see.
//! Only the key's SHA-256 is stored, and the image's base64 never reaches the
//! catalog, the outbox or an error message.
use crate::error::{ApiError, Result};
use base64::Engine;
use serde_json::Value;

/// Largest decoded screenshot accepted, in bytes.
pub const MAX_SCREENSHOT_BYTES: usize = 5 * 1024 * 1024;
/// Largest screenshot file name kept, in characters, after trimming.
pub const MAX_NAME_CHARS: usize = 120;
/// Largest report request body: a 5 MiB image as base64 plus the message.
pub const MAX_REQUEST_BYTES: usize = 8 * 1024 * 1024;
pub const CONTENT_TYPES: [&str; 3] = ["image/png", "image/jpeg", "image/webp"];
const FIELDS: [&str; 3] = ["message", "pr", "screenshot"];
const SCREENSHOT_FIELDS: [&str; 3] = ["name", "content_type", "data"];

pub struct Screenshot {
    pub name: String,
    pub content_type: &'static str,
    pub bytes: Vec<u8>,
    /// The secret half of the maintainers' link. Stored only as a hash.
    pub key: String,
    /// `APPS_PUBLIC_URL`, the base of the maintainers' link.
    pub base_url: String,
}

impl Screenshot {
    pub fn url(&self, report_id: &str) -> String {
        format!(
            "{}/v1/reports/{report_id}/screenshot?key={}",
            self.base_url, self.key
        )
    }
}

/// Refuses fields a report does not have, then validates, decodes and removes
/// `screenshot` from `body` so the base64 goes no further than this function.
pub fn take_screenshot(body: &mut Value, base_url: &str) -> Result<Option<Screenshot>> {
    let object = body
        .as_object_mut()
        .ok_or_else(|| ApiError::bad("A report must be a JSON object."))?;
    if let Some(field) = object.keys().find(|k| !FIELDS.contains(&k.as_str())) {
        return Err(ApiError::bad(format!(
            "Unknown field `{field}`. A report accepts message, pr and screenshot."
        )));
    }
    let shot = match object.remove("screenshot") {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::Object(shot)) => shot,
        Some(_) => {
            return Err(ApiError::bad(
                "`screenshot` must be an object with name, content_type and data.",
            ));
        }
    };
    if let Some(field) = shot
        .keys()
        .find(|k| !SCREENSHOT_FIELDS.contains(&k.as_str()))
    {
        return Err(ApiError::bad(format!(
            "Unknown field `screenshot.{field}`. A screenshot accepts name, content_type and data."
        )));
    }
    let text = |k: &str| {
        shot.get(k)
            .and_then(Value::as_str)
            .ok_or_else(|| ApiError::bad(format!("`screenshot.{k}` must be a string.")))
    };
    let name: String = text("name")?
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME_CHARS)
        .collect();
    let name = match name.trim() {
        "" => "screenshot".to_owned(),
        n => n.to_owned(),
    };
    let requested = text("content_type")?;
    let content_type = CONTENT_TYPES
        .into_iter()
        .find(|t| t.eq_ignore_ascii_case(requested.trim()))
        .ok_or_else(|| {
            ApiError::bad("`screenshot.content_type` must be image/png, image/jpeg or image/webp.")
        })?;
    let data = text("data")?;
    // Refuse an oversize image before decoding it.
    if data.len() > MAX_SCREENSHOT_BYTES.div_ceil(3) * 4 {
        return Err(too_large());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|_| ApiError::bad("`screenshot.data` must be standard base64."))?;
    if bytes.len() > MAX_SCREENSHOT_BYTES {
        return Err(too_large());
    }
    if bytes.is_empty() {
        return Err(ApiError::bad("`screenshot.data` is empty."));
    }
    if !matches_type(&bytes, content_type) {
        return Err(ApiError::bad(format!(
            "`screenshot.data` is not a {content_type} image."
        )));
    }
    Ok(Some(Screenshot {
        name,
        content_type,
        bytes,
        key: hex::encode(rand::random::<[u8; 32]>()),
        base_url: base_url.trim_end_matches('/').to_owned(),
    }))
}

fn too_large() -> ApiError {
    ApiError::bad("`screenshot` must be 5 MiB or smaller once decoded.")
}

/// The image is served with its declared type, so its bytes must agree.
fn matches_type(bytes: &[u8], content_type: &str) -> bool {
    match content_type {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/webp" => bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP",
        _ => false,
    }
}

/// `message` followed by the screenshot link, cut so the whole stays within
/// the 20,000 bytes the mail bridge accepts.
pub fn message_with_link(message: &str, url: &str) -> String {
    let suffix = format!("\n\nScreenshot: {url}");
    let room = 20000usize.saturating_sub(suffix.len());
    if message.len() <= room {
        return format!("{message}{suffix}");
    }
    let mut end = room.saturating_sub('…'.len_utf8());
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…{suffix}", &message[..end])
}

#[cfg(test)]
mod tests {
    use super::message_with_link;

    #[test]
    fn the_link_always_fits_the_mail_bridge_limit() {
        let url = "https://apps.example/v1/reports/r/screenshot?key=k";
        let short = message_with_link("Hi", url);
        assert_eq!(short, format!("Hi\n\nScreenshot: {url}"));
        let long = message_with_link(&"é".repeat(10000), url);
        assert!(long.len() <= 20000, "{}", long.len());
        assert!(long.ends_with(&format!("…\n\nScreenshot: {url}")));
    }
}
