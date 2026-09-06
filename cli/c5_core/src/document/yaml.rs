//! YAML read and in-place write.
//!
//! Reading is yaml-rust2's parse. Writing replaces the lines one entry
//! occupies, or adds an entry to the block that should hold it, and leaves
//! every other byte alone. A value written in flow style, `key: {a: 1}`, has
//! no lines of its own to replace and is refused by name rather than reflowed.

use yaml_rust2::parser::{Event, MarkedEventReceiver, Parser};
use yaml_rust2::scanner::Marker;

use crate::document::indent::{leading, unit_of};
use crate::error::C5CoreError;
use crate::path::PathSegment;
use crate::value::Value;

/// Where a node begins. yaml-rust2 reports `line` from one and, despite what
/// its documentation says, `col` from zero.
#[derive(Debug, Clone, Copy)]
struct At {
  line: usize,
  col: usize,
}

impl From<Marker> for At {
  fn from(marker: Marker) -> At {
    At { line: marker.line(), col: marker.col() }
  }
}

#[derive(Debug)]
enum Node {
  Scalar { at: At, value: String },
  Seq { at: At, items: Vec<usize> },
  Map { at: At, entries: Vec<Entry> },
}

#[derive(Debug)]
struct Entry {
  key: String,
  key_at: At,
  value: usize,
}

impl Node {
  fn at(&self) -> At {
    match self {
      Node::Scalar { at, .. } | Node::Seq { at, .. } | Node::Map { at, .. } => *at,
    }
  }
}

/// The document as an arena of nodes, each knowing where it started.
#[derive(Default)]
struct Tree {
  nodes: Vec<Node>,
  root: Option<usize>,
  /// What is being built: a sequence, or a mapping with the key it is waiting on.
  stack: Vec<Frame>,
  done: bool,
}

enum Frame {
  Seq { node: usize },
  Map { node: usize, pending: Option<(String, At)> },
}

impl Tree {
  fn push(&mut self, node: Node) -> usize {
    self.nodes.push(node);
    self.nodes.len() - 1
  }

  /// Hands a finished node to whatever is building above it.
  fn place(&mut self, id: usize) {
    match self.stack.last_mut() {
      None => {
        if self.root.is_none() {
          self.root = Some(id);
        }
      }
      Some(Frame::Seq { node }) => {
        let node = *node;
        if let Node::Seq { items, .. } = &mut self.nodes[node] {
          items.push(id);
        }
      }
      Some(Frame::Map { node, pending }) => {
        let node = *node;
        match pending.take() {
          None => {
            // A key: remembered until its value arrives.
            let key = match &self.nodes[id] {
              Node::Scalar { value, .. } => value.clone(),
              other => format!("{:?}", other.at().line),
            };
            let at = self.nodes[id].at();
            if let Some(Frame::Map { pending, .. }) = self.stack.last_mut() {
              *pending = Some((key, at));
            }
          }
          Some((key, key_at)) => {
            if let Node::Map { entries, .. } = &mut self.nodes[node] {
              entries.push(Entry { key, key_at, value: id });
            }
          }
        }
      }
    }
  }
}

impl MarkedEventReceiver for Tree {
  fn on_event(&mut self, event: Event, mark: Marker) {
    if self.done {
      return;
    }
    let at = At::from(mark);
    match event {
      Event::Scalar(value, _style, _anchor, _tag) => {
        let id = self.push(Node::Scalar { at, value });
        self.place(id);
      }
      Event::SequenceStart(..) => {
        let id = self.push(Node::Seq { at, items: Vec::new() });
        self.stack.push(Frame::Seq { node: id });
      }
      Event::MappingStart(..) => {
        let id = self.push(Node::Map { at, entries: Vec::new() });
        self.stack.push(Frame::Map { node: id, pending: None });
      }
      Event::SequenceEnd | Event::MappingEnd => {
        let id = match self.stack.pop() {
          Some(Frame::Seq { node }) | Some(Frame::Map { node, .. }) => node,
          None => return,
        };
        self.place(id);
      }
      Event::DocumentEnd => self.done = true,
      Event::Alias(_) => {
        let id = self.push(Node::Scalar { at, value: String::new() });
        self.place(id);
      }
      _ => {}
    }
  }
}

