use std::collections::HashMap;
use std::fs;
use std::iter::FromIterator;
use std::path::{Path, PathBuf};

use crate::{HydrateContext, SetDataFn};
use crate::serialization::{deserialize_json, deserialize_yaml};
use crate::value::C5DataValue;

pub (in crate) const CONFIG_KEY_KEYNAME: &str = ".key";
pub (in crate) const CONFIG_KEY_KEYPATH: &str = ".keyPath";
pub (in crate) const CONFIG_KEY_PROVIDER: &str = ".provider";

pub enum C5RawValue {
  Bytes(Vec<u8>),
  String(String),
}

pub type C5Serializer = dyn Fn(C5DataValue) -> C5RawValue + Send + Sync;
pub type C5ValueDeserializer = dyn Fn(C5RawValue) -> C5DataValue + Send + Sync;

pub trait C5ValueProvider: Send + Sync {

  fn register(&mut self, data: &C5DataValue);

  fn unregister(&mut self, key: &str);

  fn hydrate(&self, set_data_fn: &SetDataFn, force: bool, context: &HydrateContext);
}

pub struct C5ValueProviderSchema {
  pub value_provider: String,
  pub value_key_path: String,
  pub value_key: String,
}

impl C5ValueProviderSchema {

  pub fn from_map(
    map: &HashMap<String, C5DataValue>
  ) -> Result<C5ValueProviderSchema, ()> {

    let value_provider: String;
    let value_key_path: String;
    let value_key: String;

    if let C5DataValue::String(vpvalue) = map.get(CONFIG_KEY_PROVIDER).unwrap() {
      value_provider = vpvalue.clone();
    } else {

      return Err(());
    }

    if let C5DataValue::String(vpvalue) = map.get(CONFIG_KEY_KEYPATH).unwrap() {
      value_key_path = vpvalue.clone();
    } else {

      return Err(());
    }

    if let C5DataValue::String(vpvalue) = map.get(CONFIG_KEY_KEYNAME).unwrap() {
      value_key = vpvalue.clone()
    } else {

      return Err(());
    }

    return Ok(C5ValueProviderSchema {
      value_provider,
      value_key_path,
      value_key,
    });
  }
}

pub struct C5FileValueProviderSchema {
  pub value_schema: C5ValueProviderSchema,
  pub path: String,
  pub encoding: String,
  pub format: String,
}

impl C5FileValueProviderSchema {

  pub fn new_raw_utf8(
    value_schema: C5ValueProviderSchema,
    path: &str,
  ) -> C5FileValueProviderSchema {
    return C5FileValueProviderSchema {
      value_schema,
      path: path.to_string(),
      encoding: "utf8".to_string(),
      format: "raw".to_string(),
    };
  }
}

pub struct C5FileValueProvider {
  _base_dir_path: String,
  _key_data_map: HashMap<String, C5FileValueProviderSchema>,
  _deserializer: HashMap<String, Box<C5ValueDeserializer>>,
}

impl C5FileValueProvider {

  pub fn new(base_path: &str) -> C5FileValueProvider {

    return C5FileValueProvider {
      _base_dir_path: base_path.to_string(),
      _key_data_map: HashMap::new(),
      _deserializer: HashMap::new(),
    }
  }

  pub fn default(base_path: &str) -> C5FileValueProvider {

    let mut provider = C5FileValueProvider::new(base_path);

    provider.register_deserializer("json", deserialize_json);
    provider.register_deserializer("yaml", deserialize_yaml);

    return provider;
  }

  fn register_deserializer<Deserializer>(&mut self, format_name: &str, deserializer: Deserializer)
  where Deserializer: 'static + Fn(C5RawValue) -> C5DataValue + Send + Sync {

    self._deserializer.insert(
      format_name.to_string(),
      Box::from(deserializer),
    );
  }
}

impl C5ValueProvider for C5FileValueProvider {

  fn register(&mut self, data: &C5DataValue) {

    match data {
      C5DataValue::Map(map) => {
        let value_schema_result = C5ValueProviderSchema::from_map(&map);
        //TODO: above result needs to be logged if it is an error

        let value_schema = value_schema_result.unwrap();
        let path: String;
        let encoding: String;
        let format: String;

        if let C5DataValue::String(vpvalue) = map.get("path").unwrap() {
          path = vpvalue.clone();
        } else {
          return;
        }

        if let Some(encoding_value) = map.get("encoding") {
          if let C5DataValue::String(vpvalue) = encoding_value {
            encoding = vpvalue.clone();
          } else {
            return;
          }
        } else {
          encoding = "utf8".to_string();
        }

        if let Some(format_value) = map.get("format") {
          if let C5DataValue::String(vpvalue) = format_value {
            format = vpvalue.clone();
          } else {
            return;
          }
        } else {
          format = "raw".to_string();
        }

        let vp_data = C5FileValueProviderSchema {
          value_schema,
          path,
          encoding,
          format,
        };

        self._key_data_map.insert(vp_data.value_schema.value_key_path.clone(), vp_data);
      }
      _ => (),
    }
  }

  fn unregister(&mut self, key: &str) {

    self._key_data_map.remove(key);
  }

  fn hydrate(
    &self,
    set_data_fn: &SetDataFn,
    _force: bool,
    context: &HydrateContext
  ) {

    for (key_path, vp_schema) in self._key_data_map.iter() {

      let mut file_path = PathBuf::new();
      file_path.push(Path::new(&*vp_schema.path));

      if !file_path.is_absolute() {
        file_path = PathBuf::from_iter(&[&*self._base_dir_path, &*vp_schema.path]).canonicalize().unwrap();
      }

      if !file_path.exists() {
        set_data_fn(key_path.as_ref(), C5DataValue::Null);
        return;
      }

      let file_bytes = fs::read(file_path).unwrap();
      let deserialized_value: C5DataValue;

      if &*vp_schema.format != "raw" {
        if !self._deserializer.contains_key(&*vp_schema.format) {

          context.logger.warn(
            format!(
              "{} cannot be deserialized since deserializer {} does not exist",
              vp_schema.value_schema.value_key_path,
              vp_schema.format
            ).as_str()
          );
          continue;
        }

        let deserializer = self._deserializer.get(&vp_schema.format).unwrap();
        let raw_value = C5RawValue::Bytes(file_bytes);
        deserialized_value = deserializer(raw_value);
      } else {
        deserialized_value = C5DataValue::Bytes(file_bytes);
      }

      HydrateContext::push_value_to_data_store(set_data_fn, key_path, deserialized_value);
    }
  }
}

#[cfg(test)]
mod tests {
    use crate::{providers::C5FileValueProvider, value::C5DataValue, C5Store, C5StoreMgr, default_config_paths, create_c5store};


  #[test]
  fn test_config_contains_example_junk() {
    let (c5store, mut c5store_mgr) = _create_c5store();

    let file_path = "resources";
    c5store_mgr.set_value_provider("resources", C5FileValueProvider::default(file_path), 3);

    assert_eq!(c5store.get("example.junk.some").unwrap(), C5DataValue::String(String::from("data")));
    assert_eq!(c5store.get("example.junk.very").unwrap(), C5DataValue::String(String::from("doge")));
  }

  fn _create_c5store() -> (impl C5Store, C5StoreMgr) {
    let config_file_paths = default_config_paths("configs/test/config", "development", "local", "private");

    return create_c5store(config_file_paths, None);
  }
}