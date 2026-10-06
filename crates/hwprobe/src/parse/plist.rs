//! XML property lists, as printed by `ioreg -a` and `system_profiler -xml`.
//!
//! A small, dependency-free reader for the subset Apple's tools emit. Input is
//! untrusted: malformed XML returns an error, and nesting is bounded so a
//! hostile document cannot exhaust the stack.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Dict(Vec<(String, Value)>),
    Array(Vec<Value>),
    String(String),
    Integer(i128),
    Real(f64),
    Bool(bool),
    Data(Vec<u8>),
    Date(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlistError(pub String);

impl fmt::Display for PlistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "property list: {}", self.0)
    }
}

impl std::error::Error for PlistError {}

const MAX_DEPTH: usize = 128;

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Dict(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i128> {
        match self {
            Value::Integer(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_data(&self) -> Option<&[u8]> {
        match self {
            Value::Data(d) => Some(d),
            _ => None,
        }
    }

    pub fn as_array(&self) -> &[Value] {
        match self {
            Value::Array(a) => a,
            _ => &[],
        }
    }

    /// A string value, or a NUL-terminated byte string stored as data (how
    /// IOKit publishes many identity properties).
    pub fn text(&self) -> Option<String> {
        let s = match self {
            Value::String(s) => s.clone(),
            Value::Data(d) => {
                let end = d.iter().position(|&b| b == 0).unwrap_or(d.len());
                String::from_utf8_lossy(&d[..end]).into_owned()
            }
            Value::Integer(i) => i.to_string(),
            _ => return None,
        };
        let s = s.trim().to_string();
        (!s.is_empty()).then_some(s)
    }

    /// Every dictionary in the tree, depth first, including `self`.
    pub fn dicts(&self) -> Vec<&Value> {
        let mut out = Vec::new();
        let mut stack = vec![self];
        while let Some(v) = stack.pop() {
            match v {
                Value::Dict(entries) => {
                    out.push(v);
                    stack.extend(entries.iter().rev().map(|(_, c)| c));
                }
                Value::Array(items) => stack.extend(items.iter().rev()),
                _ => {}
            }
        }
        out
    }
}

pub fn parse(xml: &str) -> Result<Value, PlistError> {
    let mut p = Parser { s: xml, at: 0 };
    p.skip_prolog()?;
    let open = p.tag()?;
    let value = if open.name == "plist" {
        if open.self_closing {
            return Err(PlistError("empty plist".into()));
        }
        let v = p.value(0)?;
        p.expect_close("plist")?;
        v
    } else {
        // Some tools omit the <plist> wrapper.
        p.value_from(open, 0)?
    };
    Ok(value)
}

struct Tag<'a> {
    name: &'a str,
    closing: bool,
    self_closing: bool,
}

struct Parser<'a> {
    s: &'a str,
    at: usize,
}

impl<'a> Parser<'a> {
    fn err<T>(&self, msg: &str) -> Result<T, PlistError> {
        Err(PlistError(format!("{msg} at byte {}", self.at)))
    }

    fn rest(&self) -> &'a str {
        &self.s[self.at..]
    }

    fn skip_ws(&mut self) {
        let trimmed = self.rest().trim_start();
        self.at = self.s.len() - trimmed.len();
    }

    /// Skip whitespace, comments, the XML declaration and the doctype.
    fn skip_prolog(&mut self) -> Result<(), PlistError> {
        loop {
            self.skip_ws();
            let r = self.rest();
            let end = if r.starts_with("<?") {
                r.find("?>").map(|i| i + 2)
            } else if r.starts_with("<!--") {
                r.find("-->").map(|i| i + 3)
            } else if r.starts_with("<!") {
                r.find('>').map(|i| i + 1)
            } else {
                return Ok(());
            };
            match end {
                Some(e) => self.at += e,
                None => return self.err("unterminated declaration"),
            }
        }
    }

    fn tag(&mut self) -> Result<Tag<'a>, PlistError> {
        self.skip_prolog()?;
        let r = self.rest();
        if !r.starts_with('<') {
            return self.err("expected a tag");
        }
        let Some(end) = r.find('>') else {
            return self.err("unterminated tag");
        };
        let inner = &r[1..end];
        self.at += end + 1;
        let closing = inner.starts_with('/');
        let self_closing = inner.ends_with('/');
        let body = inner.trim_start_matches('/').trim_end_matches('/');
        let name = body.split_whitespace().next().unwrap_or("");
        Ok(Tag {
            name,
            closing,
            self_closing,
        })
    }

    fn expect_close(&mut self, name: &str) -> Result<(), PlistError> {
        let t = self.tag()?;
        if t.closing && t.name == name {
            Ok(())
        } else {
            self.err(&format!("expected </{name}>"))
        }
    }

    /// Raw text up to the next `<`, then the matching close tag.
    fn text(&mut self, name: &str) -> Result<String, PlistError> {
        let r = self.rest();
        let Some(end) = r.find('<') else {
            return self.err("unterminated text");
        };
        let raw = &r[..end];
        self.at += end;
        self.expect_close(name)?;
        Ok(unescape(raw))
    }

    fn value(&mut self, depth: usize) -> Result<Value, PlistError> {
        let t = self.tag()?;
        self.value_from(t, depth)
    }

    fn value_from(&mut self, t: Tag<'a>, depth: usize) -> Result<Value, PlistError> {
        if depth > MAX_DEPTH {
            return self.err("nesting too deep");
        }
        if t.closing {
            return self.err("unexpected close tag");
        }
        if t.self_closing {
            return match t.name {
                "true" => Ok(Value::Bool(true)),
                "false" => Ok(Value::Bool(false)),
                "dict" => Ok(Value::Dict(Vec::new())),
                "array" => Ok(Value::Array(Vec::new())),
                "string" => Ok(Value::String(String::new())),
                "data" => Ok(Value::Data(Vec::new())),
                _ => self.err("unknown empty element"),
            };
        }
        match t.name {
            "dict" => {
                let mut entries = Vec::new();
                loop {
                    let k = self.tag()?;
                    if k.closing && k.name == "dict" {
                        return Ok(Value::Dict(entries));
                    }
                    if k.name != "key" || k.closing {
                        return self.err("expected <key>");
                    }
                    let key = if k.self_closing {
                        String::new()
                    } else {
                        self.text("key")?
                    };
                    let v = self.value(depth + 1)?;
                    entries.push((key, v));
                }
            }
            "array" => {
                let mut items = Vec::new();
                loop {
                    let i = self.tag()?;
                    if i.closing && i.name == "array" {
                        return Ok(Value::Array(items));
                    }
                    items.push(self.value_from(i, depth + 1)?);
                }
            }
            "string" => Ok(Value::String(self.text("string")?)),
            "date" => Ok(Value::Date(self.text("date")?)),
            "integer" => {
                let s = self.text("integer")?;
                let s = s.trim();
                let n = match s.strip_prefix("0x") {
                    Some(hex) => i128::from_str_radix(hex, 16),
                    None => s.parse(),
                };
                n.map(Value::Integer).or_else(|_| self.err("bad integer"))
            }
            "real" => {
                let s = self.text("real")?;
                s.trim()
                    .parse()
                    .map(Value::Real)
                    .or_else(|_| self.err("bad real"))
            }
            "data" => {
                let s = self.text("data")?;
                base64(&s)
                    .map(Value::Data)
                    .ok_or(PlistError("bad base64".into()))
            }
            _ => self.err("unknown element"),
        }
    }
}

