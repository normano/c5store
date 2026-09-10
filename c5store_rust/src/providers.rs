use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::serialization::{SerializationError, deserialize_json, deserialize_yaml};
use crate::value::C5DataValue;
use crate::{HydrateContext, SetDataFn};

pub(crate) const CONFIG_KEY_KEYNAME: &str = ".key";
pub(crate) const CONFIG_KEY_KEYPATH: &str = ".keyPath";
pub(crate) const CONFIG_KEY_PROVIDER: &str = ".provider";

pub enum C5RawValue {
  Bytes(Vec<u8>),
  String(String),
}

/// Why a provider section could not be read. Every variant names the section
/// it came from, since a section is assembled from every config file that
/// mentions it and the offending key is often not in the file being edited.
#[derive(Error, Debug)]
pub enum ProviderSchemaError {
  #[error("`{key_path}` is missing `{key}`")]
  Missing { key_path: String, key: &'static str },
  #[error("`{key_path}` has a `{key}` that is not a string")]
  NotAString { key_path: String, key: &'static str },
  #[error(
    "`{key_path}` names both `path` and `paths`; these merge across config files, so the base section must use `paths` as well"
  )]
  PathAndPaths { key_path: String },
  #[error("`{key_path}` names neither `path` nor `paths`")]
  NoPath { key_path: String },
  #[error("`{key_path}` has a `paths` that is not a list of strings")]
  PathsNotStrings { key_path: String },
  #[error("`{key_path}` has an empty `paths`")]
  PathsEmpty { key_path: String },
}

pub type C5Serializer = dyn Fn(C5DataValue) -> C5RawValue + Send + Sync;
pub type C5ValueDeserializer = dyn Fn(C5RawValue) -> Result<C5DataValue, SerializationError> + Send + Sync;

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
  pub fn from_map(map: &HashMap<String, C5DataValue>) -> Result<C5ValueProviderSchema, ProviderSchemaError> {
    let key_path = string_at(map, CONFIG_KEY_KEYPATH, CONFIG_KEY_KEYPATH)?;

    return Ok(C5ValueProviderSchema {
      value_provider: string_at(map, CONFIG_KEY_PROVIDER, &key_path)?,
      value_key: string_at(map, CONFIG_KEY_KEYNAME, &key_path)?,
      value_key_path: key_path,
    });
  }
}

/// One required string from a section, named by the section it belongs to so a
/// message points at the right place even when the key came from another file.
fn string_at(
  map: &HashMap<String, C5DataValue>,
  key: &'static str,
  key_path: &str,
) -> Result<String, ProviderSchemaError> {
  match map.get(key) {
    Some(C5DataValue::String(value)) => Ok(value.clone()),
    Some(_) => Err(ProviderSchemaError::NotAString {
      key_path: key_path.to_owned(),
      key,
    }),
    None => Err(ProviderSchemaError::Missing {
      key_path: key_path.to_owned(),
      key,
    }),
  }
}

/// The files a section names, in the order they are read, with a later file's
/// keys landing over an earlier one's. `path` and `paths` are exclusive: they
/// merge across the ladder, so accepting both would make the effective order
/// depend on which files happened to mention the section.
fn paths_of(map: &HashMap<String, C5DataValue>, key_path: &str) -> Result<Vec<String>, ProviderSchemaError> {
  let named = |key: &str| map.get(key).is_some();
  match (named("path"), named("paths")) {
    (true, true) => Err(ProviderSchemaError::PathAndPaths {
      key_path: key_path.to_owned(),
    }),
    (true, false) => Ok(vec![string_at(map, "path", key_path)?]),
    (false, true) => match map.get("paths") {
      Some(C5DataValue::Array(entries)) if entries.is_empty() => Err(ProviderSchemaError::PathsEmpty {
        key_path: key_path.to_owned(),
      }),
      Some(C5DataValue::Array(entries)) => entries
        .iter()
        .map(|entry| match entry {
          C5DataValue::String(path) => Ok(path.clone()),
          _ => Err(ProviderSchemaError::PathsNotStrings {
            key_path: key_path.to_owned(),
          }),
        })
        .collect(),
      _ => Err(ProviderSchemaError::PathsNotStrings {
        key_path: key_path.to_owned(),
      }),
    },
    (false, false) => Err(ProviderSchemaError::NoPath {
      key_path: key_path.to_owned(),
    }),
  }
}

