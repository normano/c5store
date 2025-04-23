use std::collections::HashMap;
use std::convert::TryInto;

use base64::Engine;
use serde::{Deserialize, Serialize};

// Assuming ConfigError is accessible, e.g., via `crate::ConfigError` or `use crate::ConfigError;`
use crate::ConfigError;

// Macro for basic TryInto implementation (non-numeric primitives, collections)
macro_rules! try_into_impl_basic {
  // $target_type: The Rust type to convert into (e.g., bool, String)
  // $c5_variant: The corresponding C5DataValue variant (e.g., Boolean, String)
  // $expected_type_str: A static string describing the expected C5 type (e.g., "Boolean", "String")
  ($target_type:ty, $c5_variant:ident, $expected_type_str:literal) => {
    impl TryInto<$target_type> for C5DataValue {
      type Error = ConfigError;

      #[inline]
      fn try_into(self) -> Result<$target_type, Self::Error> {
        match self {
          C5DataValue::$c5_variant(inner_value) => Ok(inner_value),
          other => Err(ConfigError::TypeMismatch {
            // Key context is not available within TryInto itself.
            // The caller (get_into) handles KeyNotFound before calling try_into.
            key: "_conversion_".to_string(),
            expected_type: $expected_type_str,
            found_type: other.type_name(),
          }),
        }
      }
    }

    // Implementation for converting from a reference (&C5DataValue)
    impl TryInto<$target_type> for &C5DataValue {
      type Error = ConfigError;

      #[inline]
      fn try_into(self) -> Result<$target_type, Self::Error> {
         match self {
          // For owned types like String, Vec, HashMap, we need to clone.
          // For Copy types (like bool, numbers), cloning is cheap/implicit.
          C5DataValue::$c5_variant(inner_value) => Ok(inner_value.clone()),
          other => Err(ConfigError::TypeMismatch {
            key: "_conversion_".to_string(),
            expected_type: $expected_type_str,
            found_type: other.type_name(),
          }),
        }
      }
    }
  };
  // Specific override for Copy types where clone isn't needed on ref access
  ($target_type:ty, $c5_variant:ident, $expected_type_str:literal, Copy) => {
    impl TryInto<$target_type> for C5DataValue {
      type Error = ConfigError;

      #[inline]
      fn try_into(self) -> Result<$target_type, Self::Error> {
        match self {
          C5DataValue::$c5_variant(inner_value) => Ok(inner_value),
          other => Err(ConfigError::TypeMismatch {
            key: "_conversion_".to_string(),
            expected_type: $expected_type_str,
            found_type: other.type_name(),
          }),
        }
      }
    }

    impl TryInto<$target_type> for &C5DataValue {
      type Error = ConfigError;

      #[inline]
      fn try_into(self) -> Result<$target_type, Self::Error> {
         match self {
          C5DataValue::$c5_variant(inner_value) => Ok(*inner_value), // Direct deref for Copy types
          other => Err(ConfigError::TypeMismatch {
            key: "_conversion_".to_string(),
            expected_type: $expected_type_str,
            found_type: other.type_name(),
          }),
        }
      }
    }
  };
}

// Macro specifically for numeric TryInto where casting occurs
// Handles simple casts between C5 Integer/UInteger/Float and Rust numeric types
macro_rules! try_into_impl_numeric_cast {
  // $target_type: The Rust numeric type (e.g., i32, u16, f32)
  // $c5_variant: The primary C5DataValue variant to check (Integer, UInteger, Float)
  // $expected_type_str: Static string for error message
  ($target_type:ty, $c5_variant:ident, $expected_type_str:literal) => {
    impl TryInto<$target_type> for C5DataValue {
      type Error = ConfigError;

      #[inline]
      fn try_into(self) -> Result<$target_type, Self::Error> {
        match self {
          // Direct cast - Rust handles range checks for float->int, etc.
          // but we rely on the source type matching mostly.
          // More robust range checks could be added if needed.
          C5DataValue::$c5_variant(inner_value) => Ok(inner_value as $target_type),
          other => Err(ConfigError::TypeMismatch {
            key: "_conversion_".to_string(),
            expected_type: $expected_type_str,
            found_type: other.type_name(),
          }),
        }
      }
    }

    impl TryInto<$target_type> for &C5DataValue {
      type Error = ConfigError;

      #[inline]
      fn try_into(self) -> Result<$target_type, Self::Error> {
        match self {
          C5DataValue::$c5_variant(inner_value) => Ok(*inner_value as $target_type),
          other => Err(ConfigError::TypeMismatch {
            key: "_conversion_".to_string(),
            expected_type: $expected_type_str,
            found_type: other.type_name(),
          }),
        }
      }
    }
  };
}

