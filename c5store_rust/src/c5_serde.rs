pub(crate) mod de {
  use serde::Deserialize;
  use serde::de::{self, Deserializer, EnumAccess, IntoDeserializer, MapAccess, SeqAccess, VariantAccess, Visitor};
  use std::collections::HashMap;

  use crate::error::ConfigError;
  use crate::value::C5DataValue;


  pub struct C5SerdeValueDeserializer<'de> {
    value: &'de C5DataValue,
  }

  impl<'de> C5SerdeValueDeserializer<'de> {
    pub fn from_c5(value: &'de C5DataValue) -> Self {
      C5SerdeValueDeserializer { value }
    }
  }

  macro_rules! deserialize_primitive_direct {
    // For bool, f32, f64 where C5DataValue variant maps directly
    ($method:ident, $visitor_method:ident, $c5_path:path, $expected_type_str:literal, $val_type:ty) => {
      fn $method<V>(self, visitor: V) -> Result<V::Value, Self::Error>
      where
        V: Visitor<'de>,
      {
        match self.value {
          $c5_path(val) => visitor.$visitor_method(*val as $val_type),
          _ => Err(ConfigError::TypeMismatch {
            key: String::from(""),
            expected_type: $expected_type_str,
            found_type: self.value.type_name(),
          }),
        }
      }
    };
    // For String, Bytes (cloned)
    ($method:ident, $visitor_method:ident, $c5_path:path, $expected_type_str:literal) => {
      fn $method<V>(self, visitor: V) -> Result<V::Value, Self::Error>
      where
        V: Visitor<'de>,
      {
        match self.value {
          $c5_path(val) => visitor.$visitor_method(val.clone()),
          _ => Err(ConfigError::TypeMismatch {
            key: String::from(""),
            expected_type: $expected_type_str,
            found_type: self.value.type_name(),
          }),
        }
      }
    };
    // For String, Bytes (borrowed via accessor)
    ($method:ident, $visitor_method:ident, $c5_path:path, $expected_type_str:literal, ref $val_access:expr) => {
      fn $method<V>(self, visitor: V) -> Result<V::Value, Self::Error>
      where
        V: Visitor<'de>,
      {
        match self.value {
          $c5_path(val) => visitor.$visitor_method($val_access(val)),
          _ => Err(ConfigError::TypeMismatch {
            key: String::from(""),
            expected_type: $expected_type_str,
            found_type: self.value.type_name(),
          }),
        }
      }
    };
  }

  // Handles Integer, UInteger, String (via parse), and Bytes (via from_be_bytes)
  macro_rules! deserialize_integer {
    ($method:ident, $visit_method:ident, $target_type:ty) => {
      fn $method<V>(self, visitor: V) -> Result<V::Value, Self::Error>
      where
        V: Visitor<'de>,
      {
        match self.value {
          C5DataValue::Integer(i) => visitor.$visit_method((*i).try_into().map_err(|e| {
            de::Error::custom(format!(
              "Integer {} out of range for {}: {}",
              i,
              stringify!($target_type),
              e
            ))
          })?),
          C5DataValue::UInteger(u) => visitor.$visit_method((*u).try_into().map_err(|e| {
            de::Error::custom(format!(
              "UInteger {} out of range for {}: {}",
              u,
              stringify!($target_type),
              e
            ))
          })?),
          C5DataValue::String(s) => visitor.$visit_method(s.parse::<$target_type>().map_err(|e| {
            de::Error::custom(format!(
              "Could not parse string '{}' as {}: {}",
              s,
              stringify!($target_type),
              e
            ))
          })?),
          C5DataValue::Bytes(b) => {
            const TARGET_SIZE: usize = std::mem::size_of::<$target_type>();
            if b.len() == TARGET_SIZE {
              let val = <$target_type>::from_be_bytes(b.as_slice().try_into().unwrap());
              visitor.$visit_method(val)
            } else {
              Err(de::Error::custom(format!(
                "Expected {} bytes to deserialize into {}, found {}",
                TARGET_SIZE,
                stringify!($target_type),
                b.len()
              )))
            }
          }
          _ => Err(ConfigError::TypeMismatch {
            key: "".to_string(),
            expected_type: concat!(
              "Integer, UInteger, String, or Bytes (for ",
              stringify!($target_type),
              ")"
            ),
            found_type: self.value.type_name(),
          }),
        }
      }
    };
  }

  // Handles Float, Integer, UInteger, String (via parse), and Bytes (via from_be_bytes)
  macro_rules! deserialize_float {
    ($method:ident, $visit_method:ident, $target_type:ty) => {
      fn $method<V>(self, visitor: V) -> Result<V::Value, Self::Error>
      where
        V: Visitor<'de>,
      {
        match self.value {
          C5DataValue::Float(f) => visitor.$visit_method(*f as $target_type),
          C5DataValue::Integer(i) => visitor.$visit_method(*i as $target_type),
          C5DataValue::UInteger(u) => visitor.$visit_method(*u as $target_type),
          C5DataValue::String(s) => visitor.$visit_method(s.parse::<$target_type>().map_err(|e| {
            de::Error::custom(format!(
              "Could not parse string '{}' as {}: {}",
              s,
              stringify!($target_type),
              e
            ))
          })?),
          C5DataValue::Bytes(b) => {
            const TARGET_SIZE: usize = std::mem::size_of::<$target_type>();
            if b.len() == TARGET_SIZE {
              let val = <$target_type>::from_be_bytes(b.as_slice().try_into().unwrap());
              visitor.$visit_method(val)
            } else {
              Err(de::Error::custom(format!(
                "Expected {} bytes to deserialize into {}, found {}",
                TARGET_SIZE,
                stringify!($target_type),
                b.len()
              )))
            }
          }
          _ => Err(ConfigError::TypeMismatch {
            key: "".to_string(),
            expected_type: concat!(
              "Float, Integer, UInteger, String, or Bytes (for ",
              stringify!($target_type),
              ")"
            ),
            found_type: self.value.type_name(),
          }),
        }
      }
    };
  }

  impl<'de> Deserializer<'de> for C5SerdeValueDeserializer<'de> {
    type Error = ConfigError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      match self.value {
        C5DataValue::Null => visitor.visit_unit(),
        C5DataValue::Boolean(b) => visitor.visit_bool(*b),
        C5DataValue::Integer(i) => visitor.visit_i64(*i),
        C5DataValue::UInteger(u) => visitor.visit_u64(*u),
        C5DataValue::Float(f) => visitor.visit_f64(*f),
        C5DataValue::String(s) => visitor.visit_borrowed_str(s),
        C5DataValue::Bytes(b) => visitor.visit_borrowed_bytes(b),
        C5DataValue::Array(_) => self.deserialize_seq(visitor),
        C5DataValue::Map(_) => self.deserialize_map(visitor),
      }
    }

    // --- Smart Float Deserialization Methods ---
    deserialize_float!(deserialize_f32, visit_f32, f32);
    deserialize_float!(deserialize_f64, visit_f64, f64);

    // --- Custom Lenient Boolean Deserialization ---
    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      match self.value {
        C5DataValue::Boolean(b) => visitor.visit_bool(*b),
        C5DataValue::String(s) => {
          if s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("yes") || s.eq_ignore_ascii_case("on") || s == "1"
          {
            visitor.visit_bool(true)
          } else if s.eq_ignore_ascii_case("false")
            || s.eq_ignore_ascii_case("no")
            || s.eq_ignore_ascii_case("off")
            || s == "0"
          {
            visitor.visit_bool(false)
          } else {
            Err(ConfigError::ConversionError {
              key: "".to_string(),
              message: format!("String value '{}' could not be converted to boolean", s),
            })
          }
        }
        C5DataValue::Integer(i) => {
          if *i == 1 {
            visitor.visit_bool(true)
          } else if *i == 0 {
            visitor.visit_bool(false)
          } else {
            Err(ConfigError::ConversionError {
              key: "".to_string(),
              message: format!(
                "Integer value {} could not be converted to boolean (expected 0 or 1)",
                i
              ),
            })
          }
        }
        C5DataValue::UInteger(u) => {
          if *u == 1 {
            visitor.visit_bool(true)
          } else if *u == 0 {
            visitor.visit_bool(false)
          } else {
            Err(ConfigError::ConversionError {
              key: "".to_string(),
              message: format!(
                "UInteger value {} could not be converted to boolean (expected 0 or 1)",
                u
              ),
            })
          }
        }
        _ => Err(ConfigError::TypeMismatch {
          key: "".to_string(),
          expected_type: "Boolean, boolean-like String, or 0/1 Integer/UInteger",
          found_type: self.value.type_name(),
        }),
      }
    }

    // --- Smart Integer Deserialization Methods ---
    deserialize_integer!(deserialize_i8, visit_i8, i8);
    deserialize_integer!(deserialize_i16, visit_i16, i16);
    deserialize_integer!(deserialize_i32, visit_i32, i32);
    deserialize_integer!(deserialize_i64, visit_i64, i64);
    deserialize_integer!(deserialize_u8, visit_u8, u8);
    deserialize_integer!(deserialize_u16, visit_u16, u16);
    deserialize_integer!(deserialize_u32, visit_u32, u32);
    deserialize_integer!(deserialize_u64, visit_u64, u64);

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      match self.value {
        C5DataValue::String(s) if s.chars().count() == 1 => visitor.visit_char(s.chars().next().unwrap()),
        _ => Err(ConfigError::TypeMismatch {
          key: String::from(""),
          expected_type: "Char (String of len 1)",
          found_type: self.value.type_name(),
        }),
      }
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      match self.value {
        C5DataValue::String(s) => visitor.visit_borrowed_str(s),
        C5DataValue::Bytes(b) => match std::str::from_utf8(b) {
          Ok(s) => visitor.visit_borrowed_str(s),
          Err(e) => Err(de::Error::custom(format!("decrypted bytes are not valid UTF-8: {}", e))),
        },
        _ => Err(ConfigError::TypeMismatch {
          key: "".to_string(),
          expected_type: "String or Bytes (for &str)",
          found_type: self.value.type_name(),
        }),
      }
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      match self.value {
        C5DataValue::String(s) => visitor.visit_string(s.clone()),
        C5DataValue::Bytes(b) => match String::from_utf8(b.clone()) {
          Ok(s) => visitor.visit_string(s),
          Err(e) => Err(de::Error::custom(format!("decrypted bytes are not valid UTF-8: {}", e))),
        },
        _ => Err(ConfigError::TypeMismatch {
          key: "".to_string(),
          expected_type: "String or Bytes (for String)",
          found_type: self.value.type_name(),
        }),
      }
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      match self.value {
        C5DataValue::Bytes(b) => visitor.visit_borrowed_bytes(b),
        C5DataValue::String(s) => visitor.visit_borrowed_bytes(s.as_bytes()),
        _ => Err(ConfigError::TypeMismatch {
          key: "".to_string(),
          expected_type: "Bytes or String (for &[u8])",
          found_type: self.value.type_name(),
        }),
      }
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      match self.value {
        C5DataValue::Bytes(b) => visitor.visit_byte_buf(b.clone()),
        C5DataValue::String(s) => visitor.visit_byte_buf(s.as_bytes().to_vec()),
        _ => Err(ConfigError::TypeMismatch {
          key: "".to_string(),
          expected_type: "Bytes or String (for Vec<u8>)",
          found_type: self.value.type_name(),
        }),
      }
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      match self.value {
        C5DataValue::Null => visitor.visit_none(),
        _ => visitor.visit_some(self),
      }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      match self.value {
        C5DataValue::Null => visitor.visit_unit(),
        _ => Err(ConfigError::TypeMismatch {
          key: String::from(""),
          expected_type: "Null (for unit)",
          found_type: self.value.type_name(),
        }),
      }
    }

    fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      match self.value {
        C5DataValue::Array(arr) => visitor.visit_seq(C5SeqAccess::new(arr)),
        C5DataValue::Bytes(b) => {
          struct BytesSeqAccess<'a> {
            iter: std::slice::Iter<'a, u8>,
          }

          impl<'de, 'a> SeqAccess<'de> for BytesSeqAccess<'a> {
            type Error = ConfigError;

            fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
            where
              T: de::DeserializeSeed<'de>,
            {
              match self.iter.next() {
                Some(&byte) => {
                  seed.deserialize(byte.into_deserializer()).map(Some)
                }
                None => Ok(None),
              }
            }
          }
          visitor.visit_seq(BytesSeqAccess { iter: b.iter() })
        }
        _ => Err(ConfigError::TypeMismatch {
          key: String::from(""),
          expected_type: "Array or Bytes",
          found_type: self.value.type_name(),
        }),
      }
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(self, _name: &'static str, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      match self.value {
        C5DataValue::Map(map) => {
          visitor.visit_map(C5MapAccess::new(map))
        }
        _ => Err(ConfigError::TypeMismatch {
          key: String::from(""),
          expected_type: "Map",
          found_type: self.value.type_name(),
        }),
      }
    }

    fn deserialize_struct<V>(
      self,
      _name: &'static str,
      _fields: &'static [&'static str],
      visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_map(visitor)
    }

    fn deserialize_enum<V>(
      self,
      _name: &'static str,
      _variants: &'static [&'static str],
      visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      match self.value {
        C5DataValue::String(s) => {
          visitor.visit_enum(s.as_str().into_deserializer())
        }
        C5DataValue::Map(map) if map.len() == 1 => {
          let (variant_name, variant_value) = map.iter().next().unwrap();
          visitor.visit_enum(C5EnumRefAccess {
            variant: variant_name.as_str(),
            value: variant_value,
          })
        }
        _ => Err(ConfigError::TypeMismatch {
          key: String::from(""),
          expected_type: "String or Map (for enum)",
          found_type: self.value.type_name(),
        }),
      }
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      match self.value {
        C5DataValue::String(s) => visitor.visit_borrowed_str(s.as_str()),
        _ => Err(ConfigError::TypeMismatch {
          key: String::from(""),
          expected_type: "String (for identifier)",
          found_type: self.value.type_name(),
        }),
      }
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      let _ = self.deserialize_any(de::IgnoredAny);
      Ok(visitor.visit_unit()?)
    }
  }


  struct C5MapAccess<'de> {
    iter: std::collections::hash_map::Iter<'de, String, C5DataValue>,
    current_value: Option<&'de C5DataValue>,
  }

  impl<'de> C5MapAccess<'de> {
    fn new(map: &'de HashMap<String, C5DataValue>) -> Self {
      C5MapAccess {
        iter: map.iter(),
        current_value: None,
      }
    }
  }

  impl<'de> MapAccess<'de> for C5MapAccess<'de> {
    type Error = ConfigError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
      K: de::DeserializeSeed<'de>,
    {
      match self.iter.next() {
        Some((key, value)) => {
          self.current_value = Some(value);

          // The order of attempts is important.

          // 1. Try to parse as a signed integer first. This is the most common case
          //    and correctly handles positive and negative numbers.
          if let Ok(num_key) = key.parse::<i64>() {
            seed.deserialize(num_key.into_deserializer()).map(Some)
          }
          // 2. If that fails, try an unsigned integer. This handles very large positive
          //    numbers that might not fit in an i64.
          else if let Ok(num_key) = key.parse::<u64>() {
            seed.deserialize(num_key.into_deserializer()).map(Some)
          }
          // 3. If it's not an integer, try a float.
          else if let Ok(float_key) = key.parse::<f64>() {
            seed.deserialize(float_key.into_deserializer()).map(Some)
          }
          // 4. If it's not a number, check for boolean strings.
          else if key == "true" {
            seed.deserialize(true.into_deserializer()).map(Some)
          } else if key == "false" {
            seed.deserialize(false.into_deserializer()).map(Some)
          }
          // 5. If all else fails, treat it as a plain string.
          else {
            seed.deserialize(key.as_str().into_deserializer()).map(Some)
          }
        }
        None => Ok(None),
      }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
      V: de::DeserializeSeed<'de>,
    {
      match self.current_value.take() {
        Some(value) => seed.deserialize(C5SerdeValueDeserializer::from_c5(value)),
        None => Err(de::Error::custom(
          "value for map entry missing, next_value_seed called before next_key_seed",
        )),
      }
    }
  }

  struct C5SeqAccess<'de> {
    iter: std::slice::Iter<'de, C5DataValue>,
  }

  impl<'de> C5SeqAccess<'de> {
    fn new(seq: &'de [C5DataValue]) -> Self {
      C5SeqAccess { iter: seq.iter() }
    }
  }

  impl<'de> SeqAccess<'de> for C5SeqAccess<'de> {
    type Error = ConfigError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
      T: de::DeserializeSeed<'de>,
    {
      match self.iter.next() {
        Some(value) => seed.deserialize(C5SerdeValueDeserializer::from_c5(value)).map(Some),
        None => Ok(None),
      }
    }
  }

  struct C5EnumRefAccess<'de> {
    variant: &'de str,
    value: &'de C5DataValue,
  }

  impl<'de> EnumAccess<'de> for C5EnumRefAccess<'de> {
    type Error = ConfigError;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
      V: de::DeserializeSeed<'de>,
    {
      let variant_de = self.variant.into_deserializer();
      let val = seed.deserialize(variant_de)?;
      Ok((val, self))
    }
  }

  impl<'de> VariantAccess<'de> for C5EnumRefAccess<'de> {
    type Error = ConfigError;

    fn unit_variant(self) -> Result<(), Self::Error> {
      match self.value {
        C5DataValue::Null => Ok(()),
        _ => Err(de::Error::custom(format!(
          "Expected Null for unit variant {}, found {:?}",
          self.variant,
          self.value.type_name()
        ))),
      }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
      T: de::DeserializeSeed<'de>,
    {
      seed.deserialize(C5SerdeValueDeserializer::from_c5(self.value))
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      C5SerdeValueDeserializer::from_c5(self.value).deserialize_seq(visitor)
    }

    fn struct_variant<V>(self, _fields: &'static [&'static str], visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      C5SerdeValueDeserializer::from_c5(self.value).deserialize_map(visitor)
    }
  }
}
