//! JSON read and in-place write.
//!
//! JSON carries no comments, but it does carry a house style: indentation,
//! spacing after a colon, where the braces sit. Writing replaces the bytes of
//! one value and leaves the rest of the document exactly as it was, so that
//! style survives whether or not this tool would have chosen it.

use crate::document::indent::unit_of;
use crate::error::C5CoreError;
use crate::path::PathSegment;
use crate::value::Value;

/// A parsed value and the bytes it occupies.
#[derive(Debug)]
struct Node {
  value: Parsed,
  start: usize,
  end: usize,
}

#[derive(Debug)]
enum Parsed {
  Null,
  Bool(bool),
  Number(f64, bool),
  String(String),
  Array(Vec<Node>),
  Object(Vec<(String, Node)>),
}

struct Scan<'a> {
  bytes: &'a [u8],
  at: usize,
}

impl<'a> Scan<'a> {
  fn new(text: &'a str) -> Scan<'a> {
    Scan { bytes: text.as_bytes(), at: 0 }
  }

  fn fail(&self, what: &str) -> C5CoreError {
    C5CoreError::YamlDeserialize(format!("JSON parsing failed at byte {}: expected {what}", self.at))
  }

  fn space(&mut self) {
    while self.at < self.bytes.len() && self.bytes[self.at].is_ascii_whitespace() {
      self.at += 1;
    }
  }

  fn byte(&self) -> Option<u8> {
    self.bytes.get(self.at).copied()
  }

  fn expect(&mut self, byte: u8) -> Result<(), C5CoreError> {
    if self.byte() == Some(byte) {
      self.at += 1;
      Ok(())
    } else {
      Err(self.fail(&format!("`{}`", byte as char)))
    }
  }

  fn value(&mut self) -> Result<Node, C5CoreError> {
    self.space();
    let start = self.at;
    let value = match self.byte().ok_or_else(|| self.fail("a value"))? {
      b'{' => self.object()?,
      b'[' => self.array()?,
      b'"' => Parsed::String(self.string()?),
      b't' => self.literal("true", Parsed::Bool(true))?,
      b'f' => self.literal("false", Parsed::Bool(false))?,
      b'n' => self.literal("null", Parsed::Null)?,
      _ => self.number()?,
    };
    Ok(Node { value, start, end: self.at })
  }

  fn literal(&mut self, word: &str, value: Parsed) -> Result<Parsed, C5CoreError> {
    if self.bytes[self.at..].starts_with(word.as_bytes()) {
      self.at += word.len();
      Ok(value)
    } else {
      Err(self.fail(word))
    }
  }

  fn number(&mut self) -> Result<Parsed, C5CoreError> {
    let start = self.at;
    while let Some(b) = self.byte() {
      if b.is_ascii_digit() || matches!(b, b'-' | b'+' | b'.' | b'e' | b'E') {
        self.at += 1;
      } else {
        break;
      }
    }
    let text = std::str::from_utf8(&self.bytes[start..self.at]).map_err(|_| self.fail("a number"))?;
    let integral = !text.contains(['.', 'e', 'E']);
    text.parse::<f64>().map(|n| Parsed::Number(n, integral)).map_err(|_| self.fail("a number"))
  }

  fn string(&mut self) -> Result<String, C5CoreError> {
    self.expect(b'"')?;
    let mut out = String::new();
    loop {
      let byte = self.byte().ok_or_else(|| self.fail("the end of a string"))?;
      self.at += 1;
      match byte {
        b'"' => return Ok(out),
        b'\\' => {
          let escape = self.byte().ok_or_else(|| self.fail("an escape"))?;
          self.at += 1;
          match escape {
            b'"' => out.push('"'),
            b'\\' => out.push('\\'),
            b'/' => out.push('/'),
            b'b' => out.push('\u{8}'),
            b'f' => out.push('\u{c}'),
            b'n' => out.push('\n'),
            b'r' => out.push('\r'),
            b't' => out.push('\t'),
            b'u' => {
              let hex = std::str::from_utf8(&self.bytes[self.at..self.at + 4]).map_err(|_| self.fail("four hex digits"))?;
              let code = u32::from_str_radix(hex, 16).map_err(|_| self.fail("four hex digits"))?;
              out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
              self.at += 4;
            }
            _ => return Err(self.fail("a known escape")),
          }
        }
        _ => {
          let start = self.at - 1;
          while self.at < self.bytes.len() && (self.bytes[self.at] & 0xc0) == 0x80 {
            self.at += 1;
          }
          out.push_str(std::str::from_utf8(&self.bytes[start..self.at]).map_err(|_| self.fail("valid UTF-8"))?);
        }
      }
    }
  }

  fn array(&mut self) -> Result<Parsed, C5CoreError> {
    self.expect(b'[')?;
    let mut items = Vec::new();
    self.space();
    if self.byte() == Some(b']') {
      self.at += 1;
      return Ok(Parsed::Array(items));
    }
    loop {
      items.push(self.value()?);
      self.space();
      match self.byte() {
        Some(b',') => self.at += 1,
        Some(b']') => {
          self.at += 1;
          return Ok(Parsed::Array(items));
        }
        _ => return Err(self.fail("`,` or `]`")),
      }
    }
  }

  fn object(&mut self) -> Result<Parsed, C5CoreError> {
    self.expect(b'{')?;
    let mut entries = Vec::new();
    self.space();
    if self.byte() == Some(b'}') {
      self.at += 1;
      return Ok(Parsed::Object(entries));
    }
    loop {
      self.space();
      let key = self.string()?;
      self.space();
      self.expect(b':')?;
      entries.push((key, self.value()?));
      self.space();
      match self.byte() {
        Some(b',') => self.at += 1,
        Some(b'}') => {
          self.at += 1;
          return Ok(Parsed::Object(entries));
        }
        _ => return Err(self.fail("`,` or `}`")),
      }
    }
  }
}

fn parse(text: &str) -> Result<Node, C5CoreError> {
  let mut scan = Scan::new(text);
  let node = scan.value()?;
  scan.space();
  if scan.at != scan.bytes.len() {
    return Err(scan.fail("the end of the document"));
  }
  Ok(node)
}

pub fn check(text: &str) -> Result<(), C5CoreError> {
  parse(text).map(|_| ())
}

fn step<'a>(node: &'a Node, segment: &PathSegment) -> Option<&'a Node> {
  match (&node.value, segment) {
    (Parsed::Object(entries), PathSegment::Key(key)) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
    (Parsed::Array(items), PathSegment::Index(index)) => items.get(*index),
    (Parsed::Array(items), PathSegment::Query { key, value }) => items.iter().find(|item| {
      matches!(&item.value, Parsed::Object(entries)
        if entries.iter().any(|(k, v)| k == key && matches!(&v.value, Parsed::String(s) if s == value)))
    }),
    _ => None,
  }
}

