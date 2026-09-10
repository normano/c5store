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

/// The only variables a `paths` entry may name; the process environment is deliberately out of reach.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LadderVars {
  pub release_env: String,
  pub env: String,
  pub region: String,
}

impl LadderVars {
  fn get(&self, name: &str) -> Option<&str> {
    match name.to_lowercase().as_str() {
      "release_env" => Some(&self.release_env),
      "env" => Some(&self.env),
      "region" => Some(&self.region),
      _ => None,
    }
  }
}

/// One file a section names. Missing, a templated entry is skipped and a literal one ends the boot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathEntry {
  pub path: String,
  pub templated: bool,
}

/// Why a provider section could not be read.
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
  #[error("`{key_path}` names `${{{variable}}}`, which is not one of release_env, env or region")]
  UnknownVariable { key_path: String, variable: String },
  #[error("`{key_path}` names a variable, but the provider was given none")]
  NoVariables { key_path: String },
  #[error(
    "`{key_path}` resolves `${{{variable}}}` to `{value}`, which is not a single path segment; a rung names a file beside the others, never a path to one"
  )]
  NotOnePathSegment {
    key_path: String,
    variable: String,
    value: String,
  },
}

/// Rejects `..` and separators so a substituted variable cannot leave the config directory.
fn one_path_segment(value: &str) -> bool {
  !value.is_empty() && value != "." && value != ".." && !value.contains('/') && !value.contains('\\')
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

/// Entries in read order, later files winning. `path` and `paths` are exclusive:
/// both merge across the ladder, so mixing them makes the order depend on which files mention the section.
fn paths_of(
  map: &HashMap<String, C5DataValue>,
  key_path: &str,
  vars: Option<&LadderVars>,
) -> Result<Vec<PathEntry>, ProviderSchemaError> {
  let named = |key: &str| map.get(key).is_some();
  let written = match (named("path"), named("paths")) {
    (true, true) => {
      return Err(ProviderSchemaError::PathAndPaths {
        key_path: key_path.to_owned(),
      });
    }
    (true, false) => vec![string_at(map, "path", key_path)?],
    (false, true) => match map.get("paths") {
      Some(C5DataValue::Array(entries)) if entries.is_empty() => {
        return Err(ProviderSchemaError::PathsEmpty {
          key_path: key_path.to_owned(),
        });
      }
      Some(C5DataValue::Array(entries)) => entries
        .iter()
        .map(|entry| match entry {
          C5DataValue::String(path) => Ok(path.clone()),
          _ => Err(ProviderSchemaError::PathsNotStrings {
            key_path: key_path.to_owned(),
          }),
        })
        .collect::<Result<Vec<String>, _>>()?,
      _ => {
        return Err(ProviderSchemaError::PathsNotStrings {
          key_path: key_path.to_owned(),
        });
      }
    },
    (false, false) => {
      return Err(ProviderSchemaError::NoPath {
        key_path: key_path.to_owned(),
      });
    }
  };
  written.iter().map(|path| expanded(path, key_path, vars)).collect()
}

fn expanded(path: &str, key_path: &str, vars: Option<&LadderVars>) -> Result<PathEntry, ProviderSchemaError> {
  if !path.contains('$') {
    return Ok(PathEntry {
      path: path.to_owned(),
      templated: false,
    });
  }
  let Some(vars) = vars else {
    return Err(ProviderSchemaError::NoVariables {
      key_path: key_path.to_owned(),
    });
  };
  let resolved = shellexpand::env_with_context(path, |name: &str| match vars.get(name) {
    None => Err(ProviderSchemaError::UnknownVariable {
      key_path: key_path.to_owned(),
      variable: name.to_owned(),
    }),
    Some(value) if !one_path_segment(value) => Err(ProviderSchemaError::NotOnePathSegment {
      key_path: key_path.to_owned(),
      variable: name.to_owned(),
      value: value.to_owned(),
    }),
    Some(value) => Ok(Some(value.to_owned())),
  })
  .map_err(|e| e.cause)?;
  Ok(PathEntry {
    path: resolved.into_owned(),
    templated: true,
  })
}

pub struct C5FileValueProviderSchema {
  pub value_schema: C5ValueProviderSchema,
  /// Read in order, later files winning. `path` yields exactly one.
  pub paths: Vec<PathEntry>,
  pub encoding: String,
  pub format: String,
}

impl C5FileValueProviderSchema {
  pub fn new_raw_utf8(value_schema: C5ValueProviderSchema, path: &str) -> C5FileValueProviderSchema {
    return C5FileValueProviderSchema {
      value_schema,
      paths: vec![PathEntry {
        path: path.to_string(),
        templated: false,
      }],
      encoding: "utf8".to_string(),
      format: "raw".to_string(),
    };
  }
}

pub struct C5FileValueProvider {
  _base_dir_path: String,
  _key_data_map: HashMap<String, C5FileValueProviderSchema>,
  _deserializer: HashMap<String, Box<C5ValueDeserializer>>,
  /// Absent, a templated entry is refused rather than read as a literal.
  _vars: Option<LadderVars>,
}

impl C5FileValueProvider {
  pub fn new(base_path: &str) -> C5FileValueProvider {
    return C5FileValueProvider {
      _base_dir_path: base_path.to_string(),
      _key_data_map: HashMap::new(),
      _deserializer: HashMap::new(),
      _vars: None,
    };
  }

  pub fn default(base_path: &str) -> C5FileValueProvider {
    let mut provider = C5FileValueProvider::new(base_path);

    provider.register_deserializer("json", deserialize_json);
    provider.register_deserializer("yaml", deserialize_yaml);

    return provider;
  }

  /// The values a `paths` entry's variables resolve to.
  pub fn with_vars(mut self, vars: LadderVars) -> C5FileValueProvider {
    self._vars = Some(vars);
    return self;
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

  /// Panics on a missing literal entry or an unresolvable path; `hydrate` runs at registration, so this fails the boot.
  fn resolve(&self, key_path: &str, entry: &PathEntry) -> Option<PathBuf> {
    let path = &entry.path;
    let named = Path::new(path);
    let joined = if named.is_absolute() {
      named.to_path_buf()
    } else {
      Path::new(&self._base_dir_path).join(named)
    };
    if !joined.exists() {
      if entry.templated {
        log::debug!(
          "[PROVIDER] `{key_path}` has no `{path}`, which a resolved entry is allowed to be missing"
        );
        return None;
      }
      panic!("[PROVIDER] `{key_path}` names `{path}`, which does not exist at {}", joined.display());
    }
    Some(joined.canonicalize().unwrap_or_else(|e| {
      panic!(
        "[PROVIDER] `{key_path}` names `{path}`, which cannot be resolved at {}: {e}",
        joined.display()
      )
    }))
  }

  fn schema_of(&self, map: &HashMap<String, C5DataValue>) -> Result<C5FileValueProviderSchema, ProviderSchemaError> {
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
      paths: paths_of(map, &value_schema.value_key_path, self._vars.as_ref())?,
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
    match self.schema_of(map) {
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
      for entry in &vp_schema.paths {
        let Some(file_path) = self.resolve(key_path, entry) else {
          continue;
        };

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
    LadderVars, PathEntry, ProviderSchemaError, paths_of,
  };
  use crate::{
    C5Store, C5StoreMgr, create_c5store, default_config_paths, providers::C5FileValueProvider, value::C5DataValue,
  };

  /// A section with the three loader-injected keys plus `extra`.
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

  fn literal(path: &str) -> PathEntry {
    PathEntry {
      path: path.to_owned(),
      templated: false,
    }
  }

  fn lab() -> LadderVars {
    LadderVars {
      release_env: "lab".to_owned(),
      env: "staging".to_owned(),
      region: "sfo1".to_owned(),
    }
  }

  #[test]
  fn path_is_one_entry_and_paths_is_the_list_in_order() {
    let one = section(&[("path", C5DataValue::String("app.toml".to_owned()))]);
    assert_eq!(paths_of(&one, "fsr", None).unwrap(), vec![literal("app.toml")]);

    let many = section(&[("paths", strings(&["app.toml", "lab.toml"]))]);
    assert_eq!(
      paths_of(&many, "fsr", None).unwrap(),
      vec![literal("app.toml"), literal("lab.toml")],
      "order is the list's, so a later file's keys land over an earlier one's"
    );
  }

  #[test]
  fn path_and_paths_together_are_refused_and_the_message_names_the_merge() {
    let both = section(&[
      ("path", C5DataValue::String("app.toml".to_owned())),
      ("paths", strings(&["app.toml", "lab.toml"])),
    ]);
    let message = match paths_of(&both, "fsr", None) {
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
      paths_of(&section(&[]), "fsr", None),
      Err(ProviderSchemaError::NoPath { .. })
    ));
    assert!(matches!(
      paths_of(&section(&[("paths", C5DataValue::Array(vec![]))]), "fsr", None),
      Err(ProviderSchemaError::PathsEmpty { .. })
    ));
    assert!(matches!(
      paths_of(&section(&[("paths", C5DataValue::Array(vec![C5DataValue::Integer(1)]))]), "fsr", None),
      Err(ProviderSchemaError::PathsNotStrings { .. })
    ));
    assert!(
      matches!(
        paths_of(&section(&[("paths", C5DataValue::String("app.toml".to_owned()))]), "fsr", None),
        Err(ProviderSchemaError::PathsNotStrings { .. })
      ),
      "a bare string under `paths` is a mistake rather than a one-element list"
    );
  }

  /// Hydrates a provider whose section names a missing file.
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
  fn a_template_resolves_against_the_ladder_and_is_marked_a_rung() {
    let map = section(&[("paths", strings(&["app.toml", "${release_env}.toml"]))]);
    assert_eq!(
      paths_of(&map, "fsr", Some(&lab())).unwrap(),
      vec![
        literal("app.toml"),
        PathEntry {
          path: "lab.toml".to_owned(),
          templated: true,
        }
      ],
      "the literal stays required, the resolved one is a rung"
    );

    let all_three = section(&[("paths", strings(&["${env}-${region}.toml"]))]);
    assert_eq!(
      paths_of(&all_three, "fsr", Some(&lab())).unwrap()[0].path,
      "staging-sfo1.toml"
    );
  }

  #[test]
  fn only_the_three_ladder_variables_resolve() {
    let reached_for = section(&[("paths", strings(&["${aws_secret_access_key}.toml"]))]);
    match paths_of(&reached_for, "fsr", Some(&lab())) {
      Err(ProviderSchemaError::UnknownVariable { variable, .. }) => {
        assert_eq!(variable, "aws_secret_access_key", "and the name is not echoed anywhere else")
      }
      other => panic!("a variable outside the closed set must be refused, got {other:?}"),
    }

    let no_vars = section(&[("paths", strings(&["${release_env}.toml"]))]);
    assert!(
      matches!(
        paths_of(&no_vars, "fsr", None),
        Err(ProviderSchemaError::NoVariables { .. })
      ),
      "a template with no ladder is refused rather than read as a literal, so it cannot silently do nothing"
    );
  }

  #[test]
  fn a_variable_that_is_not_one_path_segment_is_refused() {
    for value in ["../../../../etc/passwd", "/etc/passwd", "a/b", "..", ".", ""] {
      let vars = LadderVars {
        release_env: value.to_owned(),
        ..lab()
      };
      let map = section(&[("paths", strings(&["${release_env}.toml"]))]);
      assert!(
        matches!(
          paths_of(&map, "fsr", Some(&vars)),
          Err(ProviderSchemaError::NotOnePathSegment { .. })
        ),
        "`{value}` would let a rung name a file outside the config directory"
      );
    }
  }

  #[test]
  fn a_rung_that_is_not_there_is_skipped_where_a_literal_would_end_the_boot() {
    let mut provider = Provider::default("resources").with_vars(LadderVars {
      release_env: "nosuchenv".to_owned(),
      ..lab()
    });
    provider.register(&C5DataValue::Map(section(&[
      ("paths", strings(&["example.json", "${release_env}.json"])),
      ("format", C5DataValue::String("json".to_owned())),
    ])));
    let seen = std::sync::Arc::new(parking_lot::Mutex::new(Vec::<String>::new()));
    let recorder = seen.clone();
    let set_data_fn: Box<super::SetDataFn> = Box::new(move |key, _| recorder.lock().push(key.to_owned()));
    provider.hydrate(
      &*set_data_fn,
      false,
      &crate::HydrateContext {
        logger: std::sync::Arc::new(crate::ConsoleLogger {}),
      },
    );
    let seen = seen.lock();
    assert!(
      seen.iter().any(|k| k == "fsr.some"),
      "the literal entry was still read: {seen:?}"
    );
  }

  #[test]
  fn a_refused_section_names_itself_rather_than_panicking() {
    let e = match Provider::default("resources").schema_of(&section(&[("format", C5DataValue::Integer(1))])) {
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