// Macro to implement From<primitive> for C5DataValue
macro_rules! from_impl_numeric {
    ($from_type:ty, $c5_variant:ident, $cast_type:ty) => {
        impl From<$from_type> for C5DataValue {
            #[inline]
            fn from(value: $from_type) -> Self {
                C5DataValue::$c5_variant(value as $cast_type)
            }
        }
    };
}

// Macro for Vec<T> TryInto conversion
macro_rules! try_into_impl_vec {
    ($target_element_type:ty) => {
        impl TryInto<Vec<$target_element_type>> for C5DataValue {
            type Error = ConfigError;

            fn try_into(self) -> Result<Vec<$target_element_type>, Self::Error> {
                match self {
                    C5DataValue::Array(inner_value) => inner_value
                        .into_iter()
                        .map(|vec_item| vec_item.try_into()) // Each element conversion can fail
                        .collect::<Result<Vec<$target_element_type>, ConfigError>>(), // Collect results
                    other => Err(ConfigError::TypeMismatch {
                        key: "_conversion_".to_string(),
                        expected_type: "Array",
                        found_type: other.type_name(),
                    }),
                }
            }
        }

        impl TryInto<Vec<$target_element_type>> for &C5DataValue {
            type Error = ConfigError;

            fn try_into(self) -> Result<Vec<$target_element_type>, Self::Error> {
                match self {
                     // Note: .into_iter() on a slice iterates over references
                    C5DataValue::Array(inner_value) => inner_value
                        .iter() // Iterate over references
                        .map(|vec_item_ref| vec_item_ref.try_into()) // TryInto<&C5DataValue> for T
                        .collect::<Result<Vec<$target_element_type>, ConfigError>>(),
                    other => Err(ConfigError::TypeMismatch {
                        key: "_conversion_".to_string(),
                        expected_type: "Array",
                        found_type: other.type_name(),
                    }),
                }
            }
        }
    };
}


#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum C5DataValue {
  Null,
  Bytes(Vec<u8>),
  Boolean(bool),
  // This represents numbers less than zero (or typically signed)
  Integer(i64),
  // This represents non-negative numbers (or typically unsigned)
  UInteger(u64),
  Float(f64),
  String(String),
  Array(Vec<C5DataValue>),
  Map(HashMap<String, C5DataValue>),
}

impl C5DataValue {
  // Helper to get type name as a string for error messages
  pub(crate) fn type_name(&self) -> &'static str {
    match self {
      C5DataValue::Null => "Null",
      C5DataValue::Bytes(_) => "Bytes",
      C5DataValue::Boolean(_) => "Boolean",
      C5DataValue::Integer(_) => "Integer",
      C5DataValue::UInteger(_) => "UInteger",
      C5DataValue::Float(_) => "Float",
      C5DataValue::String(_) => "String",
      C5DataValue::Array(_) => "Array",
      C5DataValue::Map(_) => "Map",
    }
  }

   // Helper method for converting value to bytes - useful internally?
   // Keep this internal or remove if not strictly needed by public API consumers
  pub(crate) fn as_bytes(&self) -> Option<Vec<u8>> {
    match self {
      C5DataValue::String(value) => Some(value.as_bytes().to_vec()),
      C5DataValue::Bytes(value) => Some(value.clone()),
      C5DataValue::Boolean(value) => Some(if *value { vec![1] } else { vec![0] }),
      C5DataValue::Float(value) => Some(value.to_ne_bytes().to_vec()),
      C5DataValue::Integer(value) => Some(value.to_ne_bytes().to_vec()),
      C5DataValue::UInteger(value) => Some(value.to_ne_bytes().to_vec()),
      _ => None,
    }
  }
}

// --- From Implementations ---

impl From<()> for C5DataValue {
  #[inline] fn from(_value: ()) -> Self { C5DataValue::Null }
}
impl From<Vec<u8>> for C5DataValue {
  #[inline] fn from(value: Vec<u8>) -> Self { C5DataValue::Bytes(value) }
}
impl From<bool> for C5DataValue {
  #[inline] fn from(value: bool) -> Self { C5DataValue::Boolean(value) }
}
impl From<String> for C5DataValue {
  #[inline] fn from(value: String) -> Self { C5DataValue::String(value) }
}
impl From<&str> for C5DataValue {
  #[inline] fn from(value: &str) -> Self { C5DataValue::String(value.to_string()) }
}
impl From<Box<str>> for C5DataValue {
  #[inline] fn from(value: Box<str>) -> Self { C5DataValue::String(value.into_string()) }
}
impl From<i64> for C5DataValue {
  #[inline] fn from(value: i64) -> Self { C5DataValue::Integer(value) }
}
impl From<u64> for C5DataValue {
  #[inline] fn from(value: u64) -> Self { C5DataValue::UInteger(value) }
}
impl From<f64> for C5DataValue {
  #[inline] fn from(value: f64) -> Self { C5DataValue::Float(value) }
}
impl From<Vec<C5DataValue>> for C5DataValue {
  #[inline] fn from(value: Vec<C5DataValue>) -> Self { C5DataValue::Array(value) }
}
impl From<HashMap<String, C5DataValue>> for C5DataValue {
  #[inline] fn from(value: HashMap<String, C5DataValue>) -> Self { C5DataValue::Map(value) }
}

