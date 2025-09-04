// c5cli/src/commands/path_parser.rs

use c5_core::C5CoreError;
use regex::Regex;

#[derive(Debug, PartialEq, Eq)]
pub enum PathSegment<'a> {
  Key(&'a str),
  Index(usize),
  Query { key: &'a str, value: &'a str },
}

/// Parses a c5cli path string into a sequence of navigation segments.
///
/// Supports three types of segments:
/// - Simple keys: `auth.bootstrap`
/// - Array indices: `users[0]`
/// - Key-value queries: `credentials[name="default"]`
///
/// # Returns
/// A `Result` containing a `Vec<PathSegment>` on success, or a `C5CoreError` on failure.
pub fn parse_path<'a>(path_str: &'a str) -> Result<Vec<PathSegment<'a>>, C5CoreError> {
  if path_str.is_empty() {
    return Ok(vec![]);
  }

  // This single regex defines the valid tokens that can appear in a path.
  // It uses named capture groups for clarity.
  let token_re = Regex::new(
    r#"(?x)
        (?P<key>[a-zA-Z_][a-zA-Z0-9_-]*) # A key
        |
        (?P<index>\[[0-9]+\])               # An index like [123]
        |
        (?P<query>\[[a-zA-Z_][a-zA-Z0-9_-]*\s*=\s*(?:"[^"]*"|'[^']*')\]) # A query like [key="value"]
        |
        (?P<sep>\.)                         # A dot separator
    "#,
  )
  .unwrap();

  let mut segments = Vec::new();
  let mut last_token_was_sep = true; // Pretend we start with a separator to allow the first key.

  for caps in token_re.captures_iter(path_str) {
    if let Some(key_match) = caps.name("key") {
      if !last_token_was_sep {
        return Err(C5CoreError::InvalidInput(format!(
          "Invalid path: Missing separator before key '{}'",
          key_match.as_str()
        )));
      }
      segments.push(PathSegment::Key(key_match.as_str()));
      last_token_was_sep = false;
    } else if let Some(index_match) = caps.name("index") {
      // Index/Query can follow a key directly without a dot.
      let index_str = &index_match.as_str()[1..index_match.as_str().len() - 1];
      let index = index_str.parse::<usize>().unwrap();
      segments.push(PathSegment::Index(index));
      last_token_was_sep = false;
    } else if let Some(query_match) = caps.name("query") {
      // Index/Query can follow a key directly without a dot.
      let query_str = &query_match.as_str()[1..query_match.as_str().len() - 1];

      let query_parts_re = Regex::new(r#"^([a-zA-Z_][a-zA-Z0-9_-]*)\s*=\s*(?:"([^"]*)"|'([^']*)')$"#).unwrap();
      if let Some(parts_caps) = query_parts_re.captures(query_str) {
        let key = parts_caps.get(1).unwrap().as_str();
        let value = parts_caps.get(2).or_else(|| parts_caps.get(3)).unwrap().as_str();
        segments.push(PathSegment::Query { key, value });
        last_token_was_sep = false;
      } else {
        // Should be unreachable if the main regex is correct
        return Err(C5CoreError::InvalidInput(format!(
          "Malformed query segment: {}",
          query_match.as_str()
        )));
      }
    } else if caps.name("sep").is_some() {
      if last_token_was_sep {
        // Two separators in a row (e.g., "..")
        return Err(C5CoreError::InvalidInput(
          "Invalid path: Contains consecutive separators '..'".to_string(),
        ));
      }
      last_token_was_sep = true;
    }
  }

  // After iterating, the last token cannot be a separator (e.g., "a.b.")
  if last_token_was_sep && !path_str.is_empty() {
    return Err(C5CoreError::InvalidInput(
      "Path cannot end with a separator '.'".to_string(),
    ));
  }

  // Finally, ensure the entire string was consumed by our tokens.
  // This catches any invalid characters.
  let total_len: usize = token_re.find_iter(path_str).map(|m| m.as_str().len()).sum();
  if total_len != path_str.len() {
    return Err(C5CoreError::InvalidInput(format!(
      "Path contains invalid characters: '{}'",
      path_str
    )));
  }

  Ok(segments)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_simple_key_path() {
    let path = "auth.bootstrap.user";
    let segments = parse_path(path).unwrap();
    assert_eq!(
      segments,
      vec![
        PathSegment::Key("auth"),
        PathSegment::Key("bootstrap"),
        PathSegment::Key("user"),
      ]
    );
  }

  #[test]
  fn test_parse_array_index_path() {
    let path = "users[0].name";
    let segments = parse_path(path).unwrap();
    assert_eq!(
      segments,
      vec![
        PathSegment::Key("users"),
        PathSegment::Index(0),
        PathSegment::Key("name"),
      ]
    );
  }

  #[test]
  fn test_parse_query_path() {
    let path = r#"users[name="admin"].token"#;
    let segments = parse_path(path).unwrap();
    assert_eq!(
      segments,
      vec![
        PathSegment::Key("users"),
        PathSegment::Query {
          key: "name",
          value: "admin"
        },
        PathSegment::Key("token"),
      ]
    );
  }

  #[test]
  fn test_parse_mixed_path() {
    let path = r#"auth.users[0].credentials[type='password'].value"#;
    let segments = parse_path(path).unwrap();
    assert_eq!(
      segments,
      vec![
        PathSegment::Key("auth"),
        PathSegment::Key("users"),
        PathSegment::Index(0),
        PathSegment::Key("credentials"),
        PathSegment::Query {
          key: "type",
          value: "password"
        },
        PathSegment::Key("value"),
      ]
    );
  }

  #[test]
  fn test_parse_invalid_paths() {
    assert!(parse_path("a..b").is_err());
    assert!(parse_path("a[b").is_err());
    assert!(parse_path("a[name=val").is_err());
    assert!(parse_path("a[]").is_err());
    assert!(parse_path(".a").is_err());
    assert!(parse_path("a.b.").is_err());
  }
}
