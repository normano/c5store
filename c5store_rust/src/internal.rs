use std::cmp::Ordering;
use std::collections::{Bound, HashSet};
use std::sync::Arc;

use multimap::MultiMap;
use parking_lot::{RwLock, RwLockReadGuard};
use skiplist::SkipMap;

use crate::ChangeListener;
use crate::value::C5DataValue;
use std::hash::Hash;

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
  _data: Arc<RwLock<SkipMap<NaturalOrderedString, C5DataValue>>>,
}

impl C5DataStore {
  pub fn new() -> C5DataStore {
    return C5DataStore{
      _data: Arc::new(RwLock::new(SkipMap::new())),
    }
  }

  // Gets, if exists, cloned value from config
  pub fn get_data(&self, key: &str) -> Option<C5DataValue> {

    let natural_key_path = NaturalOrderedString::from(key);
    let rwlock = self._data.read();
    let data_option = rwlock.get(&natural_key_path);

    return data_option.map(|value| (*value).clone());
  }

  // Gets, if exist, a reference context to value.
  // This exists if there are memory use concerns around calling get_data
  pub fn get_data_ref(&self, key: &str) -> Option<C5StoreDataValueRef> {

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

    return self._data.write().insert(NaturalOrderedString::from(key), value);
  }

  pub fn exists(&self, key: &str) -> bool {

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