pub struct C5FileValueProviderSchema {
  pub value_schema: C5ValueProviderSchema,
  /// Read in order, a later file's keys landing over an earlier one's. A
  /// section writing `path` has one entry here.
  pub paths: Vec<String>,
  pub encoding: String,
  pub format: String,
}

impl C5FileValueProviderSchema {
  pub fn new_raw_utf8(value_schema: C5ValueProviderSchema, path: &str) -> C5FileValueProviderSchema {
    return C5FileValueProviderSchema {
      value_schema,
      paths: vec![path.to_string()],
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
    };
  }

  pub fn default(base_path: &str) -> C5FileValueProvider {
    let mut provider = C5FileValueProvider::new(base_path);

    provider.register_deserializer("json", deserialize_json);
    provider.register_deserializer("yaml", deserialize_yaml);

    return provider;
  }

  pub fn register_deserializer<Deserializer>(&mut self, format_name: &str, deserializer: Deserializer)
  where
    Deserializer:
      'static + Fn(C5RawValue) -> Result<C5DataValue, crate::serialization::SerializationError> + Send + Sync,
  {
    self
      ._deserializer
      .insert(format_name.to_string(), Box::from(deserializer));
  }

  /// The file an entry names, resolved against the base directory. A file a
  /// section named and does not have is a deployment error rather than an empty
  /// section. `hydrate` runs at registration, so every failure here ends the
  /// process at boot. Each one names the section and the path, since a store is
  /// assembled from several files and several provider sections and the operator
  /// reading the message cannot otherwise tell which one is wrong.
  fn resolve(&self, key_path: &str, path: &str) -> PathBuf {
    let named = Path::new(path);
    let joined = if named.is_absolute() {
      named.to_path_buf()
    } else {
      Path::new(&self._base_dir_path).join(named)
    };
    if !joined.exists() {
      panic!("[PROVIDER] `{key_path}` names `{path}`, which does not exist at {}", joined.display());
    }
    joined.canonicalize().unwrap_or_else(|e| {
      panic!(
        "[PROVIDER] `{key_path}` names `{path}`, which cannot be resolved at {}: {e}",
        joined.display()
      )
    })
  }

  fn schema_of(map: &HashMap<String, C5DataValue>) -> Result<C5FileValueProviderSchema, ProviderSchemaError> {
    let value_schema = C5ValueProviderSchema::from_map(map)?;
    let key_path = value_schema.value_key_path.clone();
    let optional = |key: &'static str, default: &str| match map.get(key) {
      Some(C5DataValue::String(value)) => Ok(value.clone()),
      Some(_) => Err(ProviderSchemaError::NotAString {
        key_path: key_path.clone(),
        key,
      }),
      None => Ok(default.to_owned()),
    };
    Ok(C5FileValueProviderSchema {
      paths: paths_of(map, &value_schema.value_key_path)?,
      encoding: optional("encoding", "utf8")?,
      format: optional("format", "raw")?,
      value_schema,
    })
  }
}

impl C5ValueProvider for C5FileValueProvider {
  fn register(&mut self, data: &C5DataValue) {
    let C5DataValue::Map(map) = data else {
      return;
    };
    match Self::schema_of(map) {
      Ok(vp_data) => {
        self
          ._key_data_map
          .insert(vp_data.value_schema.value_key_path.clone(), vp_data);
      }
      Err(e) => log::error!("[PROVIDER] {}", e),
    }
  }

  fn unregister(&mut self, key: &str) {
    self._key_data_map.remove(key);
  }

