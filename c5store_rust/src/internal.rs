use std::cmp::Ordering;
use std::collections::{Bound, HashMap};
use std::hash::Hash;
use std::sync::Arc;

use maplit::hashmap;
use multimap::MultiMap;
use parking_lot::{RwLock, RwLockReadGuard, RwLockUpgradableReadGuard};
use sha2::{Digest, Sha256};
use skiplist::SkipMap;

use crate::ChangeListener;
use crate::secrets::SecretKeyStore;
use crate::telemetry::{Logger, StatsRecorder, TagValue};
use crate::value::C5DataValue;

pub struct C5StoreDataValueRef<'a> {
  pub (in self) _lock: RwLockReadGuard<'a, SkipMap<NaturalOrderedString, C5DataValue>>,
  pub (in self) _natural_key_path: NaturalOrderedString,
}

impl <'a> C5StoreDataValueRef<'a> {

  pub fn value(&'a self) -> Option<&'a C5DataValue> {

    return self._lock.get(&self._natural_key_path);
  }
}

#[derive(Clone)]
pub (in crate) struct C5DataStore {
  _logger: Arc<dyn Logger>,
  _stats_recorder: Arc<dyn StatsRecorder>,
  _secret_key_path_segment: Box<str>,
  _secret_key_store: Arc<SecretKeyStore>,
  _value_hash_cache: Arc<RwLock<HashMap<Box<str>, Vec<u8>>>>,
  _data: Arc<RwLock<SkipMap<NaturalOrderedString, C5DataValue>>>,
}

impl C5DataStore {
  pub fn new(
    logger: Arc<dyn Logger>,
    stats_recorder: Arc<dyn StatsRecorder>,
    secret_key_path_segment: Box<str>,
    secret_key_store: Arc<SecretKeyStore>,
  ) -> C5DataStore {
    return C5DataStore{
      _logger: logger,
      _stats_recorder: stats_recorder,
      _secret_key_path_segment: Box::from(format!(".{}", secret_key_path_segment).as_str()),
      _secret_key_store: secret_key_store,
      _value_hash_cache: Arc::new(RwLock::new(HashMap::new())),
      _data: Arc::new(RwLock::new(SkipMap::new())),
    }
  }

  // Gets, if exists, cloned value from config
  pub fn get_data(&self, key: &str) -> Option<C5DataValue> {

    self._stats_recorder.record_counter_increment(
      hashmap!{
        Box::from("group") => TagValue::String(Box::from("c5store")),
      },
      Box::from("get_attempts")
    );
    let natural_key_path = NaturalOrderedString::from(key);
    let rwlock = self._data.read();
    let data_option = rwlock.get(&natural_key_path);

    return data_option.map(|value| (*value).clone());
  }

  // Gets, if exist, a reference context to value.
  // This exists if there are memory use concerns around calling get_data
  pub fn get_data_ref(&self, key: &str) -> Option<C5StoreDataValueRef> {

    self._stats_recorder.record_counter_increment(
      hashmap!{
        Box::from("group") => TagValue::String(Box::from("c5store")),
      },
      Box::from("get_attempts")
    );
    let natural_key_path = NaturalOrderedString::from(key);
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

    self._stats_recorder.record_counter_increment(
      hashmap!{
        Box::from("group") => TagValue::String(Box::from("c5store")),
      },
      Box::from("set_attempts")
    );

    if key.ends_with(&*self._secret_key_path_segment) {
    

      let decrypted_val_result = self._get_secret(key, &value);

      if decrypted_val_result.is_none() {
  
        return None;
      }

      let data_path = Box::from(&key[..(key.len() - self._secret_key_path_segment.len())]);

      let decrypted_val = decrypted_val_result.unwrap();

      return self._data.write().insert(NaturalOrderedString::from(data_path), C5DataValue::Bytes(decrypted_val));
    }

    return self._data.write().insert(NaturalOrderedString::from(key), value);
  }

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

      RwLockUpgradableReadGuard::upgrade(value_hash_cache_rlock).insert(Box::from(key_path), hash_value);
    }

    self._stats_recorder.record_counter_increment(
      hashmap!{
        Box::from("group") => TagValue::String(Box::from("c5store")),
      },
      Box::from("set_secret_attempts")
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

  pub fn exists(&self, key: &str) -> bool {

    self._stats_recorder.record_counter_increment(
      hashmap!{
        Box::from("group") => TagValue::String(Box::from("c5store")),
      },
      Box::from("exists_attempts")
    );
    return self._data.read().contains_key(&NaturalOrderedString::from(key));
  }

  pub fn keys_with_prefix(&self, key_path_option: Option<&str>) -> Vec<Box<str>> {

    return match key_path_option {
      None => {
        self._data.read().iter().map(|entry| (entry.0).0.clone()).collect()
      },
      Some(key_path) => {
        let mut result = vec![];

        let prefix_key = (key_path.to_string() + ".").into_boxed_str();
        let natural_key_path = NaturalOrderedString::from(key_path);
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

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
struct NaturalOrderedString(Box<str>);

impl Ord for NaturalOrderedString {
  fn cmp(&self, other: &Self) -> Ordering {
    return natord::compare_ignore_case(&self.0, &other.0);
  }
}

impl PartialOrd for NaturalOrderedString {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    return Some(natord::compare_ignore_case(&self.0, &other.0));
  }
}

impl From<&str> for NaturalOrderedString {
  fn from(value: &str) -> Self {
    return NaturalOrderedString(Box::from(value));
  }
}

impl From<Box<str>> for NaturalOrderedString {
  fn from(value: Box<str>) -> Self {
    return NaturalOrderedString(value);
  }
}

impl Into<Box<str>> for NaturalOrderedString {
  fn into(self) -> Box<str> {
    return self.0;
  }
}

impl From<String> for NaturalOrderedString {
  fn from(value: String) -> Self {
    return NaturalOrderedString(value.into_boxed_str());
  }
}

#[derive(Clone)]
pub (in crate) struct C5StoreSubscriptions {
  _change_listeners: Arc<RwLock<MultiMap<Box<str>, Box<ChangeListener>>>>,
}

impl C5StoreSubscriptions {
  pub fn new() -> C5StoreSubscriptions {
    return C5StoreSubscriptions {
      _change_listeners: Arc::new(RwLock::new(MultiMap::new())),
    };
  }
}

impl C5StoreSubscriptions {
  pub fn add(&self, key_path: &str, listener: Box<ChangeListener>) {

    self._change_listeners.write().insert(Box::from(key_path), listener);
  }

  pub fn with_subscribers<SubscriberFn>(&self, key_path: &str, subscriber_fn: SubscriberFn)
  where SubscriberFn: FnMut(&Box<ChangeListener>)
  {

    let rwlock = self._change_listeners.read();
    let subscribers_option = rwlock.get_vec(key_path);

    if subscribers_option.is_some() {

      subscribers_option.unwrap().iter().for_each(subscriber_fn);
    }
  }

  pub fn notify_value_change(&self, notify_key_path: &str, key_path: &str, value: &C5DataValue) {

    let rwlock = self._change_listeners.read();
    for change_listener in rwlock.get(notify_key_path) {

      change_listener(notify_key_path, key_path, value);
    }
  }
}

fn _calc_hash_value(algo: &String, secret_key_name: &String, encoded_data: &String,) -> Option<Vec<u8>> {
  
  let mut hasher = Sha256::new();
  hasher.update(algo.as_bytes());
  hasher.update(secret_key_name.as_bytes());
  hasher.update(encoded_data.as_bytes());

  return Some(hasher.finalize().to_vec());
}