use std::cmp::Ordering;
use std::collections::{Bound, HashMap};
use std::hash::Hash;
use std::sync::Arc;

use maplit::hashmap;
use multimap::MultiMap;
use parking_lot::{RwLock, RwLockReadGuard, RwLockUpgradableReadGuard};
use serde_json::{json, Value as JsonValue, Map as JsonMap};
#[cfg(feature = "secrets")]
use sha2::{Digest, Sha256};
use skiplist::SkipMap;

use crate::config_source::ConfigSource;
use natlex_sort::NatLexOrderedString;
use crate::error::ConfigError;
use crate::secrets::SecretKeyStore;
use crate::telemetry::{Logger, StatsRecorder, TagValue};
use crate::value::C5DataValue;
use crate::{ChangeListener, DetailedChangeListener};

pub struct C5StoreDataValueRef<'a> {
  pub(self) _lock: RwLockReadGuard<'a, SkipMap<NatLexOrderedString, (C5DataValue, ConfigSource)>>,
  pub(self) _natural_key_path: NatLexOrderedString,
}

impl<'a> C5StoreDataValueRef<'a> {
  pub fn value(&'a self) -> Option<&'a C5DataValue> {
    // Extract value from tuple
    self
      ._lock
      .get(&self._natural_key_path)
      .map(|(value, _source)| value)
  }

  pub fn source(&'a self) -> Option<&'a ConfigSource> {
    // Extract source from tuple
    self
      ._lock
      .get(&self._natural_key_path)
      .map(|(_value, source)| source)
  }
}

#[derive(Clone)]
pub(crate) struct C5DataStore {
  _logger: Arc<dyn Logger>,
  _stats_recorder: Arc<dyn StatsRecorder>,
  _secret_key_path_segment: String,
  _secret_key_store: Arc<SecretKeyStore>,
  _value_hash_cache: Arc<RwLock<HashMap<String, Vec<u8>>>>,
  _data: Arc<RwLock<SkipMap<NatLexOrderedString, (C5DataValue, ConfigSource)>>>,
}

impl C5DataStore {
  pub fn new(
    logger: Arc<dyn Logger>,
    stats_recorder: Arc<dyn StatsRecorder>,
    secret_key_path_segment: String,
    secret_key_store: Arc<SecretKeyStore>,
  ) -> C5DataStore {
    return C5DataStore {
      _logger: logger,
      _stats_recorder: stats_recorder,
      _secret_key_path_segment: format!(".{}", secret_key_path_segment),
      _secret_key_store: secret_key_store,
      _value_hash_cache: Arc::new(RwLock::new(HashMap::new())),
      _data: Arc::new(RwLock::new(SkipMap::new())),
    };
  }

  // Gets, if exists, cloned value from config
  pub fn get_data(&self, key: &str) -> Option<C5DataValue> {
    self._stats_recorder.record_counter_increment(
      hashmap! {
        "group".to_string() => TagValue::String("c5store".to_string()),
      },
      "get_attempts".to_string(),
    );
    let natural_key_path = NatLexOrderedString::from(key);
    let rwlock = self._data.read();

    return rwlock
      .get(&natural_key_path)
      .map(|(value, _source)| value.clone());
  }

  // Gets, if exist, a reference context to value.
  // This exists if there are memory use concerns around calling get_data
  pub fn get_data_ref(&self, key: &str) -> Option<C5StoreDataValueRef> {
    self._stats_recorder.record_counter_increment(
      hashmap! {
        "group".to_string() => TagValue::String("c5store".to_string()),
      },
      "get_attempts".to_string(),
    );
    let natural_key_path = NatLexOrderedString::from(key);
    let rwlock = self._data.read();
    let contains_key = rwlock.contains_key(&natural_key_path);

    if contains_key {
      return Some(C5StoreDataValueRef {
        _lock: rwlock,
        _natural_key_path: natural_key_path,
      });
    }

    return None;
  }

  pub fn set_data(&self, key: &str, value: C5DataValue) -> Option<C5DataValue> {
    let source = ConfigSource::Provider("UnknownProvider".to_string()); // Or SetProgrammatically/Unknown
    self._set_data_internal(key, value, source)
  }

  // Called by read_config_data and potentially the public set_data
  pub(crate) fn _set_data_internal(
    &self,
    key: &str,
    value: C5DataValue,
    source: ConfigSource,
  ) -> Option<C5DataValue> {
    self._stats_recorder.record_counter_increment(
      hashmap! {
        "group".to_string() => TagValue::String("c5store".to_string()),
      },
      "set_attempts".to_string(),
    );

    // Handle secrets conditionally
    #[cfg(feature = "secrets")]
    if key.ends_with(&*self._secret_key_path_segment) {
      let decrypted_val_result = self._get_secret(key, &value);

      if let Some(decrypted_val) = decrypted_val_result {
        let data_path = Box::from(&key[..(key.len() - self._secret_key_path_segment.len())]);
        // Store decrypted bytes with the original source info
        return self
          ._data
          .write()
          .insert(
            NatLexOrderedString::from(data_path),
            (C5DataValue::Bytes(decrypted_val), source), // Use provided source
          )
          .map(|(old_value, _old_source)| old_value);
      } else {
        // Decryption failed or skipped (e.g., cached), don't store
        // Logged inside _get_secret
        return None;
      }
    }

    // Default behavior (not a secret or secrets disabled)
    // Store the value and source tuple
    return self
      ._data
      .write()
      .insert(
        NatLexOrderedString::from(key),
        (value, source), // Use provided source
      )
      .map(|(old_value, _old_source)| old_value);
  }

  // Public method to get source info
  pub fn get_source_info(&self, key: &str) -> Option<ConfigSource> {
    let natural_key_path = NatLexOrderedString::from(key);
    let rwlock = self._data.read();
    // Extract source info from tuple and clone it
    rwlock
      .get(&natural_key_path)
      .map(|(_value, source)| source.clone())
  }

  #[cfg(feature = "secrets")]
  fn _get_secret(&self, key_path: &str, value: &C5DataValue) -> Option<Vec<u8>> {
    let data_result = value.clone().try_into();
    if data_result.is_err() {
      self
        ._logger
        .warn(format!("Key Path `{}` data is invalid", key_path).as_str());
      return None;
    }

    let data: Vec<C5DataValue> = data_result.unwrap();

    if data.len() != 3 {
      self._logger.warn(
        format!(
          "Key path `{}` does not have the required number of arguments",
          key_path
        )
        .as_str(),
      );
      return None;
    }

    let algo_value = data[0].clone().try_into();
    if algo_value.is_err() {
      self
        ._logger
        .warn(format!("Key Path `{}` algo is invalid", key_path).as_str());
      return None;
    }

    let secret_key_name_value = data[1].clone().try_into();
    if secret_key_name_value.is_err() {
      self
        ._logger
        .warn(format!("Key Path `{}` secret key name is invalid", key_path).as_str());
      return None;
    }

    let encoded_data_value = data[2].clone().try_into();
    if encoded_data_value.is_err() {
      self
        ._logger
        .warn(format!("Key Path `{}` encoded data is invalid", key_path).as_str());
      return None;
    }

    let algo: String = algo_value.unwrap();
    let secret_key_name: String = secret_key_name_value.unwrap();
    let encoded_data: String = encoded_data_value.unwrap();

    let hash_value = _calc_hash_value(&algo, &secret_key_name, &encoded_data)?;

    let value_hash_cache_rlock = self._value_hash_cache.upgradable_read();
    if value_hash_cache_rlock.contains_key(key_path) {
      let existing_hash_value = value_hash_cache_rlock.get(key_path).unwrap();

      if existing_hash_value == &hash_value {
        return None;
      }
    } else {
      RwLockUpgradableReadGuard::upgrade(value_hash_cache_rlock)
        .insert(key_path.to_string(), hash_value);
    }

    self._stats_recorder.record_counter_increment(
      hashmap! {
        "group".to_string() => TagValue::String("c5store".to_string()),
      },
      "set_secret_attempts".to_string(),
    );

    let decryptor_opt = self._secret_key_store.get_decryptor(&algo);
    let key_opt = self._secret_key_store.get_key(&secret_key_name);

    if decryptor_opt.is_none() || key_opt.is_none() {
      self
        ._logger
        .warn(format!("Key Path `{}` decryptor or key is not loaded", key_path).as_str());
      return None;
    }

    let decryptor = decryptor_opt.unwrap();
    let key = key_opt.unwrap();

    let encoded_data_bytes = encoded_data.as_bytes().to_vec();
    let decrypted_val_result = decryptor.decrypt(&encoded_data_bytes, &key);

    if decrypted_val_result.is_err() {
      self._logger.warn(
        format!(
          "Key Path `{}` could not decrypt due to error {:?}",
          key_path,
          decrypted_val_result.unwrap_err()
        )
        .as_str(),
      );
      return None;
    }

    return Some(decrypted_val_result.unwrap());
  }

  /// Check if the exact key exists
  pub fn exists(&self, key: &str) -> bool {
    self._stats_recorder.record_counter_increment(
      hashmap! {
        "group".to_string() => TagValue::String("c5store".to_string()),
      },
      "exists_attempts".to_string(),
    );

    return self
      ._data
      .read()
      .contains_key(&NatLexOrderedString::from(key));
  }

  /// Checks if the key's prefix exist
  pub fn prefix_key_exists(&self, key: &str) -> bool {
    self._stats_recorder.record_counter_increment(
      hashmap! {
        "group".to_string() => TagValue::String("c5store".to_string()),
      },
      "prefix_key_exists_attempts".to_string(),
    );

    if self.exists(key) {
      return true;
    }

    let natural_key_path = NatLexOrderedString::from(key);
    let rwlock = self._data.read();

    // Check if any key in the map starts with the prefix + "."
    // Use range scan for efficiency with SkipMap
    let prefix_dot = key.to_string() + ".";
    let mut range = rwlock.range(Bound::Included(&natural_key_path), Bound::Unbounded);

    // Check the first element greater than or equal to the key itself
    if let Some((found_key, _)) = range.next() {
      // If the found key starts with the original key OR the key + ".", it's a prefix match
      if found_key.0.starts_with(key) {
        // Handles exact match case again, and prefix case like "a.b" matching "a.b.c"
        // Check if it actually starts with prefix + dot if not an exact match
        if found_key.0 != key && found_key.0.starts_with(&prefix_dot) {
          return true;
        }
        // If found_key == key, it's an exact match, handled by self.exists earlier.
        // If it starts with key but not key + ".", like "abc" matching "abcdef", we don't count it as prefix for *path* exists.
      }
    }

    return false;
  }

  pub fn keys_with_prefix(&self, key_path_option: Option<&str>) -> Vec<String> {
    return match key_path_option {
      None => self
        ._data
        .read()
        .iter()
        .map(|entry| (entry.0).0.clone())
        .collect(),
      Some(key_path) => {
        let mut result = vec![];

        let prefix_key = key_path.to_string() + ".";
        let natural_key_path = NatLexOrderedString::from(key_path);
        let rwlock = self._data.read();
        let range = rwlock.range(Bound::Included(&natural_key_path), Bound::Unbounded);

        for entry in range {
          if !(entry.0).0.starts_with(&*prefix_key) {
            break;
          }

          result.push((entry.0).0.clone());
        }

        result
      }
    };
  }

  /// Fetches all configuration entries under a given prefix and reconstructs
  /// them into a hierarchical `serde_json::Value`.
  ///
  /// Treats numeric path segments (e.g., "0", "1") as array indices where possible,
  /// otherwise treats segments as object keys.
  ///
  /// # Arguments
  /// * `prefix` - The key path prefix. If empty, fetches all entries.
  ///
  /// # Returns
  /// A `Result` containing the reconstructed `serde_json::Value` or a `ConfigError`.
  /// Returns `Ok(JsonValue::Null)` if the prefix exists but has no children,
  /// or if the prefix itself doesn't exist.
  pub(crate) fn fetch_children_with_prefix(&self, prefix: &str) -> Result<JsonValue, ConfigError> {
    self._stats_recorder.record_counter_increment(
      hashmap! {
          "group".to_string() => TagValue::String("c5store".to_string()),
      },
      "fetch_children_attempts".to_string(),
    );

    let data_lock = self._data.read();
    let mut root_value = JsonValue::Object(JsonMap::new()); // Start with an empty object

    // Determine the actual prefix string to search for and the base length to strip
    let (search_prefix, prefix_len_to_strip) = if prefix.is_empty() {
      (String::new(), 0) // Fetch all, strip nothing
    } else {
      (format!("{}.", prefix), prefix.len() + 1) // Fetch children, strip "prefix."
    };
    let search_prefix_nat_lex = NatLexOrderedString::from(search_prefix.as_str());

    // Define the start bound for the range scan
    let start_bound = if prefix.is_empty() {
      Bound::Unbounded // Start from the beginning if prefix is empty
    } else {
      // Start searching *from* "prefix."
      Bound::Included(&search_prefix_nat_lex)
    };

    let range = data_lock.range(start_bound, Bound::Unbounded);

    let mut found_children = false;
    for (key_nat_lex, (c5_value, _source)) in range {
      let full_key = &key_nat_lex.0;

      // Stop if we go past the prefix (only relevant if prefix is not empty)
      if !prefix.is_empty() && !full_key.starts_with(&search_prefix) {
        break;
      }

      // Calculate the relative path
      let relative_path = &full_key[prefix_len_to_strip..];
      if relative_path.is_empty() {
        // Should not happen if key starts with "prefix."
        continue;
      }

      found_children = true;

      // Convert C5 value to JSON value
      let json_value = crate::value::c5_value_to_serde_json(c5_value.clone()).map_err(|e| {
        ConfigError::Internal(format!(
          "Failed C5->JSON conversion for key '{}': {}",
          full_key, e
        ))
      })?;

      // Split relative path and insert into the root JSON value
      let path_parts: Vec<&str> = relative_path.split('.').collect();
      if let Err(e) = insert_nested_value(&mut root_value, &path_parts, json_value) {
        // Add context to the error
        return Err(ConfigError::Internal(format!(
          "Failed to reconstruct structure for key '{}' at path '{}': {}",
          full_key, relative_path, e
        )));
      }
    }

    println!("fetch children with prefix: {:?}", root_value);
    // If we iterated but found no keys *starting with* the prefix,
    // it means the prefix might exist but has no children, or doesn't exist at all.
    // Returning Null seems reasonable in this case, differentiating it from an error.
    // If root_value is still the initial empty Object, it means no children were added.
    if root_value == JsonValue::Object(JsonMap::new()) && !found_children {
      Ok(JsonValue::Null)
    } else {
      Ok(root_value)
    }
  }
}

#[derive(Clone)]
pub(crate) struct C5StoreSubscriptions {
  _simple_listeners: Arc<RwLock<MultiMap<String, Box<ChangeListener>>>>,
  _detailed_listeners: Arc<RwLock<MultiMap<String, Box<DetailedChangeListener>>>>,
}

impl C5StoreSubscriptions {
  pub fn new() -> C5StoreSubscriptions {
    return C5StoreSubscriptions {
      _simple_listeners: Arc::new(RwLock::new(MultiMap::new())),
      _detailed_listeners: Arc::new(RwLock::new(MultiMap::new())),
    };
  }
}

impl C5StoreSubscriptions {
  pub fn add(&self, key_path: &str, listener: Box<ChangeListener>) {
    self
      ._simple_listeners
      .write()
      .insert(key_path.to_string(), listener);
  }

  pub fn add_detailed(&self, key_path: &str, listener: Box<DetailedChangeListener>) {
    self
      ._detailed_listeners
      .write()
      .insert(key_path.to_string(), listener);
  }

  pub fn notify_value_change(
    &self,
    notify_key_path: &str,
    changed_key_path: &str,
    new_value: &C5DataValue,
    old_value: Option<&C5DataValue>,
  ) {
    // Notify simple listeners (ignore old_value)
    let simple_lock = self._simple_listeners.read();
    if let Some(simple_listeners) = simple_lock.get_vec(notify_key_path) {
      for listener in simple_listeners {
        listener(notify_key_path, changed_key_path, new_value);
      }
    }
    drop(simple_lock); // Release read lock

    // Notify detailed listeners
    let detailed_lock = self._detailed_listeners.read();
    if let Some(detailed_listeners) = detailed_lock.get_vec(notify_key_path) {
      for listener in detailed_listeners {
        listener(notify_key_path, changed_key_path, new_value, old_value);
      }
    }
  }
}

#[cfg(feature = "secrets")]
fn _calc_hash_value(
  algo: &String,
  secret_key_name: &String,
  encoded_data: &String,
) -> Option<Vec<u8>> {
  let mut hasher = Sha256::new();
  hasher.update(algo.as_bytes());
  hasher.update(secret_key_name.as_bytes());
  hasher.update(encoded_data.as_bytes());

  return Some(hasher.finalize().to_vec());
}

/// Helper to insert a value into a nested `serde_json::Value` structure based on path parts.
/// Attempts to create arrays for numeric keys.
fn insert_nested_value<'a>(
  mut node: &'a mut JsonValue,
  path_parts: &[&str],
  value_to_insert: JsonValue,
) -> Result<(), String> {
  // Using String for internal error message simplicity
  for (i, part) in path_parts.iter().enumerate() {
    if part.is_empty() {
      return Err(format!(
        "Encountered empty segment in path: {:?}",
        path_parts
      ));
    }
    let is_last = i == path_parts.len() - 1;

    // Try to parse current part as index for array handling
    let maybe_index: Option<usize> = part.parse().ok();

    if is_last {
      // --- Last part: Insert the final value ---
      match node {
        JsonValue::Object(map) => {
          map.insert(part.to_string(), value_to_insert);
          return Ok(());
        }
        JsonValue::Array(arr) => {
          if let Some(index) = maybe_index {
            if index >= arr.len() {
              arr.resize_with(index + 1, || JsonValue::Null);
            }
            arr[index] = value_to_insert;
            return Ok(());
          } else {
            return Err(format!(
              "Type mismatch: Cannot insert string key '{}' into an existing array.",
              part
            ));
          }
        }
        _ => {
          return Err(format!(
            "Type mismatch: Cannot insert key '{}' into non-container node (found {}).",
            part,
            node.to_string() // Note: node.to_string() might be large
          ));
        }
      }
    } else {
      // --- Intermediate part: Traverse or create the next container ---

      // Determine if the *next* part suggests an array or object
      let next_part_is_index: bool = path_parts
        .get(i + 1)
        .and_then(|p| p.parse::<usize>().ok())
        .is_some();

      // Function to create the default container for the *next* level
      let create_default_container = || {
        if next_part_is_index {
          JsonValue::Array(vec![])
        } else {
          JsonValue::Object(JsonMap::new())
        }
      };

      match node {
        JsonValue::Object(map) => {
          // Get mutable ref to the entry for 'part', creating default if needed
          let entry = map
            .entry(part.to_string())
            .or_insert_with(create_default_container);

          // Type check: Did we just create the right type? Or does the existing type match?
          if (next_part_is_index && !entry.is_array())
            || (!next_part_is_index && !entry.is_object())
          {
            return Err(format!(
              "Type mismatch at key '{}'. Expected {} based on next key '{}', but found {}.",
              part,
              if next_part_is_index {
                "Array"
              } else {
                "Object"
              },
              path_parts[i + 1],
              entry.to_string()
            ));
          }
          node = entry; // Continue traversal
        }
        JsonValue::Array(arr) => {
          if let Some(index) = maybe_index {
            // Ensure array is long enough and potentially create intermediate Nulls
            if index >= arr.len() {
              arr.resize_with(index + 1, || JsonValue::Null);
            }

            // Get mutable ref to element at 'index', creating default container if it's Null
            let element = &mut arr[index];
            if element.is_null() {
              *element = create_default_container();
            }

            // Type check: Does the existing/new element match expectation?
            if (next_part_is_index && !element.is_array())
              || (!next_part_is_index && !element.is_object())
            {
              return Err(format!(
                "Type mismatch at index {}. Expected {} based on next key '{}', but found {}.",
                index,
                if next_part_is_index {
                  "Array"
                } else {
                  "Object"
                },
                path_parts[i + 1],
                element.to_string()
              ));
            }
            node = element; // Continue traversal
          } else {
            return Err(format!(
              "Type mismatch: Cannot traverse using string key '{}' within an existing array.",
              part
            ));
          }
        }
        _ => {
          return Err(format!(
            "Type mismatch: Cannot traverse using key '{}' into non-container node (found {}).",
            part,
            node.to_string()
          ));
        }
      }
    }
  }
  // Should only be reached if path_parts is empty, which we check at the start.
  // If path_parts was empty, we wouldn't enter the loop, so this is unreachable.
  unreachable!("Loop should handle all path parts or error out.");
}
