//! The file's own indentation, learned rather than assumed. A line already in
//! the document is never re-indented; only a line being created needs one, and
//! it takes what the file already does.

/// The whitespace one level of nesting adds, taken from the first place the
/// text nests. A text that never nests has nothing to learn from and gets two
/// spaces.
pub fn unit_of(text: &str) -> String {
  let mut previous: Option<&str> = None;
  for line in text.lines() {
    if line.trim().is_empty() || line.trim_start().starts_with('#') {
      continue;
    }
    let indent = leading(line);
    if let Some(outer) = previous {
      if indent.len() > outer.len() && indent.starts_with(outer) {
        return indent[outer.len()..].to_owned();
      }
    }
    previous = Some(indent);
  }
  "  ".to_owned()
}

/// The whitespace a line begins with.
pub fn leading(line: &str) -> &str {
  &line[..line.len() - line.trim_start().len()]
}