fn value_of(node: &Node) -> Value {
  match &node.value {
    Parsed::Null => Value::Null,
    Parsed::Bool(b) => Value::Bool(*b),
    Parsed::Number(n, true) => Value::Int(*n as i64),
    Parsed::Number(n, false) => Value::Float(*n),
    Parsed::String(s) => Value::String(s.clone()),
    Parsed::Array(items) => Value::Array(items.iter().map(value_of).collect()),
    Parsed::Object(entries) => Value::Map(entries.iter().map(|(k, v)| (k.clone(), value_of(v))).collect()),
  }
}

pub fn get(text: &str, segments: &[PathSegment]) -> Result<Option<Value>, C5CoreError> {
  let root = parse(text)?;
  let mut node = &root;
  for segment in segments {
    match step(node, segment) {
      Some(next) => node = next,
      None => return Ok(None),
    }
  }
  Ok(Some(value_of(node)))
}

fn escaped(text: &str) -> String {
  let mut out = String::with_capacity(text.len() + 2);
  out.push('"');
  for c in text.chars() {
    match c {
      '"' => out.push_str("\\\""),
      '\\' => out.push_str("\\\\"),
      '\n' => out.push_str("\\n"),
      '\r' => out.push_str("\\r"),
      '\t' => out.push_str("\\t"),
      c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
      c => out.push(c),
    }
  }
  out.push('"');
  out
}

