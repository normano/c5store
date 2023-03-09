use std::collections::HashMap;
use std::convert::TryInto;

use paste::paste;
use serde::{Deserialize, Serialize};

macro_rules! to_vec_fn {
  ($($data_ty:ident)*) => {
    paste! {
      $(
        pub fn [<to_vec_ $data_ty>](&self) -> Vec<$data_ty> {
          let data_vals: Vec<C5DataValue> = self.clone().try_into().unwrap();
          return data_vals.into_iter().map(|data_val| data_val.try_into().unwrap()).collect();
        }
      )*
    }
  };
}

macro_rules! int_from_into_impl {
  ($($signed_ty:ident)*) => {
    $(
      impl From<$signed_ty> for C5DataValue {
        #[inline]
        fn from(value: $signed_ty) -> Self {
          return C5DataValue::Integer(value as i64);
        }
      }

      impl TryInto<$signed_ty> for C5DataValue {
        type Error = ();

        fn try_into(self) -> Result<$signed_ty, Self::Error> {

          return match self {
            C5DataValue::Integer(inner_value) => Result::Ok(inner_value as $signed_ty),
            _ => Result::Err(()),
          };
        }
      }

      impl TryInto<$signed_ty> for &C5DataValue {
        type Error = ();

        fn try_into(self) -> Result<$signed_ty, Self::Error> {

          return match self {
            C5DataValue::Integer(inner_value) => Result::Ok(*inner_value as $signed_ty),
            _ => Result::Err(()),
          };
        }
      }
    )*
  };
}

macro_rules! uint_from_into_impl {
  ($($unsigned_ty:ident)*) => {
    $(
      impl From<$unsigned_ty> for C5DataValue {
        #[inline]
        fn from(value: $unsigned_ty) -> Self {
          return C5DataValue::UInteger(value as u64);
        }
      }

      impl TryInto<$unsigned_ty> for C5DataValue {
        type Error = ();

        fn try_into(self) -> Result<$unsigned_ty, Self::Error> {

          return match self {
            C5DataValue::UInteger(inner_value) => Result::Ok(inner_value as $unsigned_ty),
            _ => Result::Err(()),
          };
        }
      }

      impl TryInto<$unsigned_ty> for &C5DataValue {
        type Error = ();

        fn try_into(self) -> Result<$unsigned_ty, Self::Error> {

          return match self {
            C5DataValue::UInteger(inner_value) => Result::Ok(*inner_value as $unsigned_ty),
            _ => Result::Err(()),
          };
        }
      }
    )*
  };
}

macro_rules! float_from_into_impl {
  ($($float_ty:ident)*) => {
    $(
      impl From<$float_ty> for C5DataValue {
        #[inline]
        fn from(f: $float_ty) -> Self {
          return C5DataValue::Float(f as f64);
        }
      }

      impl TryInto<$float_ty> for C5DataValue {
        type Error = ();

        fn try_into(self) -> Result<$float_ty, Self::Error> {

          return match self {
            C5DataValue::Float(inner_value) => Result::Ok(inner_value as $float_ty),
            _ => Result::Err(()),
          };
        }
      }

      impl TryInto<$float_ty> for &C5DataValue {
        type Error = ();

        fn try_into(self) -> Result<$float_ty, Self::Error> {

          return match self {
            C5DataValue::Float(inner_value) => Result::Ok(*inner_value as $float_ty),
            _ => Result::Err(()),
          };
        }
      }
    )*
  }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum C5DataValue {
  Null,
  Bytes(Vec<u8>),
  Boolean(bool),
  // This represents numbers less than zero
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

impl TryInto<()> for &C5DataValue {
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

impl TryInto<Vec<u8>> for &C5DataValue {
  type Error = ();

  fn try_into(self) -> Result<Vec<u8>, Self::Error> {

    let data_val = self.to_owned();
    return data_val.try_into();
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

impl TryInto<bool> for &C5DataValue {
  type Error = ();

  fn try_into(self) -> Result<bool, Self::Error> {

    let data_val = self.to_owned();
    return data_val.try_into();
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

impl TryInto<String> for &C5DataValue {
  type Error = ();

  fn try_into(self) -> Result<String, Self::Error> {

    let data_val = self.to_owned();
    return data_val.try_into();
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

impl TryInto<Box<str>> for &C5DataValue {
  type Error = ();

  fn try_into(self) -> Result<Box<str>, Self::Error> {

    let data_val = self.to_owned();
    return data_val.try_into();
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
      C5DataValue::UInteger(u64_val) => {
        if u64_val <= i64::MAX as u64 {
          Result::Ok(u64_val as i64)
        } else {
          Result::Err(())
        }
      },
      _ => Result::Err(()),
    };
  }
}

impl TryInto<i64> for &C5DataValue {
  type Error = ();

  fn try_into(self) -> Result<i64, Self::Error> {

    let data_val = self.to_owned();
    return data_val.try_into();
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
      C5DataValue::Integer(i64_val) => {
        if i64_val >= 0 {
          Result::Ok(i64_val as u64)
        } else {
          Result::Err(())
        }
      },
      _ => Result::Err(()),
    };
  }
}

impl TryInto<u64> for &C5DataValue {
  type Error = ();

  fn try_into(self) -> Result<u64, Self::Error> {

    let data_val = self.to_owned();
    return data_val.try_into();
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

impl TryInto<f64> for &C5DataValue {
  type Error = ();

  fn try_into(self) -> Result<f64, Self::Error> {

    return match self {
      C5DataValue::Float(inner_value) => Result::Ok(*inner_value),
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

impl TryInto<Vec<C5DataValue>> for &C5DataValue {
  type Error = ();

  fn try_into(self) -> Result<Vec<C5DataValue>, Self::Error> {

    return match self {
      C5DataValue::Array(inner_value) => Result::Ok(inner_value.to_owned()),
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

impl TryInto<Vec<Vec<u8>>> for &C5DataValue {
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

impl TryInto<Vec<bool>> for &C5DataValue {
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

impl TryInto<Vec<String>> for &C5DataValue {
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

impl TryInto<Vec<Box<str>>> for &C5DataValue {
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

impl TryInto<Vec<i64>> for &C5DataValue {
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

impl TryInto<Vec<u64>> for &C5DataValue {
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

impl TryInto<HashMap<String, C5DataValue>> for &C5DataValue {
  type Error = ();

  fn try_into(self) -> Result<HashMap<String, C5DataValue>, Self::Error> {

    return match self {
      C5DataValue::Map(inner_value) => Result::Ok(inner_value.clone()),
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

  to_vec_fn!(i8 i16 i32 isize u8 u16 u32 usize f32 f64);
}

int_from_into_impl!(i8 i16 i32 isize);
uint_from_into_impl!(u8 u16 u32 usize);
float_from_into_impl!(f32);