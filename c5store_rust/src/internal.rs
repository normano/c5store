use std::cmp::Ordering;
use std::collections::{Bound, HashMap};
use std::hash::Hash;
use std::sync::Arc;

use maplit::hashmap;
use multimap::MultiMap;
use parking_lot::{RwLock, RwLockReadGuard, RwLockUpgradableReadGuard};
#[cfg(feature="secrets")]
use sha2::{Digest, Sha256};
use skiplist::SkipMap;

use crate::config_source::ConfigSource;
use crate::core::nat_lex_sort::NatLexOrderedString;
use crate::{core, ChangeListener, DetailedChangeListener};
use crate::secrets::SecretKeyStore;
use crate::telemetry::{Logger, StatsRecorder, TagValue};
use crate::value::C5DataValue;

pub struct C5StoreDataValueRef<'a> {
  pub (in self) _lock: RwLockReadGuard<'a, SkipMap<NatLexOrderedString, (C5DataValue, ConfigSource)>>,
  pub (in self) _natural_key_path: NatLexOrderedString,
}

impl <'a> C5StoreDataValueRef<'a> {

  pub fn value(&'a self) -> Option<&'a C5DataValue> {
    // Extract value from tuple
    self._lock.get(&self._natural_key_path).map(|(value, _source)| value)
  }

  pub fn source(&'a self) -> Option<&'a ConfigSource> {
    // Extract source from tuple
    self._lock.get(&self._natural_key_path).map(|(_value, source)| source)
  }
}

#[derive(Clone)]
pub (in crate) struct C5DataStore {
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
    return C5DataStore{
      _logger: logger,
      _stats_recorder: stats_recorder,
      _secret_key_path_segment: format!(".{}", secret_key_path_segment),
      _secret_key_store: secret_key_store,
      _value_hash_cache: Arc::new(RwLock::new(HashMap::new())),
      _data: Arc::new(RwLock::new(SkipMap::new())),
    }
  }

  // Gets, if exists, cloned value from config
  pub fn get_data(&self, key: &str) -> Option<C5DataValue> {

    self._stats_recorder.record_counter_increment(
      hashmap!{
        "group".to_string() => TagValue::String("c5store".to_string()),
      },
      "get_attempts".to_string()
    );
    let natural_key_path = NatLexOrderedString::from(key);
    let rwlock = self._data.read();

    return rwlock.get(&natural_key_path).map(|(value, _source)| value.clone());
  }

  // Gets, if exist, a reference context to value.
  // This exists if there are memory use concerns around calling get_data
  pub fn get_data_ref(&self, key: &str) -> Option<C5StoreDataValueRef> {

    self._stats_recorder.record_counter_increment(
      hashmap!{
        "group".to_string() => TagValue::String("c5store".to_string()),
      },
      "get_attempts".to_string()
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
  pub(in crate) fn _set_data_internal(
    &self,
    key: &str,
    value: C5DataValue,
    source: ConfigSource,
  ) -> Option<C5DataValue> {
    self._stats_recorder.record_counter_increment(
      hashmap!{
        "group".to_string() => TagValue::String("c5store".to_string()),
      },
      "set_attempts".to_string()
    );

    // Handle secrets conditionally
    #[cfg(feature = "secrets")]
    if key.ends_with(&*self._secret_key_path_segment) {
      let decrypted_val_result = self._get_secret(key, &value);

      if let Some(decrypted_val) = decrypted_val_result {
        let data_path = Box::from(&key[..(key.len() - self._secret_key_path_segment.len())]);
        // Store decrypted bytes with the original source info
        return self._data.write().insert(
          NatLexOrderedString::from(data_path),
          (C5DataValue::Bytes(decrypted_val), source) // Use provided source
        ).map(|(old_value, _old_source)| old_value);
      } else {
         // Decryption failed or skipped (e.g., cached), don't store
         // Logged inside _get_secret
         return None;
      }
    }

    // Default behavior (not a secret or secrets disabled)
    // Store the value and source tuple
    return self._data.write().insert(
      NatLexOrderedString::from(key),
      (value, source) // Use provided source
    ).map(|(old_value, _old_source)| old_value);
  }

  // Public method to get source info
  pub fn get_source_info(&self, key: &str) -> Option<ConfigSource> {
    let natural_key_path = NatLexOrderedString::from(key);
    let rwlock = self._data.read();
    // Extract source info from tuple and clone it
    rwlock.get(&natural_key_path).map(|(_value, source)| source.clone())
  }
  
  #[cfg(feature = "secrets")]
  fn _get_secret(&self, key_path: &str, value: &C5DataValue) -> Option<Vec<u8>> {

    let data_result = value.clone().try_into();
    if data_result.is_err() {

      self._logger.warn(format!("Key Path `{}` data is invalid", key_path).as_str());
      return None;
    }

    let data: Vec<C5DataValue> = data_result.unwrap();

    if data.len() != 3 {
      self._logger.warn(format!("Key path `{}` does not have the required number of arguments", key_path).as_str());
      return None;
    }

    let algo_value = data[0].clone().try_into();
    if algo_value.is_err() {

      self._logger.warn(format!("Key Path `{}` algo is invalid", key_path).as_str());
      return None;
    }

    let secret_key_name_value = data[1].clone().try_into();
    if secret_key_name_value.is_err() {
      self._logger.warn(format!("Key Path `{}` secret key name is invalid", key_path).as_str());
      return None;
    }

    let encoded_data_value = data[2].clone().try_into();
    if encoded_data_value.is_err() {
      self._logger.warn(format!("Key Path `{}` encoded data is invalid", key_path).as_str());
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

      RwLockUpgradableReadGuard::upgrade(value_hash_cache_rlock).insert(key_path.to_string(), hash_value);
    }

    self._stats_recorder.record_counter_increment(
      hashmap!{
        "group".to_string() => TagValue::String("c5store".to_string()),
      },
      "set_secret_attempts".to_string()
    );

    let decryptor_opt = self._secret_key_store.get_decryptor(&algo);
    let key_opt = self._secret_key_store.get_key(&secret_key_name);

    if decryptor_opt.is_none() || key_opt.is_none() {

      self._logger.warn(format!("Key Path `{}` decryptor or key is not loaded", key_path).as_str());
      return None;
    }

    let decryptor = decryptor_opt.unwrap();
    let key = key_opt.unwrap();
    
    let encoded_data_bytes = encoded_data.as_bytes().to_vec();
    let decrypted_val_result = decryptor.decrypt(&encoded_data_bytes, &key);

    if decrypted_val_result.is_err() {

      self._logger.warn(format!("Key Path `{}` could not decrypt due to error {:?}", key_path, decrypted_val_result.unwrap_err()).as_str());
      return None;
    }

    return Some(decrypted_val_result.unwrap());
  }

  /// Check if the exact key exists
  pub fn exists(&self, key: &str) -> bool {

    self._stats_recorder.record_counter_increment(
      hashmap!{
        "group".to_string() => TagValue::String("c5store".to_string()),
      },
      "exists_attempts".to_string()
    );
    
    return self._data.read().contains_key(&NatLexOrderedString::from(key));
  }

  /// Checks if the key's prefix exist
  pub fn prefix_key_exists(&self, key: &str) -> bool {
    
    self._stats_recorder.record_counter_increment(
      hashmap!{
        "group".to_string() => TagValue::String("c5store".to_string()),
      },
      "prefix_key_exists_attempts".to_string()
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
      if found_key.0.starts_with(key) { // Handles exact match case again, and prefix case like "a.b" matching "a.b.c"
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
      None => {
        self._data.read().iter().map(|entry| (entry.0).0.clone()).collect()
      },
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
      },
    }
  }
}

#[derive(Clone)]
pub (in crate) struct C5StoreSubscriptions {
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

    self._simple_listeners.write().insert(key_path.to_string(), listener);
  }

  pub fn add_detailed(&self, key_path: &str, listener: Box<DetailedChangeListener>) {
    self._detailed_listeners.write().insert(key_path.to_string(), listener);
  }

  pub fn notify_value_change(&self, notify_key_path: &str, changed_key_path: &str, new_value: &C5DataValue,
    old_value: Option<&C5DataValue>,) {

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

#[cfg(feature="secrets")]
fn _calc_hash_value(algo: &String, secret_key_name: &String, encoded_data: &String,) -> Option<Vec<u8>> {
  
  let mut hasher = Sha256::new();
  hasher.update(algo.as_bytes());
  hasher.update(secret_key_name.as_bytes());
  hasher.update(encoded_data.as_bytes());

  return Some(hasher.finalize().to_vec());
}