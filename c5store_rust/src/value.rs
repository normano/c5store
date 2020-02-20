use std::collections::HashMap;
use std::convert::TryInto;

use serde::{Serialize, Deserialize};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub enum C5DataValue {
  Null,
  Bytes(Vec<u8>),
  Boolean(bool),
  Integer(i64),
  Float(f64),
  String(String),
  Array(Vec<C5DataValue>),
  Map(HashMap<String, C5DataValue>),
}

impl From<()> for C5DataValue {
  fn from(value: ()) -> Self {
    return C5DataValue::Null;
  }
}

impl TryInto<()> for C5DataValue {
  type Error = ();

  fn try_into(self) -> Result<(), Self::Error> {

    return match self {
      C5DataValue::Null => Result::Ok(()),
      _ => Result::Err(()),
    };
  }
}

impl From<bool> for C5DataValue {
  fn from(value: bool) -> Self {
    return C5DataValue::Boolean(value);
  }
}

impl TryInto<bool> for C5DataValue {
  type Error = ();

  fn try_into(self) -> Result<bool, Self::Error> {

    return match self {
      C5DataValue::Boolean(value) => Result::Ok(value),
      _ => Result::Err(()),
    };
  }
}

impl From<String> for C5DataValue {
  fn from(value: String) -> Self {
    return C5DataValue::String(value);
  }
}

impl TryInto<String> for C5DataValue {
  type Error = ();

  fn try_into(self) -> Result<String, Self::Error> {

    return match self {
      C5DataValue::String(inner_value) => Result::Ok(inner_value),
      _ => Result::Err(()),
    };
  }
}

impl From<&str> for C5DataValue {
  fn from(value: &str) -> Self {
    return C5DataValue::String(value.to_string());
  }
}

impl From<Box<str>> for C5DataValue {
  fn from(value: Box<str>) -> Self {
    return C5DataValue::String(value.into_string());
  }
}

impl From<Vec<C5DataValue>> for C5DataValue {
  fn from(value: Vec<C5DataValue>) -> Self {
    return C5DataValue::Array(value);
  }
}

impl TryInto<Vec<C5DataValue>> for C5DataValue {
  type Error = ();

  fn try_into(self) -> Result<Vec<C5DataValue>, Self::Error> {

    return match self {
      C5DataValue::Array(inner_value) => Result::Ok(inner_value),
      _ => Result::Err(()),
    };
  }
}

impl From<HashMap<String, C5DataValue>> for C5DataValue {
  fn from(value: HashMap<String, C5DataValue>) -> Self {
    return C5DataValue::Map(value);
  }
}

impl TryInto<HashMap<String, C5DataValue>> for C5DataValue {
  type Error = ();

  fn try_into(self) -> Result<HashMap<String, C5DataValue>, Self::Error> {

    return match self {
      C5DataValue::Map(inner_value) => Result::Ok(inner_value),
      _ => Result::Err(()),
    };
  }
}