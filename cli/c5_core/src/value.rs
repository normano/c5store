//! The value a document holds, independent of the format it was written in.
//! Only what a configuration file can express and what a secret is made of.

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
  Null,
  Bool(bool),
  Int(i64),
  Float(f64),
  String(String),
  Array(Vec<Value>),
  /// Ordered, since a document keeps the order it was written in.
  Map(Vec<(String, Value)>),
}

impl Value {
  pub fn as_str(&self) -> Option<&str> {
    match self {
      Value::String(s) => Some(s),
      _ => None,
    }
  }

  pub fn as_array(&self) -> Option<&[Value]> {
    match self {
      Value::Array(items) => Some(items),
      _ => None,
    }
  }

  pub fn as_map(&self) -> Option<&[(String, Value)]> {
    match self {
      Value::Map(entries) => Some(entries),
      _ => None,
    }
  }

  pub fn get(&self, key: &str) -> Option<&Value> {
    self.as_map()?.iter().find(|(k, _)| k == key).map(|(_, v)| v)
  }

  /// The name of the kind, for an error that has to say what it found.
  pub fn kind(&self) -> &'static str {
    match self {
      Value::Null => "null",
      Value::Bool(_) => "a boolean",
      Value::Int(_) => "an integer",
      Value::Float(_) => "a float",
      Value::String(_) => "a string",
      Value::Array(_) => "an array",
      Value::Map(_) => "a map",
    }
  }
}

impl From<&str> for Value {
  fn from(s: &str) -> Self {
    Value::String(s.to_owned())
  }
}

impl From<String> for Value {
  fn from(s: String) -> Self {
    Value::String(s)
  }
}