/// `value` as JSON, indented from `at` outwards with the document's own unit.
fn render(value: &Value, at: &str, unit: &str) -> String {
  match value {
    Value::Null => "null".to_owned(),
    Value::Bool(b) => b.to_string(),
    Value::Int(n) => n.to_string(),
    Value::Float(n) => n.to_string(),
    Value::String(s) => escaped(s),
    Value::Array(items) if items.is_empty() => "[]".to_owned(),
    Value::Array(items) => {
      let inner = format!("{at}{unit}");
      let body: Vec<String> = items.iter().map(|i| format!("{inner}{}", render(i, &inner, unit))).collect();
      format!("[\n{}\n{at}]", body.join(",\n"))
    }
    Value::Map(entries) if entries.is_empty() => "{}".to_owned(),
    Value::Map(entries) => {
      let inner = format!("{at}{unit}");
      let body: Vec<String> =
        entries.iter().map(|(k, v)| format!("{inner}{}: {}", escaped(k), render(v, &inner, unit))).collect();
      format!("{{\n{}\n{at}}}", body.join(",\n"))
    }
  }
}

/// The whitespace the line holding byte `at` begins with.
fn column(text: &str, at: usize) -> String {
  let line_start = text[..at].rfind('\n').map(|i| i + 1).unwrap_or(0);
  let line = &text[line_start..at];
  line[..line.len() - line.trim_start().len()].to_owned()
}

pub fn set(text: &str, segments: &[PathSegment], value: &Value) -> Result<String, C5CoreError> {
  let root = parse(text)?;
  let unit = unit_of(text);

  let mut node = &root;
  let mut depth = 0;
  while depth < segments.len() {
    match step(node, &segments[depth]) {
      Some(next) => {
        node = next;
        depth += 1;
      }
      None => break,
    }
  }

  if depth == segments.len() {
    let at = column(text, node.start);
    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..node.start]);
    out.push_str(&render(value, &at, &unit));
    out.push_str(&text[node.end..]);
    return Ok(out);
  }

  // What is left of the path becomes objects under the entry being added.
  let mut nested = value.clone();
  for segment in segments[depth + 1..].iter().rev() {
    let PathSegment::Key(key) = segment else {
      return Err(C5CoreError::YamlNavigation(
        "an index or a query cannot be created in a document that does not hold it".to_owned(),
      ));
    };
    nested = Value::Map(vec![((*key).to_owned(), nested)]);
  }
  let PathSegment::Key(key) = &segments[depth] else {
    return Err(C5CoreError::YamlNavigation(
      "an index or a query cannot be created in a document that does not hold it".to_owned(),
    ));
  };

  let Parsed::Object(entries) = &node.value else {
    return Err(C5CoreError::YamlNavigation(format!("`{key}` cannot be added to a value that is not an object")));
  };

  let at = column(text, node.start);
  let inner = format!("{at}{unit}");
  let rendered = format!("{}: {}", escaped(key), render(&nested, &inner, &unit));

  let mut out = String::with_capacity(text.len() + rendered.len());
  match entries.last() {
    Some((_, last)) => {
      out.push_str(&text[..last.end]);
      out.push_str(",\n");
      out.push_str(&inner);
      out.push_str(&rendered);
      out.push_str(&text[last.end..]);
    }
    None => {
      // An empty object: `{}` becomes a block holding the one entry.
      out.push_str(&text[..node.start]);
      out.push_str(&format!("{{\n{inner}{rendered}\n{at}}}"));
      out.push_str(&text[node.end..]);
    }
  }
  Ok(out)
}
