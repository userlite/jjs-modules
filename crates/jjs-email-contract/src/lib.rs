//! Shared validation for the guest module, trusted host boundary, and stored email.
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
pub const MAX_MESSAGE_BYTES: usize = 256 * 1024;
pub const FIELDS: &[&str] = &[
    "idempotencyKey",
    "to",
    "cc",
    "bcc",
    "replyTo",
    "from",
    "subject",
    "text",
    "html",
    "attachments",
    "headers",
    "inReplyTo",
    "references",
    "priority",
];
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmailError {
    pub code: String,
    pub field: String,
    pub message: String,
}
impl EmailError {
    pub fn new(code: &str, field: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            field: field.into(),
            message: message.into(),
        }
    }
    pub fn wire(&self) -> String {
        serde_json::to_string(self).expect("email error is JSON")
    }
}
pub fn error(code: &str, field: &str, message: impl Into<String>) -> String {
    EmailError::new(code, field, message).wire()
}
type Result<T> = std::result::Result<T, String>;
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Addresses {
    One(String),
    Many(Vec<String>),
}
impl Addresses {
    pub fn values(&self) -> Vec<String> {
        match self {
            Self::One(s) => vec![s.clone()],
            Self::Many(v) => v.clone(),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Attachment {
    pub filename: String,
    /// Canonical base64; all bytes are captured before enqueue.
    pub content: String,
    pub content_type: String,
    pub content_disposition: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cid: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Header {
    pub name: String,
    pub value: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EmailMessage {
    pub idempotency_key: String,
    pub to: Option<Addresses>,
    pub cc: Option<Addresses>,
    pub bcc: Option<Addresses>,
    pub subject: String,
    pub text: Option<String>,
    pub html: Option<String>,
    pub reply_to: Option<Addresses>,
    pub from: Option<String>,
    // Omitted new fields preserve the serialized identity of existing queued messages.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<Attachment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<Header>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
}
fn object<'a>(v: &'a Value, field: &str) -> Result<&'a Map<String, Value>> {
    v.as_object()
        .ok_or_else(|| error("EINVALIDTYPE", field, format!("{field} must be an object")))
}
fn string<'a>(v: &'a Value, field: &str) -> Result<&'a str> {
    v.as_str()
        .ok_or_else(|| error("EINVALIDTYPE", field, format!("{field} must be a string")))
}
fn required<'a>(m: &'a Map<String, Value>, key: &str, field: &str) -> Result<&'a Value> {
    m.get(key)
        .ok_or_else(|| error("EREQUIRED", field, format!("{field} is required")))
}
fn optional(m: &Map<String, Value>, key: &str, prefix: &str) -> Result<Option<String>> {
    m.get(key)
        .map(|v| string(v, &format!("{prefix}{key}")).map(str::to_owned))
        .transpose()
}
fn fields(m: &Map<String, Value>, allowed: &[&str], prefix: &str) -> Result<()> {
    for k in m.keys() {
        if !allowed.contains(&k.as_str()) {
            let field = format!("{prefix}{k}");
            let hint = match k.as_str() {
                "path" | "href" => "Supply attachment content as a string or Buffer; file and URL loading are not supported.".into(),
                "raw" | "icalEvent" | "alternatives" | "amp" | "watchHtml" => "Use text, html, or supported attachments; custom MIME content is not supported.".into(),
                "sender" | "envelope" | "dkim" => "Envhost controls the sender and SES configuration; use replyTo for replies.".into(),
                "messageId" | "date" => "SES controls Message-ID and Date; use idempotencyKey for duplicate prevention.".into(),
                _ => format!("Supported fields: {}.", allowed.join(", ")),
            };
            return Err(error(
                "EUNSUPPORTEDFIELD",
                &field,
                format!("Unsupported field {field}. {hint}"),
            ));
        }
    }
    Ok(())
}
pub fn address_valid(s: &str) -> bool {
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && local.len() <= 64
        && !local.starts_with('.')
        && !local.ends_with('.')
        && !local.contains("..")
        && local
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".!#$%&'*+-/=?^_`{|}~".contains(&b))
        && domain_valid(domain)
        && s.len() <= 254
}
pub fn domain_valid(s: &str) -> bool {
    s.len() <= 253
        && s.contains('.')
        && s.split('.').all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && !l.starts_with('-')
                && !l.ends_with('-')
                && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}
