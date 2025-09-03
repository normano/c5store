pub(crate) mod de {
  // c5store_rust/src/c5_serde_de.rs
  use serde::de::{self, Deserializer, EnumAccess, IntoDeserializer, MapAccess, SeqAccess, VariantAccess, Visitor};
  use serde::Deserialize;
  use std::collections::HashMap; // Keep this, it's generally useful

  use crate::error::ConfigError;
  use crate::value::C5DataValue;

  // Helper to convert our ConfigError into a serde::de::Error
  // fn to_serde_error<E: std::fmt::Display>(e: E) -> ConfigError {
  //   ConfigError::Message(e.to_string())
  // }
  // This helper might not be strictly necessary anymore if ConfigError directly implements serde::de::Error

  // <<< MODIFIED struct definition and impl block signature >>>
  pub struct C5SerdeValueDeserializer<'de> {
    // Changed 'a to 'de
    value: &'de C5DataValue,
  }

  impl<'de> C5SerdeValueDeserializer<'de> {
    // Changed 'a to 'de
    pub fn from_c5(value: &'de C5DataValue) -> Self {
      C5SerdeValueDeserializer { value }
    }
  }

  // Macro to implement deserialize_primitive for C5SerdeValueDeserializer
  // The macro itself doesn't need to change regarding lifetimes here,
  // as it inherits them from the impl block.
  macro_rules! deserialize_primitive_direct {
    // For bool, f32, f64 where C5DataValue variant maps directly
    ($method:ident, $visitor_method:ident, $c5_path:path, $expected_type_str:literal, $val_type:ty) => {
      fn $method<V>(self, visitor: V) -> Result<V::Value, Self::Error>
      where
        V: Visitor<'de>,
      {
        match self.value {
          $c5_path(val) => visitor.$visitor_method(*val as $val_type), // as val_type for consistency, though often direct
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
    // Changed 'a to 'de
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
        C5DataValue::String(s) => visitor.visit_borrowed_str(s), // Use visit_borrowed_str for &str
        C5DataValue::Bytes(b) => visitor.visit_borrowed_bytes(b), // Use visit_borrowed_bytes for &[u8]
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
              // Using ConversionError might be more fitting here
              key: "".to_string(), // Key context is limited here
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
        _ => visitor.visit_some(self), // 'self' is C5SerdeValueDeserializer<'de>
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
      visitor.visit_newtype_struct(self) // 'self' is C5SerdeValueDeserializer<'de>
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      println!(
        "[c5_serde] >> Calling deserialize_seq on value {:?}",
        self.value.type_name()
      );
      match self.value {
        C5DataValue::Array(arr) => visitor.visit_seq(C5SeqAccess::new(arr)),
        C5DataValue::Bytes(b) => {
          // Create a SeqAccess that deserializes each byte directly.
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
                  // Deserialize a single u8 directly.
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
        // self.value is &'de C5DataValue
        C5DataValue::Map(map) => {
          // map is &'de HashMap<String, C5DataValue>
          visitor.visit_map(C5MapAccess::new(map)) // C5MapAccess needs 'de
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
        // self.value is &'de C5DataValue
        C5DataValue::String(s) => {
          // s is &'de String
          // To use into_deserializer for the variant name, we need an owned String
          // or a type that directly implements IntoDeserializer.
          // s.clone().into_deserializer() works if String implements IntoDeserializer.
          // Alternatively, treat it as a string literal.
          visitor.visit_enum(s.as_str().into_deserializer())
        }
        C5DataValue::Map(map) if map.len() == 1 => {
          // map is &'de HashMap
          let (variant_name, variant_value) = map.iter().next().unwrap(); // variant_name is &'de String, variant_value is &'de C5DataValue
          visitor.visit_enum(C5EnumRefAccess {
            variant: variant_name.as_str(), // Pass &'de str
            value: variant_value,           // Pass &'de C5DataValue
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
      // Identifiers are usually strings.
      // If self.value is C5DataValue::String(s), then s is &'de String.
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
      // Create a dummy visitor to consume the value if needed, or just proceed.
      // Serde's IgnoredAny handles this.
      let _ = self.deserialize_any(de::IgnoredAny);
      Ok(visitor.visit_unit()?) // Ensure the unit visit result is propagated if it matters.
                                // The error from deserialize_any would be our ConfigError, which is fine.
                                // But visit_unit is simpler if we just want to signal "ignored".
    }
  }

  // C5MapAccess, C5SeqAccess, and C5EnumRefAccess now also need to be generic over 'de
  // and use it consistently.

  struct C5MapAccess<'de> {
    // iter now yields &'de String and &'de C5DataValue
    iter: std::collections::hash_map::Iter<'de, String, C5DataValue>,
    // current_value is now &'de C5DataValue
    current_value: Option<&'de C5DataValue>,
  }

  impl<'de> C5MapAccess<'de> {
    // map is &'de HashMap<String, C5DataValue>
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
          // key is &'de String, value is &'de C5DataValue
          self.current_value = Some(value);
          // Key is &'de String. Deserialize it as a borrowed string.
          let key_de = key.as_str().into_deserializer();
          seed.deserialize(key_de).map(Some)
        }
        None => Ok(None),
      }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
      V: de::DeserializeSeed<'de>,
    {
      match self.current_value.take() {
        Some(value) => seed.deserialize(C5SerdeValueDeserializer::from_c5(value)), // value is &'de C5DataValue
        None => Err(de::Error::custom(
          "value for map entry missing, next_value_seed called before next_key_seed",
        )),
      }
    }
  }

  struct C5SeqAccess<'de> {
    iter: std::slice::Iter<'de, C5DataValue>, // iter over &'de C5DataValue
  }

  impl<'de> C5SeqAccess<'de> {
    // seq is &'de [C5DataValue]
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
        // .next() gives &'de C5DataValue
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
    type Variant = Self; // Self is C5EnumRefAccess<'de>

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