  fn hydrate(&self, set_data_fn: &SetDataFn, _force: bool, context: &HydrateContext) {
    for (key_path, vp_schema) in self._key_data_map.iter() {
      for path in &vp_schema.paths {
        let file_path = self.resolve(key_path, path);

        let file_bytes = fs::read(&file_path).unwrap_or_else(|e| {
          panic!("[PROVIDER] `{key_path}` cannot read {}: {e}", file_path.display());
        });
        let deserialized_value: C5DataValue;

        if &*vp_schema.format != "raw" {
          if !self._deserializer.contains_key(&*vp_schema.format) {
            context.logger.warn(
              format!(
                "{} cannot be deserialized since deserializer {} does not exist",
                vp_schema.value_schema.value_key_path, vp_schema.format
              )
              .as_str(),
            );
            continue;
          }

          let deserializer = self._deserializer.get(&vp_schema.format).unwrap();
          let raw_value = C5RawValue::Bytes(file_bytes);
          match deserializer(raw_value) {
            Ok(value) => {
              deserialized_value = value;
            }
            Err(e) => {
              context.logger.error(
                &format!(
                  "Failed to deserialize file '{}' for key '{}': {}",
                  file_path.display(),
                  key_path,
                  e
                ),
                None,
              );
              continue;
            }
          };
        } else {
          deserialized_value = C5DataValue::Bytes(file_bytes);
        }

        log::trace!("[PROVIDER] Hydrating key '{}' with C5DataValue: {:?}", key_path, &deserialized_value);
        HydrateContext::push_value_to_data_store(set_data_fn, key_path, deserialized_value);
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;

  use super::{
    C5FileValueProvider as Provider, C5ValueProvider, CONFIG_KEY_KEYNAME, CONFIG_KEY_KEYPATH, CONFIG_KEY_PROVIDER,
    ProviderSchemaError, paths_of,
  };
  use crate::{
    C5Store, C5StoreMgr, create_c5store, default_config_paths, providers::C5FileValueProvider, value::C5DataValue,
  };

  /// A section carrying the three keys the loader injects, plus whatever the
  /// case under test adds.
  fn section(extra: &[(&str, C5DataValue)]) -> HashMap<String, C5DataValue> {
    let mut map = HashMap::new();
    map.insert(CONFIG_KEY_PROVIDER.to_owned(), C5DataValue::String("file".to_owned()));
    map.insert(CONFIG_KEY_KEYPATH.to_owned(), C5DataValue::String("fsr".to_owned()));
    map.insert(CONFIG_KEY_KEYNAME.to_owned(), C5DataValue::String("fsr".to_owned()));
    for (key, value) in extra {
      map.insert((*key).to_owned(), value.clone());
    }
    map
  }

  fn strings(values: &[&str]) -> C5DataValue {
    C5DataValue::Array(values.iter().map(|v| C5DataValue::String((*v).to_owned())).collect())
  }

  #[test]
  fn path_is_one_entry_and_paths_is_the_list_in_order() {
    let one = section(&[("path", C5DataValue::String("app.toml".to_owned()))]);
    assert_eq!(paths_of(&one, "fsr").unwrap(), vec!["app.toml".to_owned()]);

    let many = section(&[("paths", strings(&["app.toml", "lab.toml"]))]);
    assert_eq!(
      paths_of(&many, "fsr").unwrap(),
      vec!["app.toml".to_owned(), "lab.toml".to_owned()],
      "order is the list's, so a later file's keys land over an earlier one's"
    );
  }

  #[test]
  fn path_and_paths_together_are_refused_and_the_message_names_the_merge() {
    let both = section(&[
      ("path", C5DataValue::String("app.toml".to_owned())),
      ("paths", strings(&["app.toml", "lab.toml"])),
    ]);
    let message = match paths_of(&both, "fsr") {
      Err(e @ ProviderSchemaError::PathAndPaths { .. }) => e.to_string(),
      other => panic!("both keys must be refused, got {other:?}"),
    };
    assert!(message.contains("merge across config files"), "{message}");
    assert!(
      message.contains("base section must use `paths`"),
      "the collision is not in the file being edited, so the message has to say where to fix it: {message}"
    );
  }

  #[test]
  fn a_section_naming_no_file_or_an_unusable_paths_is_refused() {
    assert!(matches!(
      paths_of(&section(&[]), "fsr"),
      Err(ProviderSchemaError::NoPath { .. })
    ));
    assert!(matches!(
      paths_of(&section(&[("paths", C5DataValue::Array(vec![]))]), "fsr"),
      Err(ProviderSchemaError::PathsEmpty { .. })
    ));
    assert!(matches!(
      paths_of(&section(&[("paths", C5DataValue::Array(vec![C5DataValue::Integer(1)]))]), "fsr"),
      Err(ProviderSchemaError::PathsNotStrings { .. })
    ));
    assert!(
      matches!(
        paths_of(&section(&[("paths", C5DataValue::String("app.toml".to_owned()))]), "fsr"),
        Err(ProviderSchemaError::PathsNotStrings { .. })
      ),
      "a bare string under `paths` is a mistake rather than a one-element list"
    );
  }

  /// A provider whose section names a file that is not there, hydrated. Both
  /// spellings of the path take the same route, so one helper drives both cases.
  fn hydrate_missing(path: &str) {
    let mut provider = Provider::default("resources");
    provider.register(&C5DataValue::Map(section(&[(
      "path",
      C5DataValue::String(path.to_owned()),
    )])));
    let set_data_fn: Box<super::SetDataFn> = Box::new(|_, _| {});
    provider.hydrate(
      &*set_data_fn,
      false,
      &crate::HydrateContext {
        logger: std::sync::Arc::new(crate::ConsoleLogger {}),
      },
    );
  }

  #[test]
  #[should_panic(expected = "`fsr` names `nowhere.json`")]
  fn a_relative_path_that_is_not_there_ends_the_boot_and_names_the_section() {
    hydrate_missing("nowhere.json");
  }

  #[test]
  #[should_panic(expected = "`fsr` names `/nowhere/at/all.json`")]
  fn an_absolute_path_that_is_not_there_ends_the_boot_the_same_way() {
    hydrate_missing("/nowhere/at/all.json");
  }

  #[test]
  fn a_refused_section_names_itself_rather_than_panicking() {
    let e = match Provider::schema_of(&section(&[("format", C5DataValue::Integer(1))])) {
      Err(e) => e,
      Ok(_) => panic!("a format that is not a string must be refused"),
    };
    assert!(e.to_string().contains("fsr"), "{e}");
  }

  #[test]
  fn test_config_contains_example_junk() {
    let (c5store, mut c5store_mgr) = _create_c5store();

    let file_path = "resources";
    c5store_mgr.set_value_provider("resources", C5FileValueProvider::default(file_path), 3);

    assert_eq!(
      c5store.get("example.junk.some").unwrap(),
      C5DataValue::String(String::from("data"))
    );
    assert_eq!(
      c5store.get("example.junk.very").unwrap(),
      C5DataValue::String(String::from("doge"))
    );
  }

  #[test]
  fn a_paths_section_reads_every_file_with_the_last_winning() {
    let (c5store, mut c5store_mgr) = _create_c5store();
    c5store_mgr.set_value_provider("resources", C5FileValueProvider::default("resources"), 0);

    assert_eq!(
      c5store.get("example.layered.some").unwrap(),
      C5DataValue::String(String::from("data")),
      "a key only the first file holds survives"
    );
    assert_eq!(
      c5store.get("example.layered.very").unwrap(),
      C5DataValue::String(String::from("shiba")),
      "a key both hold takes the later file's value"
    );
    assert_eq!(
      c5store.get("example.layered.extra").unwrap(),
      C5DataValue::String(String::from("added")),
      "and a key only the later file holds arrives"
    );
  }

  fn _create_c5store() -> (impl C5Store, C5StoreMgr) {
    let config_file_paths = default_config_paths("configs/test/config", "development", "local", "private");

    return create_c5store(config_file_paths, None).expect("Test store creation failed");
  }
}