fn tree_of(text: &str) -> Result<Tree, C5CoreError> {
  let mut tree = Tree::default();
  Parser::new_from_str(text)
    .load(&mut tree, false)
    .map_err(|e| C5CoreError::YamlDeserialize(format!("YAML parsing failed: {e}")))?;
  Ok(tree)
}

pub fn check(text: &str) -> Result<(), C5CoreError> {
  tree_of(text).map(|_| ())
}

/// The node `segments` names, if the document holds one.
fn find(tree: &Tree, segments: &[PathSegment]) -> Option<usize> {
  let mut current = tree.root?;
  for segment in segments {
    current = step(tree, current, segment)?;
  }
  Some(current)
}

/// The one node a segment names, refusing a query that matches more than one:
/// a write has to know which object it is changing.
fn step_once(tree: &Tree, node: usize, segment: &PathSegment) -> Result<Option<usize>, C5CoreError> {
  if let (Node::Seq { items, .. }, PathSegment::Query { key, value }) = (&tree.nodes[node], segment) {
    let matched: Vec<usize> = items
      .iter()
      .copied()
      .filter(|item| {
        matches!(&tree.nodes[*item], Node::Map { entries, .. }
          if entries.iter().any(|e| e.key == *key && matches!(&tree.nodes[e.value], Node::Scalar { value: v, .. } if v == value)))
      })
      .collect();
    return match matched.len() {
      0 => Ok(None),
      1 => Ok(Some(matched[0])),
      n => Err(C5CoreError::YamlNavigation(format!(
        "Query '[{key}={value}]' matched multiple objects ({n}). Path must be unique for encryption."
      ))),
    };
  }
  Ok(step(tree, node, segment))
}

fn step(tree: &Tree, node: usize, segment: &PathSegment) -> Option<usize> {
  match (&tree.nodes[node], segment) {
    (Node::Map { entries, .. }, PathSegment::Key(key)) => entries.iter().find(|e| e.key == *key).map(|e| e.value),
    (Node::Seq { items, .. }, PathSegment::Index(index)) => items.get(*index).copied(),
    (Node::Seq { items, .. }, PathSegment::Query { key, value }) => items.iter().copied().find(|item| {
      matches!(&tree.nodes[*item], Node::Map { entries, .. }
        if entries.iter().any(|e| e.key == *key && matches!(&tree.nodes[e.value], Node::Scalar { value: v, .. } if v == value)))
    }),
    _ => None,
  }
}

fn value_of(tree: &Tree, node: usize) -> Value {
  match &tree.nodes[node] {
    Node::Scalar { value, .. } => Value::String(value.clone()),
    Node::Seq { items, .. } => Value::Array(items.iter().map(|i| value_of(tree, *i)).collect()),
    Node::Map { entries, .. } => {
      Value::Map(entries.iter().map(|e| (e.key.clone(), value_of(tree, e.value))).collect())
    }
  }
}

pub fn get(text: &str, segments: &[PathSegment]) -> Result<Option<Value>, C5CoreError> {
  let tree = tree_of(text)?;
  Ok(find(&tree, segments).map(|node| value_of(&tree, node)))
}

/// Whether a string can be written as it is and read back as the same string.
fn plain(text: &str) -> bool {
  if text.is_empty() || text != text.trim() {
    return false;
  }
  if text.contains(['\n', '\r', '\t']) || text.contains(": ") || text.contains(" #") || text.ends_with(':') {
    return false;
  }
  let first = text.chars().next().expect("checked not empty");
  if "-?:,[]{}#&*!|>'\"%@`".contains(first) {
    return false;
  }
  // `.inf` and `.nan` are floats to a YAML reader, so a leading dot is never
  // written bare.
  if first == '.' {
    return false;
  }
  if matches!(text.to_ascii_lowercase().as_str(), "true" | "false" | "null" | "~" | "yes" | "no" | "on" | "off") {
    return false;
  }
  true
}

/// A value written as it is reads back as the same string, unless it would
/// read back as a number instead.
fn plain_value(text: &str) -> bool {
  plain(text) && text.parse::<f64>().is_err()
}

