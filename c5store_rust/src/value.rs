use std::collections::HashMap;
use std::convert::TryInto;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum C5DataValue {
  Null,
  Bytes(Vec<u8>),
  Boolean(bool),
  Integer(i64),
  UInteger(u64),
  Float(f64),
  String(String),
  Array(Vec<C5DataValue>),
  Map(HashMap<String, C5DataValue>),
}

impl From<()> for C5DataValue {
  fn from(_value: ()) -> Self {
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

impl From<Vec<u8>> for C5DataValue {
  fn from(value: Vec<u8>) -> Self {
    return C5DataValue::Bytes(value);
  }
}

impl TryInto<Vec<u8>> for C5DataValue {
  type Error = ();

  fn try_into(self) -> Result<Vec<u8>, Self::Error> {

    return match self {
      C5DataValue::Bytes(value) => Result::Ok(value),
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

impl TryInto<Box<str>> for C5DataValue {
  type Error = ();

  fn try_into(self) -> Result<Box<str>, Self::Error> {

    return match self {
      C5DataValue::String(inner_value) => Result::Ok(inner_value.into_boxed_str()),
      _ => Result::Err(()),
    };
  }
}

impl From<i64> for C5DataValue {
  fn from(value: i64) -> Self {
    return C5DataValue::Integer(value);
  }
}

impl TryInto<i64> for C5DataValue {
  type Error = ();

  fn try_into(self) -> Result<i64, Self::Error> {

    return match self {
      C5DataValue::Integer(inner_value) => Result::Ok(inner_value),
      _ => Result::Err(()),
    };
  }
}

impl From<u64> for C5DataValue {
  fn from(value: u64) -> Self {
    return C5DataValue::UInteger(value);
  }
}

impl TryInto<u64> for C5DataValue {
  type Error = ();

  fn try_into(self) -> Result<u64, Self::Error> {

    return match self {
      C5DataValue::UInteger(inner_value) => Result::Ok(inner_value),
      _ => Result::Err(()),
    };
  }
}

impl From<f64> for C5DataValue {
  fn from(value: f64) -> Self {
    return C5DataValue::Float(value);
  }
}

impl TryInto<f64> for C5DataValue {
  type Error = ();

  fn try_into(self) -> Result<f64, Self::Error> {

    return match self {
      C5DataValue::Float(inner_value) => Result::Ok(inner_value),
      _ => Result::Err(()),
    };
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

impl TryInto<Vec<Vec<u8>>> for C5DataValue {
  type Error = ();

  fn try_into(self) -> Result<Vec<Vec<u8>>, Self::Error> {

    return match self {
      C5DataValue::Array(inner_value) => Result::Ok(inner_value.into_iter().map(|vec_item| vec_item.try_into().unwrap()).collect()),
      _ => Result::Err(()),
    };
  }
}

impl TryInto<Vec<bool>> for C5DataValue {
  type Error = ();

  fn try_into(self) -> Result<Vec<bool>, Self::Error> {

    return match self {
      C5DataValue::Array(inner_value) => Result::Ok(inner_value.into_iter().map(|vec_item| vec_item.try_into().unwrap()).collect()),
      _ => Result::Err(()),
    };
  }
}

impl TryInto<Vec<String>> for C5DataValue {
  type Error = ();

  fn try_into(self) -> Result<Vec<String>, Self::Error> {

    return match self {
      C5DataValue::Array(inner_value) => Result::Ok(inner_value.into_iter().map(|vec_item| vec_item.try_into().unwrap()).collect()),
      _ => Result::Err(()),
    };
  }
}

impl TryInto<Vec<Box<str>>> for C5DataValue {
  type Error = ();

  fn try_into(self) -> Result<Vec<Box<str>>, Self::Error> {

    return match self {
      C5DataValue::Array(inner_value) => Result::Ok(inner_value.into_iter().map(|vec_item| vec_item.try_into().unwrap()).collect()),
      _ => Result::Err(()),
    };
  }
}

impl TryInto<Vec<i64>> for C5DataValue {
  type Error = ();

  fn try_into(self) -> Result<Vec<i64>, Self::Error> {

    return match self {
      C5DataValue::Array(inner_value) => Result::Ok(inner_value.into_iter().map(|vec_item| vec_item.try_into().unwrap()).collect()),
      _ => Result::Err(()),
    };
  }
}

impl TryInto<Vec<u64>> for C5DataValue {
  type Error = ();

  fn try_into(self) -> Result<Vec<u64>, Self::Error> {

    return match self {
      C5DataValue::Array(inner_value) => Result::Ok(inner_value.into_iter().map(|vec_item| vec_item.try_into().unwrap()).collect()),
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

impl C5DataValue {

  pub(crate) fn as_bytes(&self) -> Option<Vec<u8>> {
    return match self {
      C5DataValue::String(value) => Some(value.as_bytes().to_vec()),
      C5DataValue::Bytes(value) => Some(value.clone()),
      C5DataValue::Boolean(value) => Some(if *value == true { vec![1] } else {vec![0]}),
      C5DataValue::Float(value) => Some(value.to_ne_bytes().to_vec()),
      C5DataValue::Integer(value) => Some(value.to_ne_bytes().to_vec()),
      C5DataValue::UInteger(value) => Some(value.to_ne_bytes().to_vec()),
      _ => None,
    }
  }
}