use std::cmp::Ordering;

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub struct NaturalOrderedString(pub String);

impl Ord for NaturalOrderedString {
  fn cmp(&self, other: &Self) -> Ordering {
    return natord::compare_ignore_case(&self.0, &other.0);
  }
}

impl PartialOrd for NaturalOrderedString {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    return Some(natord::compare_ignore_case(&self.0, &other.0));
  }
}

impl From<&str> for NaturalOrderedString {
  fn from(value: &str) -> Self {
    return NaturalOrderedString(value.to_string());
  }
}

impl From<Box<str>> for NaturalOrderedString {
  fn from(value: Box<str>) -> Self {
    return NaturalOrderedString(value.to_string());
  }
}

impl Into<Box<str>> for NaturalOrderedString {
  fn into(self) -> Box<str> {
    return self.0.into_boxed_str();
  }
}

impl From<String> for NaturalOrderedString {
  fn from(value: String) -> Self {
    return NaturalOrderedString(value);
  }
}