// From impls for smaller numeric types using macro
from_impl_numeric!(i8, Integer, i64);
from_impl_numeric!(i16, Integer, i64);
from_impl_numeric!(i32, Integer, i64);
from_impl_numeric!(isize, Integer, i64);
from_impl_numeric!(u8, UInteger, u64);
from_impl_numeric!(u16, UInteger, u64);
from_impl_numeric!(u32, UInteger, u64);
from_impl_numeric!(usize, UInteger, u64);
from_impl_numeric!(f32, Float, f64);


// --- TryInto Implementations ---

// TryInto<()>
impl TryInto<()> for C5DataValue {
  type Error = ConfigError;
  #[inline] fn try_into(self) -> Result<(), Self::Error> {
    match self {
      C5DataValue::Null => Ok(()),
      other => Err(ConfigError::TypeMismatch { key: "_conversion_".to_string(), expected_type: "Null", found_type: other.type_name() }),
    }
  }
}
impl TryInto<()> for &C5DataValue {
  type Error = ConfigError;
  #[inline] fn try_into(self) -> Result<(), Self::Error> {
    match self {
      C5DataValue::Null => Ok(()),
      other => Err(ConfigError::TypeMismatch { key: "_conversion_".to_string(), expected_type: "Null", found_type: other.type_name() }),
    }
  }
}

// TryInto<Vec<u8>> using macro
try_into_impl_basic!(Vec<u8>, Bytes, "Bytes");

// TryInto<bool> using macro (with Copy optimization)
try_into_impl_basic!(bool, Boolean, "Boolean", Copy);

// TryInto<String> using macro
try_into_impl_basic!(String, String, "String");

// TryInto<Box<str>>
impl TryInto<Box<str>> for C5DataValue {
  type Error = ConfigError;
  #[inline] fn try_into(self) -> Result<Box<str>, Self::Error> {
    match self {
      C5DataValue::String(inner_value) => Ok(inner_value.into_boxed_str()),
      other => Err(ConfigError::TypeMismatch { key: "_conversion_".to_string(), expected_type: "String", found_type: other.type_name() }),
    }
  }
}
impl TryInto<Box<str>> for &C5DataValue {
  type Error = ConfigError;
  #[inline] fn try_into(self) -> Result<Box<str>, Self::Error> {
    match self {
      C5DataValue::String(inner_value) => Ok(inner_value.clone().into_boxed_str()),
      other => Err(ConfigError::TypeMismatch { key: "_conversion_".to_string(), expected_type: "String", found_type: other.type_name() }),
    }
  }
}

// --- Numeric TryInto Implementations ---

// TryInto<i64> (Special case: Allow conversion from UInteger if in range)
impl TryInto<i64> for C5DataValue {
  type Error = ConfigError;
  #[inline] fn try_into(self) -> Result<i64, Self::Error> {
    match self {
      C5DataValue::Integer(i) => Ok(i),
      C5DataValue::UInteger(u) => {
        if u <= i64::MAX as u64 {
          Ok(u as i64)
        } else {
          Err(ConfigError::ConversionError {
            key: "_conversion_".to_string(),
            message: format!("UInteger value {} out of range for i64", u),
          })
        }
      },
      other => Err(ConfigError::TypeMismatch { key: "_conversion_".to_string(), expected_type: "Integer or UInteger", found_type: other.type_name() }),
    }
  }
}
impl TryInto<i64> for &C5DataValue {
  type Error = ConfigError;
  #[inline] fn try_into(self) -> Result<i64, Self::Error> {
    match self {
      C5DataValue::Integer(i) => Ok(*i),
      C5DataValue::UInteger(u) => {
        if *u <= i64::MAX as u64 {
          Ok(*u as i64)
        } else {
          Err(ConfigError::ConversionError {
            key: "_conversion_".to_string(),
            message: format!("UInteger value {} out of range for i64", u),
          })
        }
      },
      other => Err(ConfigError::TypeMismatch { key: "_conversion_".to_string(), expected_type: "Integer or UInteger", found_type: other.type_name() }),
    }
  }
}

