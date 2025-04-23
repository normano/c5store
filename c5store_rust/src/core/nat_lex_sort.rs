use std::cmp::Ordering;
use natord::compare as natord_compare;
use natord::compare_ignore_case as natord_compare_ignore;

/// Combines natural ordering with lexicographic ordering.
/// Natural ordering is used when string lengths are different
/// Lexicographic ordering is used for same string length

/// Wrapper for natural lexicographic sorting
#[derive(Debug, Eq, PartialEq)]
pub struct NatLexSort<'a>(pub &'a str);

impl<'a> PartialOrd for NatLexSort<'a> {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl<'a> Ord for NatLexSort<'a> {
  fn cmp(&self, other: &Self) -> Ordering {
    nat_lex_cmp(self.0, other.0)
  }
}

pub trait NatLexSortable {
  fn nat_lex_sort(&mut self);
}

impl NatLexSortable for Vec<String> {
  fn nat_lex_sort(&mut self) {
    self.sort_by(|a, b| nat_lex_cmp(a, b));
  }
}

impl NatLexSortable for Vec<&str> {
  fn nat_lex_sort(&mut self) {
    self.sort_by(|a, b| nat_lex_cmp(a, b));
  }
}

impl NatLexSortable for Vec<&[u8]> {
  fn nat_lex_sort(&mut self) {
    self.sort_by(|a, b| nat_lex_byte_cmp(a, b));
  }
}

pub trait NatLexSortableIgnoreCase {
  fn nat_lex_sort_ignore_case(&mut self);
}

impl NatLexSortableIgnoreCase for Vec<String> {
  fn nat_lex_sort_ignore_case(&mut self) {
    self.sort_by(|a, b| nat_lex_cmp_ignore(a, b));
  }
}

impl NatLexSortableIgnoreCase for Vec<&str> {
  fn nat_lex_sort_ignore_case(&mut self) {
    self.sort_by(|a, b| nat_lex_cmp_ignore(a, b));
  }
}

impl NatLexSortableIgnoreCase for Vec<&[u8]> {
  fn nat_lex_sort_ignore_case(&mut self) {
    self.sort_by(|a, b| nat_lex_byte_cmp_ignore(a, b));
  }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub struct NatLexOrderedString(pub String);

impl Ord for NatLexOrderedString {
  fn cmp(&self, other: &Self) -> Ordering {
    return nat_lex_cmp_ignore(&self.0, &other.0);
  }
}

impl PartialOrd for NatLexOrderedString {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    return Some(nat_lex_cmp_ignore(&self.0, &other.0));
  }
}

impl From<&str> for NatLexOrderedString {
  fn from(value: &str) -> Self {
    return NatLexOrderedString(value.to_string());
  }
}

impl From<Box<str>> for NatLexOrderedString {
  fn from(value: Box<str>) -> Self {
    return NatLexOrderedString(value.to_string());
  }
}

impl Into<Box<str>> for NatLexOrderedString {
  fn into(self) -> Box<str> {
    return self.0.into_boxed_str();
  }
}

impl From<String> for NatLexOrderedString {
  fn from(value: String) -> Self {
    return NatLexOrderedString(value);
  }
}

/// A hybrid comparator for keys:
/// - If the two keys have the same length, perform a plain lexicographical (byte‑wise) comparison.
///   This is useful for fixed‑length identifiers (e.g. ULIDs) which are zero‑padded.
/// - Otherwise, fall back to a natural order comparison that interprets embedded numbers naturally.
pub fn nat_lex_cmp(a: &str, b: &str) -> Ordering {
  if a.len() == b.len() {
    a.cmp(b)
  } else {
    natord_compare(a, b)
  }
}

/// Ignore case version of the nat_lex_cmp fn
pub fn nat_lex_cmp_ignore(a: &str, b: &str) -> Ordering {
  if a.len() == b.len() {
    a.cmp(b)
  } else {
    natord_compare_ignore(a, b)
  }
}

/// Sorts a mutable slice of strings in “natural” order using our hybrid comparator.
///
/// # Examples
///
/// ```
/// # use crate::core::nat_lex_sort;
/// let mut keys = vec![
///     String::from("hub/note/2note.txt"),
///     String::from("hub/note/01JN7YC5RTJKNKKWNZ5FT9K2YS"),
///     String::from("hub/note/01JN4244RAKWNDR48TXFN2XJCY"),
///     String::from("hub/note/1note.txt"),
///     String::from("hub/note/10note.txt"),
/// ];
///
/// nat_lex_sort(&mut keys);
/// assert_eq!(keys, vec![
///     "hub/note/2note.txt",
///     "hub/note/01JN4244RAKWNDR48TXFN2XJCY",
///     "hub/note/01JN7YC5RTJKNKKWNZ5FT9K2YS",
///     "hub/note/1note.txt",
///     "hub/note/10note.txt",
/// ]);
/// ```
pub fn nat_lex_sort<S: AsRef<str>>(keys: &mut [S]) {
  keys.sort_by(|a, b| nat_lex_cmp(a.as_ref(), b.as_ref()));
}

/// Compares two strings in a natural order by working directly on their bytes.
/// It iterates through both strings and, when digits are encountered in both,
/// compares the numeric values without allocating temporary strings.
pub fn nat_lex_byte_cmp(a: &[u8], b: &[u8]) -> Ordering {
  if a.len() == b.len() {
    return a.cmp(b);
  }

  let mut i = 0;
  let mut j = 0;

  while i < a.len() && j < b.len() {
    let ca = a[i];
    let cb = b[j];

    if ca.is_ascii_digit() && cb.is_ascii_digit() {
      let start_i = i;
      let start_j = j;

      // Skip leading zeros
      while i < a.len() && a[i] == b'0' {
        i += 1;
      }
      while j < b.len() && b[j] == b'0' {
        j += 1;
      }

      let num_start_i = i;
      let num_start_j = j;
      while i < a.len() && a[i].is_ascii_digit() {
        i += 1;
      }
      while j < b.len() && b[j].is_ascii_digit() {
        j += 1;
      }

      let len_a = i - num_start_i;
      let len_b = j - num_start_j;

      if len_a != len_b {
        return len_a.cmp(&len_b);
      }

      for k in 0..len_a {
        let da = a[num_start_i + k];
        let db = b[num_start_j + k];
        if da != db {
          return da.cmp(&db);
        }
      }
    } else {
      if ca != cb {
        return ca.cmp(&cb);
      }
      i += 1;
      j += 1;
    }
  }

  // If natural order is equal, fallback to lexicographic order
  a.cmp(b)
}

/// Compares two strings in a natural order by working directly on their bytes.
/// It iterates through both strings and, when digits are encountered in both,
/// compares the numeric values without allocating temporary strings.
pub fn nat_lex_byte_cmp_ignore(a: &[u8], b: &[u8]) -> Ordering {
  // If the lengths are equal, do a full case-insensitive lexicographic compare.
  if a.len() == b.len() {
    for i in 0..a.len() {
      let ca = a[i].to_ascii_lowercase();
      let cb = b[i].to_ascii_lowercase();
      if ca != cb {
          return ca.cmp(&cb);
      }
    }
    // If they are equal ignoring case, fallback to the raw comparison.
    return a.cmp(b);
  }


  let to_lower = |c: u8| if c.is_ascii_uppercase() { c.to_ascii_lowercase() } else { c };

  let mut i = 0;
  let mut j = 0;

  while i < a.len() && j < b.len() {
    let ca = to_lower(a[i]);
    let cb = to_lower(b[j]);

    if ca.is_ascii_digit() && cb.is_ascii_digit() {
      let start_i = i;
      let start_j = j;

      while i < a.len() && a[i] == b'0' {
        i += 1;
      }
      while j < b.len() && b[j] == b'0' {
        j += 1;
      }

      let num_start_i = i;
      let num_start_j = j;
      while i < a.len() && a[i].is_ascii_digit() {
        i += 1;
      }
      while j < b.len() && b[j].is_ascii_digit() {
        j += 1;
      }

      let len_a = i - num_start_i;
      let len_b = j - num_start_j;

      if len_a != len_b {
        return len_a.cmp(&len_b);
      }

      for k in 0..len_a {
        let da = a[num_start_i + k];
        let db = b[num_start_j + k];
        if da != db {
          return da.cmp(&db);
        }
      }
    } else {
      if ca != cb {
        return ca.cmp(&cb);
      }
      i += 1;
      j += 1;
    }
  }

  a.cmp(b)
}

pub fn nat_lex_sort_bytes(keys: &mut [&[u8]]) {
  keys.sort_by(|a, b| nat_lex_byte_cmp(a, b));
}

pub fn nat_lex_sort_bytes_ignore(keys: &mut [&[u8]]) {
  keys.sort_by(|a, b| nat_lex_byte_cmp_ignore(a, b));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_length_ids() {
        // These mimic fixed-length identifiers (like ULIDs).
        let mut keys = vec![
            String::from("01JN4244RAKWNDR48TXFN2XJCY"),
            String::from("01JN7YC5RTJKNKKWNZ5FT9K2YS"),
        ];
        // Lexicographical ordering should be used.
        nat_lex_sort(&mut keys);
        assert_eq!(keys, vec![
            "01JN4244RAKWNDR48TXFN2XJCY",
            "01JN7YC5RTJKNKKWNZ5FT9K2YS",
        ]);
    }

    #[test]
    fn test_variable_length_filenames() {
        // These mimic filenames with variable numeric parts.
        let mut keys = vec![
            String::from("2note.txt"),
            String::from("1note.txt"),
            String::from("10note.txt"),
        ];
        nat_lex_sort(&mut keys);
        // Natural order: "1note.txt" comes before "2note.txt", which comes before "10note.txt"
        assert_eq!(keys, vec![
            "1note.txt",
            "2note.txt",
            "10note.txt",
        ]);
    }

    #[test]
    fn test_mixed_keys() {
        // A mixed array that might come from a keyspace
        // where some parts are fixed-format identifiers and others are natural filenames.
        let mut keys = vec![
            String::from("hub/note/2note.txt"),
            String::from("hub/note/01JN7YC5RTJKNKKWNZ5FT9K2YS"),
            String::from("hub/note/01JN4244RAKWNDR48TXFN2XJCY"),
            String::from("hub/note/1note.txt"),
            String::from("hub/note/10note.txt"),
        ];
        nat_lex_sort(&mut keys);
        // In this scheme, fixed-length segments (the ULIDs) are compared lexicographically,
        // while the variable-length filenames are compared naturally.
        // Expected order (for this example) is defined by our comparator:
        // Keys with fixed-length identifiers compare using .cmp(), so they remain in lex order,
        // while the natural numbers in the filenames are ordered using natord.
        assert_eq!(keys, vec![
            "hub/note/01JN4244RAKWNDR48TXFN2XJCY",
            "hub/note/01JN7YC5RTJKNKKWNZ5FT9K2YS",
            "hub/note/1note.txt",
            "hub/note/2note.txt",
            "hub/note/10note.txt",
        ]);
    }

    #[test]
    fn test_equal_length_fallback() {
        // When two keys are exactly equal in length, we use lexicographical comparison.
        let mut keys = vec![
            String::from("abc123"),
            String::from("abc124"),
            String::from("abc122"),
        ];
        nat_lex_sort(&mut keys);
        assert_eq!(keys, vec![
            "abc122",
            "abc123",
            "abc124",
        ]);
    }
    #[test]
    fn test_byte_nat_lex_cmp_basic_numbers() {
        assert_eq!(nat_lex_byte_cmp(b"file7.txt", b"file10.txt"), Ordering::Less);
        assert_eq!(nat_lex_byte_cmp(b"file10.txt", b"file7.txt"), Ordering::Greater);
        // Leading zeros: these compare as equal numerically.
        assert_eq!(nat_lex_byte_cmp(b"file07.txt", b"file7.txt"), Ordering::Less);
    }

    #[test]
    fn test_nat_lexsort_bytes_simple() {
        let mut keys: Vec<&[u8]> = vec![
            b"2note.txt",
            b"1note.txt",
            b"10note.txt",
        ];
        nat_lex_sort_bytes(&mut keys);

        let result_keys: Vec<&[u8]> = vec![
            b"1note.txt",
            b"2note.txt",
            b"10note.txt",
        ];
        assert_eq!(keys, result_keys);
    }

    #[test]
    fn test_nat_lex_sort_bytes_mixed_keys() {
        let mut keys: Vec<&[u8]> = vec![
            b"hub/note/2note.txt",
            b"hub/note/01JN7YC5RTJKNKKWNZ5FT9K2YS",
            b"hub/note/01JN4244RAKWNDR48TXFN2XJCY",
            b"hub/note/1note.txt",
            b"hub/note/10note.txt",
        ];
        nat_lex_sort_bytes(&mut keys);
        // Expected order: natural numbers in the variable parts are sorted numerically.
        // Fixed-format identifiers (same length) are sorted lexicographically.
        let expected: Vec<&[u8]> = vec![
            b"hub/note/01JN4244RAKWNDR48TXFN2XJCY",
            b"hub/note/01JN7YC5RTJKNKKWNZ5FT9K2YS",
            b"hub/note/1note.txt",
            b"hub/note/2note.txt",
            b"hub/note/10note.txt",
        ];
        assert_eq!(keys, expected);
    }

    #[test]
    fn test_byte_nat_lex_cmp_equal_strings() {
        // When strings are equal, the comparison should be Equal.
        assert_eq!(nat_lex_byte_cmp(b"abc123", b"abc123"), Ordering::Equal);
    }
}