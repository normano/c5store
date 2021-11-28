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
  String(Box<str>),
}

pub type C5Serializer = dyn Fn(C5DataValue) -> C5RawValue + Send + Sync;
pub type C5ValueDeserializer = dyn Fn(C5RawValue) -> C5DataValue + Send + Sync;

pub trait C5ValueProvider: Send + Sync {

  fn register(&mut self, data: &C5DataValue);

  fn unregister(&mut self, key: &str);

  fn hydrate(&self, set_data_fn: &SetDataFn, force: bool, context: &HydrateContext);
}

pub struct C5ValueProviderSchema {
  pub value_provider: Box<str>,
  pub value_key_path: Box<str>,
  pub value_key: Box<str>,
}

impl C5ValueProviderSchema {

  pub fn from_map(
    map: &HashMap<String, C5DataValue>
  ) -> Result<C5ValueProviderSchema, ()> {

    let value_provider: Box<str>;
    let value_key_path: Box<str>;
    let value_key: Box<str>;

    if let C5DataValue::String(vpvalue) = map.get(CONFIG_KEY_PROVIDER).unwrap() {
      value_provider = vpvalue.clone().into_boxed_str();
    } else {

      return Err(());
    }

    if let C5DataValue::String(vpvalue) = map.get(CONFIG_KEY_KEYPATH).unwrap() {
      value_key_path = vpvalue.clone().into_boxed_str();
    } else {

      return Err(());
    }

    if let C5DataValue::String(vpvalue) = map.get(CONFIG_KEY_KEYNAME).unwrap() {
      value_key = vpvalue.clone().into_boxed_str()
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
  pub path: Box<str>,
  pub encoding: Box<str>,
  pub format: Box<str>,
}

impl C5FileValueProviderSchema {

  pub fn new_raw_utf8(
    value_schema: C5ValueProviderSchema,
    path: &str,
  ) -> C5FileValueProviderSchema {
    return C5FileValueProviderSchema {
      value_schema,
      path: Box::from(path),
      encoding: Box::from("utf8"),
      format: Box::from("raw"),
    };
  }
}

pub struct C5FileValueProvider {
  _file_path: Box<str>,
  _key_data_map: HashMap<Box<str>, C5FileValueProviderSchema>,
  _deserializer: HashMap<Box<str>, Box<C5ValueDeserializer>>,
}

impl C5FileValueProvider {

  pub fn new(file_path: &str) -> C5FileValueProvider {

    return C5FileValueProvider {
      _file_path: Box::from(file_path),
      _key_data_map: HashMap::new(),
      _deserializer: HashMap::new(),
    }
  }

  pub fn default(file_path: &str) -> C5FileValueProvider {

    let mut provider = C5FileValueProvider::new(file_path);

    provider.register_deserializer("json", deserialize_json);
    provider.register_deserializer("yaml", deserialize_yaml);

    return provider;
  }

  fn register_deserializer<Deserializer>(&mut self, format_name: &str, deserializer: Deserializer)
  where Deserializer: 'static + Fn(C5RawValue) -> C5DataValue + Send + Sync {

    self._deserializer.insert(
      Box::from(format_name),
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
        let path: Box<str>;
        let encoding: Box<str>;
        let format: Box<str>;

        if let C5DataValue::String(vpvalue) = map.get("path").unwrap() {
          path = vpvalue.clone().into_boxed_str();
        } else {
          return;
        }

        if let Some(encoding_value) = map.get("encoding") {
          if let C5DataValue::String(vpvalue) = encoding_value {
            encoding = vpvalue.clone().into_boxed_str();
          } else {
            return;
          }
        } else {
          encoding = "utf8".to_string().into_boxed_str();
        }

        if let C5DataValue::String(vpvalue) = map.get("format").unwrap() {
          format = vpvalue.clone().into_boxed_str();
        } else {
          return;
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
        file_path = PathBuf::from_iter(&[&*self._file_path, &*vp_schema.path]).canonicalize().unwrap();
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

      set_data_fn(key_path, deserialized_value);
    }
  }
}