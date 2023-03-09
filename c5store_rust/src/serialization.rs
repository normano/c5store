use std::collections::HashMap;

use crate::providers::C5RawValue;
use crate::value::C5DataValue;

pub fn deserialize_json(raw_value: C5RawValue) -> C5DataValue {

  let value_result: Result<serde_json::Value, serde_json::Error>;

  match raw_value {
    C5RawValue::Bytes(data) => {

      value_result = serde_json::from_slice(data.as_slice());
    },
    C5RawValue::String(data) => {
      value_result = serde_json::from_str(&data);
    }
  }

  if value_result.is_err() {
    return C5DataValue::Null;
  }

  let value = value_result.unwrap();

  return serde_json_val_to_c5_value(value);
}

pub fn deserialize_yaml(raw_value: C5RawValue) -> C5DataValue {

  let value_result: Result<serde_yaml::Value, serde_yaml::Error>;

  match raw_value {
    C5RawValue::Bytes(data) => {

      value_result = serde_yaml::from_slice(data.as_slice());
    },
    C5RawValue::String(data) => {
      value_result = serde_yaml::from_str(&data);
    }
  }

  if value_result.is_err() {
    return C5DataValue::Null;
  }

  let value = value_result.unwrap();

  return serde_yaml_val_to_c5_value(value);
}

pub fn serde_yaml_val_to_c5_value(raw_value: serde_yaml::Value) -> C5DataValue {

  return match raw_value.clone() {
    serde_yaml::Value::Null => C5DataValue::Null,
    serde_yaml::Value::Bool(value) => C5DataValue::Boolean(value),
    serde_yaml::Value::String(value) => C5DataValue::String(value),
    serde_yaml::Value::Number(value) => {
      if value.is_f64() {
        C5DataValue::Float(value.as_f64().unwrap())
      } else if value.is_u64() {
        C5DataValue::UInteger(value.as_u64().unwrap())
      } else {
        C5DataValue::Integer(value.as_i64().unwrap())
      }
    },
    serde_yaml::Value::Sequence(value) => C5DataValue::Array(value.into_iter().map(|item| serde_yaml_val_to_c5_value(item)).collect()),
    serde_yaml::Value::Mapping(_value) => {
      let map_result: Result<HashMap<serde_yaml::Value, serde_yaml::Value>, serde_yaml::Error> = serde_yaml::from_value(raw_value);

      if map_result.is_err() {
        C5DataValue::Null
      } else {
        let mut new_map = HashMap::new();
        for (key, value) in map_result.unwrap() {
          let final_key = match key {
            serde_yaml::Value::String(key_str) => {
              key_str
            },
            serde_yaml::Value::Number(key_num) => {
              key_num.to_string()
            },
            serde_yaml::Value::Bool(key_num) => {
              key_num.to_string()
            },
            serde_yaml::Value::Null => {
              "null".to_string()
            },
            _ => {
              continue; // Should never happen
            }
          };
          new_map.insert(final_key, serde_yaml_val_to_c5_value(value));
        }

        C5DataValue::Map(new_map)
      }
    },
  };
}

pub fn serde_json_val_to_c5_value(raw_value: serde_json::Value) -> C5DataValue {

  return match raw_value.clone() {
    serde_json::Value::Null => C5DataValue::Null,
    serde_json::Value::Bool(value) => C5DataValue::Boolean(value),
    serde_json::Value::String(value) => C5DataValue::String(value),
    serde_json::Value::Number(value) => {
      if value.is_f64() {
        C5DataValue::Float(value.as_f64().unwrap())
      } else if value.is_i64() {
        C5DataValue::Integer(value.as_i64().unwrap())
      } else {
        C5DataValue::UInteger(value.as_u64().unwrap())
      }
    },
    serde_json::Value::Array(value) => C5DataValue::Array(value.into_iter().map(serde_json_val_to_c5_value).collect()),
    serde_json::Value::Object(_value) => {

      let map_result: Result<HashMap<String, serde_json::Value>, serde_json::Error> = serde_json::from_value(raw_value);

      if map_result.is_err() {
        C5DataValue::Null
      } else {
        let mut new_map = HashMap::new();
        for (key, value) in map_result.unwrap() {
          new_map.insert(key, serde_json_val_to_c5_value(value));
        }

        C5DataValue::Map(new_map)
      }
    },
  };
}