// TryInto<u64> (Special case: Allow conversion from Integer if non-negative)
impl TryInto<u64> for C5DataValue {
  type Error = ConfigError;
  #[inline] fn try_into(self) -> Result<u64, Self::Error> {
    match self {
      C5DataValue::UInteger(u) => Ok(u),
      C5DataValue::Integer(i) => {
        if i >= 0 {
          Ok(i as u64)
        } else {
           Err(ConfigError::ConversionError {
            key: "_conversion_".to_string(),
            message: format!("Negative Integer value {} cannot be converted to u64", i),
          })
        }
      },
      other => Err(ConfigError::TypeMismatch { key: "_conversion_".to_string(), expected_type: "Integer or UInteger", found_type: other.type_name() }),
    }
  }
}
impl TryInto<u64> for &C5DataValue {
  type Error = ConfigError;
  #[inline] fn try_into(self) -> Result<u64, Self::Error> {
     match self {
      C5DataValue::UInteger(u) => Ok(*u),
      C5DataValue::Integer(i) => {
        if *i >= 0 {
          Ok(*i as u64)
        } else {
           Err(ConfigError::ConversionError {
            key: "_conversion_".to_string(),
            message: format!("Negative Integer value {} cannot be converted to u64", i),
          })
        }
      },
      other => Err(ConfigError::TypeMismatch { key: "_conversion_".to_string(), expected_type: "Integer or UInteger", found_type: other.type_name() }),
    }
  }
}

// TryInto<f64> using macro (Copy type)
try_into_impl_basic!(f64, Float, "Float", Copy);

// TryInto for smaller integer types using casting macro
// Note: These only check the C5 type, not the range. A C5DataValue::Integer(1000)
// could be cast to i8 resulting in overflow if not careful. More robust checks
// could be added using try_into() on the number itself if strictness is required.
try_into_impl_numeric_cast!(i8, Integer, "Integer");
try_into_impl_numeric_cast!(i16, Integer, "Integer");
try_into_impl_numeric_cast!(i32, Integer, "Integer");
try_into_impl_numeric_cast!(isize, Integer, "Integer");
try_into_impl_numeric_cast!(u8, UInteger, "UInteger");
try_into_impl_numeric_cast!(u16, UInteger, "UInteger");
try_into_impl_numeric_cast!(u32, UInteger, "UInteger");
try_into_impl_numeric_cast!(usize, UInteger, "UInteger");

// TryInto for smaller float types
try_into_impl_numeric_cast!(f32, Float, "Float");

// --- Collection TryInto Implementations ---

// TryInto<Vec<C5DataValue>> using macro
try_into_impl_basic!(Vec<C5DataValue>, Array, "Array");

// TryInto<HashMap<String, C5DataValue>> using macro
try_into_impl_basic!(HashMap<String, C5DataValue>, Map, "Map");

// --- Vec<T> TryInto Implementations using macro ---
try_into_impl_vec!(Vec<u8>);
try_into_impl_vec!(bool);
try_into_impl_vec!(String);
try_into_impl_vec!(Box<str>);
try_into_impl_vec!(i64);
try_into_impl_vec!(u64);
try_into_impl_vec!(f64);
// Add others like i32, u32 etc. if needed
try_into_impl_vec!(i32);
try_into_impl_vec!(u32);
try_into_impl_vec!(f32);

pub(in crate) fn c5_value_to_serde_json(c5_value: C5DataValue) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
  match c5_value {
    C5DataValue::Null => Ok(serde_json::Value::Null),
    C5DataValue::Bytes(b) => Ok(serde_json::Value::String(base64::engine::general_purpose::STANDARD.encode(&b))), // Represent bytes as base64 string
    C5DataValue::Boolean(b) => Ok(serde_json::Value::Bool(b)),
    C5DataValue::Integer(i) => Ok(serde_json::json!(i)), // Use json! macro for numbers
    C5DataValue::UInteger(u) => Ok(serde_json::json!(u)),
    C5DataValue::Float(f) => Ok(serde_json::json!(f)),
    C5DataValue::String(s) => Ok(serde_json::Value::String(s)),
    C5DataValue::Array(arr) => {
      let mut json_arr = Vec::with_capacity(arr.len());
      for item in arr {
        json_arr.push(c5_value_to_serde_json(item)?);
      }
      Ok(serde_json::Value::Array(json_arr))
    }
    C5DataValue::Map(map) => {
      let mut json_map = serde_json::Map::with_capacity(map.len());
      for (key, value) in map {
        json_map.insert(key, c5_value_to_serde_json(value)?);
      }
      Ok(serde_json::Value::Object(json_map))
    }
  }
}