fn bad_address(field: &str) -> String {
    error("EADDRESS", field, format!("{field} requires valid ASCII email addresses; use name <address>, {{name, address}}, or an array; address groups and comments are not supported"))
}
fn parts(s: &str, field: &str) -> Result<(String, String)> {
    if s.chars().any(char::is_control) {
        return Err(bad_address(field));
    }
    let s = s.trim();
    let (name, addr) = if let Some((name, tail)) = s.split_once('<') {
        if !tail.ends_with('>') || tail[..tail.len() - 1].contains(['<', '>']) {
            return Err(bad_address(field));
        }
        let name = name.trim();
        let name = if name.starts_with('"') {
            if name.len() < 2 || !name.ends_with('"') {
                return Err(bad_address(field));
            }
            let mut out = String::new();
            let mut escape = false;
            for c in name[1..name.len() - 1].chars() {
                if escape {
                    out.push(c);
                    escape = false;
                } else if c == '\\' {
                    escape = true;
                } else if c == '"' {
                    return Err(bad_address(field));
                } else {
                    out.push(c);
                }
            }
            if escape {
                return Err(bad_address(field));
            }
            out
        } else {
            if name.contains(['"', '\\', '(', ')', ':', ';', '>', ',']) {
                return Err(bad_address(field));
            }
            name.to_owned()
        };
        (name, tail[..tail.len() - 1].trim().to_owned())
    } else {
        (String::new(), s.to_owned())
    };
    if !address_valid(&addr) || name.len() > 1024 {
        return Err(bad_address(field));
    }
    Ok((name, addr))
}
pub fn mailbox(s: &str) -> Result<String> {
    parts(s, "from").map(|(_, a)| a)
}
fn render(name: &str, addr: &str, field: &str) -> Result<String> {
    if !address_valid(addr) || name.chars().any(char::is_control) || name.len() > 256 {
        return Err(bad_address(field));
    }
    if name.is_empty() {
        return Ok(addr.into());
    }
    if name.is_ascii() {
        return Ok(format!(
            "\"{}\" <{addr}>",
            name.replace('\\', "\\\\").replace('"', "\\\"")
        ));
    }
    let mut words = Vec::new();
    let mut chunk = String::new();
    for c in name.chars() {
        if chunk.len() + c.len_utf8() > 30 {
            words.push(format!("=?UTF-8?B?{}?=", STANDARD.encode(chunk.as_bytes())));
            chunk.clear();
        }
        chunk.push(c);
    }
    if !chunk.is_empty() {
        words.push(format!("=?UTF-8?B?{}?=", STANDARD.encode(chunk.as_bytes())));
    }
    Ok(format!("{} <{addr}>", words.join(" ")))
}
fn address_string(s: &str, field: &str) -> Result<Vec<String>> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    let mut escaped = false;
    let mut angle = false;
    for (i, ch) in s.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quoted && ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            quoted = !quoted;
        }
        if !quoted {
            match ch {
                '<' if !angle => angle = true,
                '>' if angle => angle = false,
                '<' | '>' => return Err(bad_address(field)),
                ',' if !angle => {
                    let (n, a) = parts(&s[start..i], field)?;
                    result.push(render(&n, &a, field)?);
                    start = i + 1;
                }
                _ => (),
            }
        }
    }
    if quoted || escaped || angle {
        return Err(bad_address(field));
    }
    let (n, a) = parts(&s[start..], field)?;
    result.push(render(&n, &a, field)?);
    Ok(result)
}
fn addresses(v: &Value, field: &str) -> Result<Addresses> {
    fn item(v: &Value, field: &str) -> Result<Vec<String>> {
        if let Some(s) = v.as_str() {
            return address_string(s, field);
        }
        let m = object(v, field)?;
        fields(m, &["name", "address"], &format!("{field}."))?;
        let addr = string(
            required(m, "address", &format!("{field}.address"))?,
            &format!("{field}.address"),
        )?;
        let name = optional(m, "name", &format!("{field}."))?.unwrap_or_default();
        Ok(vec![render(&name, addr, field)?])
    }
    if let Some(a) = v.as_array() {
        let mut all = Vec::new();
        for (i, x) in a.iter().enumerate() {
            all.extend(item(x, &format!("{field}[{i}]"))?);
        }
        if all.is_empty() {
            return Err(bad_address(field));
        }
        Ok(Addresses::Many(all))
    } else {
        let mut all = item(v, field)?;
        if all.len() == 1 {
            Ok(Addresses::One(all.remove(0)))
        } else {
            Ok(Addresses::Many(all))
        }
    }
}
fn attachment(v: &Value, i: usize) -> Result<Attachment> {
    let f = format!("attachments[{i}]");
    let p = format!("{f}.");
    let m = object(v, &f)?;
    fields(
        m,
        &[
            "filename",
            "content",
            "encoding",
            "contentType",
            "contentDisposition",
            "cid",
        ],
        &p,
    )?;
    let filename = string(
        required(m, "filename", &(p.clone() + "filename"))?,
        &(p.clone() + "filename"),
    )?
    .to_owned();
    let content = required(m, "content", &(p.clone() + "content"))?;
    let encoding = optional(m, "encoding", &p)?;
    let bytes = if let Some(s) = content.as_str() {
        match encoding.as_deref().unwrap_or("utf8") {
            "utf8" | "utf-8" => s.as_bytes().to_vec(),
            "base64" => STANDARD.decode(s).map_err(|_| {
                error(
                    "EENCODING",
                    &(p.clone() + "content"),
                    "Attachment content must be valid padded base64",
                )
            })?,
            "hex" => {
                if s.len() % 2 != 0 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(error(
                        "EENCODING",
                        &(p.clone() + "content"),
                        "Hex attachment content requires pairs of hexadecimal digits",
                    ));
                }
                (0..s.len())
                    .step_by(2)
                    .map(|j| u8::from_str_radix(&s[j..j + 2], 16).expect("validated hex"))
                    .collect()
            }
            _ => {
                return Err(error(
                    "EUNSUPPORTEDENCODING",
                    &(p.clone() + "encoding"),
                    "Supported attachment encodings: utf8, utf-8, base64, hex",
                ))
            }
        }
    } else {
        if encoding.is_some() {
            return Err(error(
                "EINVALIDVALUE",
                &(p.clone() + "encoding"),
                "encoding is only valid with string content; omit it for Buffer content",
            ));
        }
        let b = object(content, &(p.clone() + "content"))?;
        fields(b, &["type", "data"], &(p.clone() + "content."))?;
        if b.get("type").and_then(Value::as_str) != Some("Buffer") {
            return Err(error(
                "EINVALIDTYPE",
                &(p.clone() + "content"),
                "Attachment content must be a string or Buffer; streams are not supported",
            ));
        }
        let a = required(b, "data", &(p.clone() + "content.data"))?
            .as_array()
            .ok_or_else(|| {
                error(
                    "EINVALIDTYPE",
                    &(p.clone() + "content.data"),
                    "Buffer data must be an array of bytes",
                )
            })?;
        a.iter()
            .enumerate()
            .map(|(j, v)| {
                v.as_u64()
                    .filter(|n| *n <= 255)
                    .map(|n| n as u8)
                    .ok_or_else(|| {
                        error(
                            "EINVALIDVALUE",
                            &format!("{p}content.data[{j}]"),
                            "Buffer bytes must be integers from 0 to 255",
                        )
                    })
            })
            .collect::<Result<Vec<_>>>()?
    };
    let content_type = optional(m, "contentType", &p)?.unwrap_or_else(|| {
        match filename
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "pdf" => "application/pdf",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "txt" => "text/plain",
            "csv" => "text/csv",
            "json" => "application/json",
            "zip" => "application/zip",
            _ => "application/octet-stream",
        }
        .into()
    });
    let cid = optional(m, "cid", &p)?;
    let content_disposition = optional(m, "contentDisposition", &p)?.unwrap_or_else(|| {
        if cid.is_some() {
            "inline"
        } else {
            "attachment"
        }
        .into()
    });
    let a = Attachment {
        filename,
        content: STANDARD.encode(bytes),
        content_type,
        content_disposition,
        cid,
    };
    validate_attachment(&a, i)?;
    Ok(a)
}
fn validate_attachment(a: &Attachment, i: usize) -> Result<()> {
    let f = format!("attachments[{i}]");
    if a.filename.is_empty()
        || a.filename.chars().count() > 255
        || a.filename.chars().any(char::is_control)
        || a.filename.contains(['/', '\\'])
    {
        return Err(error("EINVALIDVALUE",&(f.clone()+".filename"),"filename must contain 1–255 characters without directory separators or control characters"));
    }
    if !matches!(a.content_disposition.as_str(), "inline" | "attachment") {
        return Err(error(
            "EINVALIDVALUE",
            &(f.clone() + ".contentDisposition"),
            "contentDisposition must be inline or attachment",
        ));
    }
    let mime = &a.content_type;
    if mime.len() > 78
        || mime.split('/').count() != 2
        || mime.split('/').any(|s| {
            s.is_empty()
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"!#$&^_.+-".contains(&b))
        })
    {
        return Err(error(
            "EINVALIDVALUE",
            &(f.clone() + ".contentType"),
            "contentType must be a MIME type such as application/pdf, at most 78 ASCII characters",
        ));
    }
    if a.cid.as_ref().is_some_and(|s| {
        s.is_empty()
            || s.len() > 78
            || !s
                .bytes()
                .all(|b| b.is_ascii_graphic() && !b"<>\"\\".contains(&b))
    }) {
        return Err(error("EINVALIDVALUE",&(f.clone()+".cid"),"cid must contain 1–78 printable ASCII characters without spaces, brackets, quotes, or backslashes"));
    }
    STANDARD.decode(&a.content).map_err(|_| {
        error(
            "EENCODING",
            &(f + ".content"),
            "Stored attachment content must be valid base64",
        )
    })?;
    Ok(())
}
fn header(name: &str, value: &str, field: &str) -> Result<Header> {
    let lower = name.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "from"
            | "to"
            | "cc"
            | "bcc"
            | "sender"
            | "reply-to"
            | "return-path"
            | "subject"
            | "message-id"
            | "date"
            | "mime-version"
            | "received"
            | "dkim-signature"
            | "authentication-results"
            | "in-reply-to"
            | "references"
            | "x-priority"
            | "x-msmail-priority"
            | "importance"
    ) || lower.starts_with("content-")
        || lower.starts_with("x-ses-")
        || lower.starts_with("resent-")
        || lower.starts_with("arc-")
    {
        return Err(error("EPROTECTEDHEADER",field,format!("Header {name} is controlled by Envhost, SES, or a dedicated message field; use that field instead")));
    }
    check_header(name, value, field)?;
    Ok(Header {
        name: name.into(),
        value: value.into(),
    })
}
fn check_header(name: &str, value: &str, field: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 126
        || !name.bytes().all(|b| b.is_ascii_graphic() && b != b':')
    {
        return Err(error(
            "EHEADER",
            field,
            "Header names require 1–126 printable ASCII characters excluding colon",
        ));
    }
    if value.is_empty()
        || value.len() > 995
        || name.len() + value.len() > 996
        || !value.bytes().all(|b| (32..=126).contains(&b))
    {
        return Err(error("EHEADER",field,"Header values require printable ASCII without newlines; value limit 995 bytes and name plus value limit 996 bytes"));
    }
    Ok(())
}
fn message_id(s: &str, field: &str) -> Result<()> {
    if s.len() > 512
        || !s.starts_with('<')
        || !s.ends_with('>')
        || !s[1..s.len() - 1].contains('@')
        || !s[1..s.len() - 1]
            .bytes()
            .all(|b| b.is_ascii_graphic() && !b"<>".contains(&b))
    {
        return Err(error(
            "EINVALIDVALUE",
            field,
            "Message IDs must use <id@domain> format without whitespace, at most 512 bytes",
        ));
    }
    Ok(())
}
pub fn decode_message(encoded: &str) -> Result<EmailMessage> {
    if encoded.len() > MAX_MESSAGE_BYTES {
        return Err(error("EMESSAGESIZE","message",format!("Serialized message exceeds {MAX_MESSAGE_BYTES} bytes, including attachment content")));
    }
    let v: Value = serde_json::from_str(encoded)
        .map_err(|_| error("EINVALIDTYPE", "message", "Message must be valid JSON"))?;
    let m = object(&v, "message")?;
    fields(m, FIELDS, "")?;
    let addr = |k: &str| m.get(k).map(|v| addresses(v, k)).transpose();
    let from = addr("from")?
        .map(|a| {
            let v = a.values();
            if v.len() != 1 {
                Err(error(
                    "EADDRESS",
                    "from",
                    "from accepts exactly one session sender",
                ))
            } else {
                Ok(v[0].clone())
            }
        })
        .transpose()?;
    let mut headers = Vec::new();
    if let Some(h) = m.get("headers") {
        if let Some(a) = h.as_array() {
            for (i, v) in a.iter().enumerate() {
                let f = format!("headers[{i}]");
                let h = object(v, &f)?;
                fields(h, &["key", "value"], &(f.clone() + "."))?;
                let name = string(
                    required(h, "key", &(f.clone() + ".key"))?,
                    &(f.clone() + ".key"),
                )?;
                let val = string(
                    required(h, "value", &(f.clone() + ".value"))?,
                    &(f.clone() + ".value"),
                )?;
                headers.push(header(name, val, &f)?);
            }
        } else {
            for (k, v) in object(h, "headers")? {
                let f = format!("headers.{k}");
                headers.push(header(k, string(v, &f)?, &f)?);
            }
        }
    }
    let attachments = match m.get("attachments") {
        None => vec![],
        Some(v) => v
            .as_array()
            .ok_or_else(|| {
                error(
                    "EINVALIDTYPE",
                    "attachments",
                    "attachments must be an array",
                )
            })?
            .iter()
            .enumerate()
            .map(|(i, v)| attachment(v, i))
            .collect::<Result<_>>()?,
    };
    let references = match m.get("references") {
        None => vec![],
        Some(Value::String(s)) => {
            if s.trim().is_empty() {
                return Err(error(
                    "EINVALIDVALUE",
                    "references",
                    "references must contain message IDs",
                ));
            }
            s.split_whitespace().map(str::to_owned).collect()
        }
        Some(Value::Array(a)) => a
            .iter()
            .enumerate()
            .map(|(i, v)| string(v, &format!("references[{i}]")).map(str::to_owned))
            .collect::<Result<_>>()?,
        Some(_) => {
            return Err(error(
                "EINVALIDTYPE",
                "references",
                "references must be a space-separated string or array of message IDs",
            ))
        }
    };
    let message = EmailMessage {
        idempotency_key: string(
            required(m, "idempotencyKey", "idempotencyKey")?,
            "idempotencyKey",
        )?
        .into(),
        subject: string(required(m, "subject", "subject")?, "subject")?.into(),
        to: addr("to")?,
        cc: addr("cc")?,
        bcc: addr("bcc")?,
        reply_to: addr("replyTo")?,
        from,
        text: optional(m, "text", "")?,
        html: optional(m, "html", "")?,
        attachments,
        headers,
        in_reply_to: optional(m, "inReplyTo", "")?,
        references,
        priority: optional(m, "priority", "")?,
    };
    message.validate()?;
    Ok(message)
}
impl EmailMessage {
    pub fn extra_headers(&self) -> Vec<Header> {
        let mut h = self.headers.clone();
        if let Some(s) = &self.in_reply_to {
            h.push(Header {
                name: "In-Reply-To".into(),
                value: s.clone(),
            });
        }
        if !self.references.is_empty() {
            h.push(Header {
                name: "References".into(),
                value: self.references.join(" "),
            });
        }
        if let Some(p) = &self.priority {
            if p != "normal" {
                for (n, v) in [
                    ("X-Priority", if p == "high" { "1" } else { "5" }),
                    (
                        "X-MSMail-Priority",
                        if p == "high" { "High" } else { "Low" },
                    ),
                    ("Importance", p.as_str()),
                ] {
                    h.push(Header {
                        name: n.into(),
                        value: v.into(),
                    });
                }
            }
        }
        h
    }
    pub fn validate(&self) -> Result<()> {
        if self.idempotency_key.is_empty()
            || self.idempotency_key.len() > 128
            || !self.idempotency_key.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err(error("EINVALIDVALUE","idempotencyKey","idempotencyKey must contain 1–128 non-space printable ASCII bytes; reuse it only for the same intended email"));
        }
        let mut count = 0;
        for (field, list) in [
            ("to", &self.to),
            ("cc", &self.cc),
            ("bcc", &self.bcc),
            ("replyTo", &self.reply_to),
        ] {
            if let Some(a) = list {
                let values = a.values();
                if values.is_empty() || values.len() > 50 {
                    return Err(error(
                        "ERECIPIENTLIMIT",
                        field,
                        "Address lists must contain 1–50 addresses",
                    ));
                }
                for (i, s) in values.iter().enumerate() {
                    parts(s, &format!("{field}[{i}]"))?;
                }
                if field != "replyTo" {
                    count += values.len();
                }
            }
        }
        if count == 0 || count > 50 {
            return Err(error(
                "ERECIPIENTLIMIT",
                "to,cc,bcc",
                "Message requires 1–50 total recipients across to, cc, and bcc",
            ));
        }
        if let Some(s) = &self.from {
            parts(s, "from")?;
        }
        if self.subject.len() > 512 || self.subject.contains(['\r', '\n']) {
            return Err(error(
                "ESUBJECT",
                "subject",
                "subject must be at most 512 UTF-8 bytes without newlines",
            ));
        }
        if ![&self.text, &self.html]
            .iter()
            .any(|v| v.as_ref().is_some_and(|s| !s.is_empty()))
        {
            return Err(error(
                "EREQUIRED",
                "text,html",
                "A nonempty text or html body is required",
            ));
        }
        for (i, a) in self.attachments.iter().enumerate() {
            validate_attachment(a, i)?;
        }
        let mut cids = std::collections::HashSet::new();
        for (i, a) in self.attachments.iter().enumerate() {
            if let Some(cid) = &a.cid {
                if !cids.insert(cid) {
                    return Err(error(
                        "EINVALIDVALUE",
                        &format!("attachments[{i}].cid"),
                        "Attachment cid values must be unique within the message",
                    ));
                }
            }
        }
        for (i, h) in self.headers.iter().enumerate() {
            header(&h.name, &h.value, &format!("headers[{i}]"))?;
        }
        if let Some(s) = &self.in_reply_to {
            message_id(s, "inReplyTo")?;
        }
        for (i, s) in self.references.iter().enumerate() {
            message_id(s, &format!("references[{i}]"))?;
        }
        if self
            .priority
            .as_ref()
            .is_some_and(|s| !matches!(s.as_str(), "high" | "normal" | "low"))
        {
            return Err(error(
                "EINVALIDVALUE",
                "priority",
                "priority must be high, normal, or low",
            ));
        }
        let h = self.extra_headers();
        if h.len() > 15 {
            return Err(error("EHEADERLIMIT","headers","SES allows at most 15 headers, including inReplyTo, references, and the three priority headers"));
        }
        for h in h {
            check_header(&h.name, &h.value, &h.name)?;
        }
        if serde_json::to_vec(self)
            .map_err(|_| error("EINVALIDTYPE", "message", "Message could not be serialized"))?
            .len()
            > MAX_MESSAGE_BYTES
        {
            return Err(error("EMESSAGESIZE","message",format!("Serialized message exceeds {MAX_MESSAGE_BYTES} bytes, including base64 attachments")));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn basic() -> Value {
        json!({"idempotencyKey":"message-1","to":"a@example.com","subject":"hello","text":"hello"})
    }
    fn failure(value: Value, code: &str, field: &str) {
        let e: EmailError =
            serde_json::from_str(&decode_message(&value.to_string()).unwrap_err()).unwrap();
        assert_eq!(e.code, code, "{}", e.message);
        assert_eq!(e.field, field, "{}", e.message);
    }
    #[test]
    fn formats_addresses_without_splitting_quoted_commas_and_encodes_unicode() {
        let mut v = basic();
        v["to"] = json!("\"Smith, David\" <david@example.com>, other@example.com");
        v["replyTo"] = json!({"name":"Réponses","address":"reply@example.com"});
        v["from"] = json!({"name":"Shop","address":"noreply.session@example.com"});
        let m = decode_message(&v.to_string()).unwrap();
        assert_eq!(
            m.to.unwrap().values(),
            ["\"Smith, David\" <david@example.com>", "other@example.com"]
        );
        assert!(m.reply_to.unwrap().values()[0].starts_with("=?UTF-8?B?"));
        assert_eq!(
            mailbox(m.from.as_ref().unwrap()).unwrap(),
            "noreply.session@example.com"
        );
        for value in [
            "a@example.com,",
            "a@example.com\r\nBcc: b@example.com",
            "\"broken <a@example.com>",
            "a@example.com>",
        ] {
            let mut v = basic();
            v["to"] = json!(value);
            failure(v, "EADDRESS", "to");
        }
    }
    #[test]
    fn captures_text_base64_hex_and_buffer_attachments_with_inline_metadata() {
        let mut v = basic();
        v["attachments"] = json!([
            {"filename":"note.txt","content":"hello"},
            {"filename":"photo.png","content":"aGk=","encoding":"base64","cid":"photo@session"},
            {"filename":"bytes.bin","content":"00ff","encoding":"hex","contentType":"application/octet-stream","contentDisposition":"attachment"},
            {"filename":"buffer.bin","content":{"type":"Buffer","data":[0,255]}}
        ]);
        let m = decode_message(&v.to_string()).unwrap();
        assert_eq!(m.attachments[0].content, "aGVsbG8=");
        assert_eq!(m.attachments[0].content_type, "text/plain");
        assert_eq!(m.attachments[1].content_disposition, "inline");
        assert_eq!(m.attachments[2].content, "AP8=");
        assert_eq!(m.attachments[3].content, "AP8=");
        let mut v = basic();
        v["attachments"] = json!([{"filename":"test.pdf","path":"/secret"}]);
        failure(v, "EUNSUPPORTEDFIELD", "attachments[0].path");
        let mut v = basic();
        v["attachments"] =
            json!([{"filename":"test.pdf","content":"not base64!","encoding":"base64"}]);
        failure(v, "EENCODING", "attachments[0].content");
    }
    #[test]
    fn validates_headers_threading_priority_and_reserved_headers() {
        let mut v = basic();
        v["headers"] = json!([{ "key":"X-App","value":"one"},{"key":"X-App","value":"two"}]);
        v["inReplyTo"] = json!("<original@example.com>");
        v["references"] = json!("<first@example.com> <original@example.com>");
        v["priority"] = json!("high");
        let m = decode_message(&v.to_string()).unwrap();
        let h = m.extra_headers();
        assert_eq!(h.len(), 7);
        assert_eq!(h[2].name, "In-Reply-To");
        assert_eq!(h[6].value, "high");
        for name in [
            "From",
            "bCc",
            "X-SES-CONFIGURATION-SET",
            "Content-Type",
            "Return-Path",
            "Subject",
        ] {
            let mut v = basic();
            v["headers"] = json!({name:"spoof"});
            failure(v, "EPROTECTEDHEADER", &format!("headers.{name}"));
        }
        let mut v = basic();
        v["headers"] = json!({"X-App":"one\r\nFrom: attacker@example.com"});
        failure(v, "EHEADER", "headers.X-App");
        let mut v = basic();
        v["references"] = json!(["not-an-id"]);
        failure(v, "EINVALIDVALUE", "references[0]");
        let mut v = basic();
        v["priority"] = json!("urgent");
        failure(v, "EINVALIDVALUE", "priority");
        let mut v = basic();
        v["headers"] = json!((0..13)
            .map(|i| json!({"key":format!("X-{i}"),"value":"value"}))
            .collect::<Vec<_>>());
        v["priority"] = json!("high");
        failure(v, "EHEADERLIMIT", "headers");
    }
    #[test]
    fn errors_name_missing_unknown_and_wrongly_typed_fields() {
        let mut v = basic();
        v.as_object_mut().unwrap().remove("idempotencyKey");
        failure(v, "EREQUIRED", "idempotencyKey");
        let mut v = basic();
        v["html"] = json!(42);
        failure(v, "EINVALIDTYPE", "html");
        let mut v = basic();
        v["attachements"] = json!([]);
        failure(v, "EUNSUPPORTEDFIELD", "attachements");
        let mut v = basic();
        v["to"] = json!({"name":"David","address":false});
        failure(v, "EINVALIDTYPE", "to.address");
        let mut v = basic();
        v["attachments"] =
            json!([{"filename":"a.txt","content":"a","contentDisposition":"inlien"}]);
        failure(v, "EINVALIDVALUE", "attachments[0].contentDisposition");
        let mut v = basic();
        v["html"] = json!("x".repeat(MAX_MESSAGE_BYTES));
        failure(v, "EMESSAGESIZE", "message");
    }
    #[test]
    fn legacy_messages_keep_their_serialized_identity() {
        let v = json!({"idempotencyKey":"message-1","to":"a@example.com","cc":null,"bcc":null,"subject":"hello","text":"hello","html":null,"replyTo":null,"from":null});
        let m: EmailMessage = serde_json::from_value(v.clone()).unwrap();
        m.validate().unwrap();
        assert_eq!(serde_json::to_value(m).unwrap(), v);
    }
}