fn unescape(raw: &str) -> String {
    if !raw.contains('&') {
        return raw.to_string();
    }
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';').filter(|&e| e <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let ch = match entity {
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            e => e
                .strip_prefix("#x")
                .map(|h| u32::from_str_radix(h, 16))
                .or_else(|| e.strip_prefix('#').map(str::parse))
                .and_then(Result::ok)
                .and_then(char::from_u32),
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn base64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.bytes().filter(|c| !c.is_ascii_whitespace()) {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            _ => return None,
        };
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<array>
	<dict>
		<key>IOPlatformSerialNumber</key>
		<string>C02XK0AAJG5H</string>
		<key>model</key>
		<data>
		TWFjQm9va1BybzE1LDIA
		</data>
		<key>CycleCount</key>
		<integer>412</integer>
		<key>IOBuiltin</key>
		<true/>
		<key>Note</key>
		<string>A &amp; B &lt;x&gt; &#65;</string>
		<key>IORegistryEntryChildren</key>
		<array>
			<dict>
				<key>BSD Name</key>
				<string>en0</string>
			</dict>
		</array>
		<key>Empty</key>
		<dict/>
	</dict>
</array>
</plist>
"#;

    #[test]
    fn parses_ioreg_output() {
        let v = parse(SAMPLE).unwrap();
        let d = &v.as_array()[0];
        assert_eq!(
            d.get("IOPlatformSerialNumber").and_then(Value::as_str),
            Some("C02XK0AAJG5H")
        );
        assert_eq!(
            d.get("model").and_then(Value::text).as_deref(),
            Some("MacBookPro15,2")
        );
        assert_eq!(d.get("CycleCount").and_then(Value::as_int), Some(412));
        assert_eq!(d.get("IOBuiltin").and_then(Value::as_bool), Some(true));
        assert_eq!(d.get("Note").and_then(Value::as_str), Some("A & B <x> A"));
        assert_eq!(v.dicts().len(), 3);
        assert!(v
            .dicts()
            .iter()
            .any(|d| d.get("BSD Name").and_then(Value::as_str) == Some("en0")));
    }

    #[test]
    fn decodes_base64() {
        assert_eq!(base64("TWFu").unwrap(), b"Man");
        assert_eq!(base64("TWE=").unwrap(), b"Ma");
        assert_eq!(base64("TQ==").unwrap(), b"M");
        assert_eq!(base64(""), Some(vec![]));
        assert_eq!(base64("T!"), None);
    }

    #[test]
    fn rejects_malformed_and_deep_input() {
        assert!(parse("<plist><dict><key>a</key></dict></plist>").is_err());
        assert!(parse("<plist><integer>x</integer></plist>").is_err());
        assert!(parse("<plist><string>unterminated").is_err());
        let deep = format!(
            "<plist>{}{}</plist>",
            "<array>".repeat(10_000),
            "</array>".repeat(10_000)
        );
        assert!(parse(&deep).is_err());
    }

    #[test]
    fn never_panics_on_arbitrary_input() {
        for cut in 0..SAMPLE.len() {
            if SAMPLE.is_char_boundary(cut) {
                let _ = parse(&SAMPLE[..cut]);
            }
        }
        for junk in [
            "",
            "<",
            ">",
            "</>",
            "<plist/>",
            "&#xFFFFFFFF;",
            "<plist><string>&#xD800;</string></plist>",
        ] {
            let _ = parse(junk);
        }
    }
}