/// A key that is written bare, which leaves a numeric key an integer key,
/// the way c5store has always filed one.
fn quoted_key(text: &str) -> String {
  if plain(text) {
    return text.to_owned();
  }
  escaped(text)
}

/// A YAML string that reads back as itself: as written when that is
/// unambiguous, quoted when it is not.
fn quoted(text: &str) -> String {
  if plain_value(text) {
    return text.to_owned();
  }
  escaped(text)
}

fn escaped(text: &str) -> String {
  let mut out = String::with_capacity(text.len() + 2);
  out.push('"');
  for c in text.chars() {
    match c {
      '"' => out.push_str("\\\""),
      '\\' => out.push_str("\\\\"),
      '\n' => out.push_str("\\n"),
      c => out.push(c),
    }
  }
  out.push('"');
  out
}

/// `value` as block YAML, every line already carrying `at`, the first one
/// written after `head` on its own line.
fn render(head: &str, value: &Value, at: &str, unit: &str, out: &mut Vec<String>) {
  match value {
    Value::Null => out.push(format!("{at}{head} null")),
    Value::Bool(b) => out.push(format!("{at}{head} {b}")),
    Value::Int(n) => out.push(format!("{at}{head} {n}")),
    Value::Float(n) => out.push(format!("{at}{head} {n}")),
    Value::String(s) => out.push(format!("{at}{head} {}", quoted(s))),
    Value::Array(items) if items.is_empty() => out.push(format!("{at}{head} []")),
    Value::Array(items) => {
      out.push(format!("{at}{head}"));
      let inner = format!("{at}{unit}");
      for item in items {
        render("-", item, &inner, unit, out);
      }
    }
    Value::Map(entries) if entries.is_empty() => out.push(format!("{at}{head} {{}}")),
    Value::Map(entries) => {
      out.push(format!("{at}{head}"));
      let inner = format!("{at}{unit}");
      for (key, item) in entries {
        render(&format!("{}:", quoted_key(key)), item, &inner, unit, out);
      }
    }
  }
}

/// The lines an entry occupies: its key line through the last line belonging
/// to its value. Blank lines and comments trailing the value belong to
/// whatever comes next and are left where they are.
fn extent(lines: &[&str], key_line: usize, key_indent: usize) -> (usize, usize) {
  let mut end = key_line + 1;
  let mut last_content = end;
  while end < lines.len() {
    let line = lines[end];
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') {
      end += 1;
      continue;
    }
    if leading(line).len() <= key_indent {
      break;
    }
    end += 1;
    last_content = end;
  }
  (key_line, last_content)
}

pub fn set(text: &str, segments: &[PathSegment], value: &Value) -> Result<String, C5CoreError> {
  let tree = tree_of(text)?;
  let unit = unit_of(text);
  let lines: Vec<&str> = text.lines().collect();

  // As far down the path as the document already goes.
  let mut node = tree.root;
  let mut depth = 0;
  while depth < segments.len() {
    let Some(current) = node else { break };
    match step_once(&tree, current, &segments[depth])? {
      Some(next) => {
        node = Some(next);
        depth += 1;
      }
      None => break,
    }
  }

  if depth == segments.len() {
    return replace(text, &lines, &tree, segments, node.expect("walked the whole path"), value, &unit);
  }
  insert(text, &lines, &tree, segments, node, depth, value, &unit)
}

/// The path exists: its entry's lines go, the rendering takes their place.
fn replace(
  text: &str,
  lines: &[&str],
  tree: &Tree,
  segments: &[PathSegment],
  node: usize,
  value: &Value,
  unit: &str,
) -> Result<String, C5CoreError> {
  let (key, key_at) = key_of(tree, segments, node).ok_or_else(|| {
    C5CoreError::YamlNavigation(format!("`{}` is not an entry of a mapping, so it cannot be replaced.", name(segments)))
  })?;
  let value_at = tree.nodes[node].at();
  if value_at.line == key_at.line && !matches!(tree.nodes[node], Node::Scalar { .. }) {
    return Err(C5CoreError::YamlNavigation(format!(
      "`{}` is written in flow style on one line, which cannot be edited in place; rewrite it as a block first.",
      name(segments)
    )));
  }
  let key_line = key_at.line - 1;
  let key_indent = " ".repeat(key_at.col);
  let (start, end) = extent(lines, key_line, key_indent.len());

  let mut rendered = Vec::new();
  render(&format!("{}:", quoted_key(&key)), value, &key_indent, unit, &mut rendered);
  Ok(splice(text, lines, start, end, rendered))
}

/// The path stops short: the deepest map that exists gains an entry, and the
/// rest of the path is written under it.
fn insert(
  text: &str,
  lines: &[&str],
  tree: &Tree,
  segments: &[PathSegment],
  node: Option<usize>,
  depth: usize,
  value: &Value,
  unit: &str,
) -> Result<String, C5CoreError> {
  // Everything the path still has to say, innermost first.
  let mut nested = value.clone();
  for segment in segments[depth + 1..].iter().rev() {
    let PathSegment::Key(key) = segment else {
      return Err(uncreatable(segment));
    };
    nested = Value::Map(vec![((*key).to_owned(), nested)]);
  }
  let PathSegment::Key(key) = &segments[depth] else {
    return Err(uncreatable(&segments[depth]));
  };

  let Some(parent) = node else {
    // An empty document: the whole path is the document.
    let mut rendered = Vec::new();
    render(&format!("{}:", quoted_key(key)), &nested, "", unit, &mut rendered);
    let mut out = rendered.join("\n");
    out.push('\n');
    return Ok(out);
  };

  let Node::Map { entries, at } = &tree.nodes[parent] else {
    return Err(C5CoreError::YamlNavigation(format!(
      "`{}` is not a mapping, so `{key}` cannot be added to it.",
      name(&segments[..depth])
    )));
  };

  let (indent, after) = match entries.last() {
    Some(last) => {
      let line = last.key_at.line - 1;
      let indent = " ".repeat(last.key_at.col);
      let (_, end) = extent(lines, line, indent.len());
      (indent, end)
    }
    // A mapping with no entries at all is only the document itself.
    None => (" ".repeat(at.col), lines.len()),
  };

  let mut rendered = Vec::new();
  render(&format!("{}:", quoted_key(key)), &nested, &indent, unit, &mut rendered);
  Ok(splice(text, lines, after, after, rendered))
}

/// The key an entry is filed under, and where that key is written.
fn key_of(tree: &Tree, segments: &[PathSegment], node: usize) -> Option<(String, At)> {
  let PathSegment::Key(_) = segments.last()? else { return None };
  tree
    .nodes
    .iter()
    .find_map(|n| match n {
      Node::Map { entries, .. } => entries.iter().find(|e| e.value == node).map(|e| (e.key.clone(), e.key_at)),
      _ => None,
    })
}

/// A segment that names something the document has to already hold.
fn uncreatable(segment: &PathSegment) -> C5CoreError {
  C5CoreError::YamlNavigation(match segment {
    PathSegment::Query { key, value } => {
      format!("Query '[{key}={value}]' matched no objects. Cannot encrypt.")
    }
    PathSegment::Index(index) => format!("Index [{index}] is out of bounds. Cannot encrypt."),
    PathSegment::Key(key) => format!("`{key}` cannot be created here."),
  })
}

fn name(segments: &[PathSegment]) -> String {
  segments
    .iter()
    .map(|s| match s {
      PathSegment::Key(k) => (*k).to_owned(),
      PathSegment::Index(i) => format!("[{i}]"),
      PathSegment::Query { key, value } => format!("[{key}={value}]"),
    })
    .collect::<Vec<_>>()
    .join(".")
}

/// Lines `start..end` become `rendered`; the trailing newline the file had is
/// the trailing newline it keeps.
fn splice(text: &str, lines: &[&str], start: usize, end: usize, rendered: Vec<String>) -> String {
  let mut out: Vec<String> = Vec::with_capacity(lines.len() + rendered.len());
  out.extend(lines[..start].iter().map(|l| (*l).to_owned()));
  out.extend(rendered);
  out.extend(lines[end..].iter().map(|l| (*l).to_owned()));
  let mut joined = out.join("\n");
  if text.ends_with('\n') || text.is_empty() {
    joined.push('\n');
  }
  joined
}
