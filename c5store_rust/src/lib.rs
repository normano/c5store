mod data;
mod internal;
pub mod providers;
pub mod secrets;
pub mod serialization;
pub mod telemetry;
pub mod value;
pub mod util;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs::read_dir;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use curve25519_parser::parse_openssl_25519_privkey;
use multimap::MultiMap;
use parking_lot::Mutex;
use scheduled_thread_pool::{JobHandle, ScheduledThreadPool};
use serde_yaml::Value;
use util::build_flat_map;

use crate::data::HashsetMultiMap;
use crate::internal::{C5DataStore, C5StoreDataValueRef, C5StoreSubscriptions};
use crate::providers::{C5ValueProvider, CONFIG_KEY_KEYNAME, CONFIG_KEY_KEYPATH, CONFIG_KEY_PROVIDER};
use crate::secrets::SecretKeyStore;
use crate::serialization::serde_yaml_val_to_c5_value;
use crate::telemetry::{ConsoleLogger, Logger, StatsRecorder, StatsRecorderStub};
use crate::value::C5DataValue;

const DEFAULT_CHANGE_DELAY_PERIOD: u64 = 500;

pub struct HydrateContext {
  pub logger: Arc<dyn Logger>,
}

impl HydrateContext {
  pub fn push_value_to_data_store(set_data_fn: &SetDataFn, key: &str, value: C5DataValue) {

    match value {
      C5DataValue::Map(mut value) => {

        let mut config_data = HashMap::new();
        build_flat_map(&mut value, &mut config_data, String::from(key));

        for config_entry in config_data.into_iter() {

          let config_entry_key = config_entry.0;
          let config_value = config_entry.1;

          set_data_fn(&config_entry_key, config_value);
        }
        return;
      }
      _ => {}
    }
    set_data_fn(key, value);
  }
}

// params: notify key path, key path, value
pub type ChangeListener = dyn Fn(&str, &str, &C5DataValue) -> () + Send + Sync;
pub type SetDataFn = dyn Fn(&str, C5DataValue) + Send + Sync;
pub type SecretKeyStoreConfiguratorFn = dyn FnMut(&mut SecretKeyStore);

pub struct SecretOptions {
  pub secret_key_path_segment: Option<String>,
  pub secret_keys_path: Option<PathBuf>,
  pub secret_key_store_configure_fn: Option<Box<SecretKeyStoreConfiguratorFn>>,
}

impl Default for SecretOptions {
  fn default() -> Self {
    return Self {
      secret_key_path_segment: Some(".c5encval".to_string()),
      secret_keys_path: None,
      secret_key_store_configure_fn: None,
    };
  }
}

pub struct C5StoreOptions {
  pub logger: Option<Arc<dyn Logger>>,
  pub stats: Option<Arc<dyn StatsRecorder>>,
  pub change_delay_period: Option<u64>,
  pub secret_opts: SecretOptions,
}

impl Default for C5StoreOptions {
  fn default() -> Self {
    return Self {
      logger: None,
      stats: None,
      change_delay_period: Some(DEFAULT_CHANGE_DELAY_PERIOD),
      secret_opts: SecretOptions::default(),
    }
  }
}

struct ChangeNotifier {
  debounce_job_handle: Arc<Mutex<RefCell<Option<JobHandle>>>>,
  thread_pool: Arc<ScheduledThreadPool>,
  delay_period: Duration,
  changed_key_paths: Arc<Mutex<RefCell<HashSet<String>>>>,
  _data_store: C5DataStore,
  _subscriptions: C5StoreSubscriptions,
}

impl ChangeNotifier {
  pub fn new(delay_period: Duration, data_store: C5DataStore, subscriptions: C5StoreSubscriptions) -> ChangeNotifier {

    return ChangeNotifier {
      debounce_job_handle: Arc::new(Mutex::new(RefCell::new(None))),
      thread_pool: Arc::new(ScheduledThreadPool::with_name("c5Store_change_notifier", 1)),
      delay_period,
      changed_key_paths: Arc::new(Mutex::new(RefCell::new(HashSet::new()))),
      _data_store: data_store,
      _subscriptions: subscriptions,
    };
  }

  pub fn notify_changed(&self, key: &str) {

    let debounce_job_lock = self.debounce_job_handle.lock();
    let job_handle_borrow = debounce_job_lock.borrow();

    self.changed_key_paths.clone().lock().get_mut().insert(key.to_string());

    if job_handle_borrow.is_none() {

      let debounce_mut = self.debounce_job_handle.clone();
      let saved_changed_keypaths = self.changed_key_paths.clone();
      let datastore = self._data_store.clone();
      let subscriptions = self._subscriptions.clone();

      let job = move || {
        let debounce_job_lock = debounce_mut.lock();
        let mut job_handle_borrow = debounce_job_lock.borrow_mut();
        job_handle_borrow.take();

        let mut deduped_saved_changed_keypath_map: HashsetMultiMap<String, String> = hashsetmultimap!();

        let saved_changed_keypaths_lock = saved_changed_keypaths.lock();
        let saved_changed_keypaths = saved_changed_keypaths_lock.borrow();
        for saved_changed_keypath in saved_changed_keypaths.iter() {

          deduped_saved_changed_keypath_map.insert(
            saved_changed_keypath.clone(),
            saved_changed_keypath.clone()
          );

          let split_saved_changed_keypath = saved_changed_keypath.split(".");
          let mut key_ancestor_path = String::new();

          for saved_changed_keypath_part in split_saved_changed_keypath {

            if !key_ancestor_path.is_empty() {
              key_ancestor_path = key_ancestor_path + ".";
            }

            key_ancestor_path = key_ancestor_path + saved_changed_keypath_part;

            deduped_saved_changed_keypath_map.insert(
              saved_changed_keypath.clone(),
              key_ancestor_path.clone()
            );
          }
        }

        for (saved_changed_keypath, deduped_changed_keypaths) in deduped_saved_changed_keypath_map.iter() {

          let value_ref_cxt_option = datastore.get_data_ref(saved_changed_keypath);

          if let Some(value_ref_cxt) = value_ref_cxt_option {
            for deduped_changed_keypath in deduped_changed_keypaths {
              subscriptions.notify_value_change(
                deduped_changed_keypath,
                saved_changed_keypath,
                value_ref_cxt.value().unwrap(),
              );
            }
          }
        }
      };

      debounce_job_lock.replace(Some(
        self.thread_pool.execute_after(self.delay_period.clone(), job)
      ));
    }
  }
}

pub trait C5Store {
  fn get(&self, key_path: &str) -> Option<C5DataValue>;

  fn get_ref(&self, key_path: &str) -> Option<C5StoreDataValueRef>;

  fn get_into<Val>(&self, key_path: &str) -> Option<Val>
  where C5DataValue: TryInto<Val, Error = ()>;

  fn exists(&self, key_path: &str) -> bool;

  fn path_exists(&self, key: &str) -> bool;

  //
  // Listens to changes to the given keyPath. keyPath can be any the entire path or ancestors.
  // By listening to an ancestor, one will receive one change event even if two children change.
  //
  fn subscribe(&self, key_path: &str, listener: Box<ChangeListener>);

  fn branch(&self, key_path: &str) -> C5StoreBranch;

  //
  // Searches for all keypaths that relative to currentKeyPath + given keyPath
  // @return A list of Key Paths
  //
  fn key_paths_with_prefix(&self, key_path: Option<&str>) -> Vec<String>;

  //
  // @return null if root, prefixKey if branch
  //
  fn current_key_path(&self) -> &str;
}

#[derive(Clone)]
pub struct C5StoreRoot {
  _data_store: C5DataStore,
  _subscriptions: C5StoreSubscriptions,
}

impl C5StoreRoot {
  pub (in crate) fn new(c5data_store: C5DataStore, subscriptions: C5StoreSubscriptions) -> C5StoreRoot {

    return C5StoreRoot {
      _data_store: c5data_store,
      _subscriptions: subscriptions,
    }
  }
}

impl C5Store for C5StoreRoot {

  fn get(&self, key_path: &str) -> Option<C5DataValue> {

    return self._data_store.get_data(key_path);
  }

  fn get_into<Val>(&self, key_path: &str) -> Option<Val>
  where C5DataValue: TryInto<Val, Error = ()> {
    return self._data_store.get_data(key_path).map(|val| val.try_into().unwrap());
  }

  fn get_ref(&self, key_path: &str) -> Option<C5StoreDataValueRef> {

    return self._data_store.get_data_ref(key_path);
  }

  fn exists(&self, key_path: &str) -> bool {

    return self._data_store.exists(key_path);
  }

  fn path_exists(&self, key_path: &str) -> bool {

    return self._data_store.prefix_key_exists(key_path);
  }

  fn subscribe(&self, key_path: &str, listener: Box<ChangeListener>) {

    self._subscriptions.add(key_path, listener);
  }

  fn branch(&self, key_path: &str) -> C5StoreBranch {
    return C5StoreBranch {
      _root: self.clone(),
      _key_path: key_path.to_string(),
    };
  }

  fn key_paths_with_prefix(&self, key_path: Option<&str>) -> Vec<String> {
    return self._data_store.keys_with_prefix(key_path);
  }

  fn current_key_path(&self) -> &str {
    return "";
  }
}

#[derive(Clone)]
pub struct C5StoreBranch {
  _root: C5StoreRoot,
  _key_path: String,
}

impl C5StoreBranch {
  fn _merge_key_path(&self, key_path: &str) -> String {

    return self._key_path.to_string() + "." + key_path;
  }
}

impl C5Store for C5StoreBranch {
  fn get(&self, key_path: &str) -> Option<C5DataValue> {

    return self._root.get(&self._merge_key_path(key_path));
  }

  fn get_into<Val>(&self, key_path: &str) -> Option<Val>
  where C5DataValue: TryInto<Val, Error = ()> {
    return self._root.get(&self._merge_key_path(key_path)).map(|val| val.try_into().unwrap());
  }

  fn get_ref(&self, key_path: &str) -> Option<C5StoreDataValueRef> {

    return self._root.get_ref(&self._merge_key_path(key_path));
  }

  fn exists(&self, key_path: &str) -> bool {

    return self._root.exists(&self._merge_key_path(key_path));
  }

  fn path_exists(&self, key_path: &str) -> bool {

    return self._root.path_exists(&self._merge_key_path(key_path));
  }

  fn subscribe(&self, key_path: &str, listener: Box<ChangeListener>) {
    self._root.subscribe(&self._merge_key_path(key_path), listener);
  }

  fn branch(&self, key_path: &str) -> C5StoreBranch {
    return C5StoreBranch {
      _root: self._root.clone(),
      _key_path: self._merge_key_path(key_path),
    };
  }

  fn key_paths_with_prefix(&self, key_path_option: Option<&str>) -> Vec<String> {

    return match key_path_option {
      Some(key_path) => {

        let merged_key_path = self._merge_key_path(key_path);
        self._root.key_paths_with_prefix(Some(&merged_key_path))
      },
      None => self._root.key_paths_with_prefix(None),
    }
  }

  fn current_key_path(&self) -> &str {
    return &self._key_path;
  }
}

pub struct C5StoreMgr {
  _value_providers: Arc<Mutex<HashMap<String, Box<dyn C5ValueProvider>>>>,
  _scheduled_thread_pool: ScheduledThreadPool,
  _scheduled_provider_job_handles: Vec<JobHandle>,
  _data_store: C5StoreRoot,
  _logger: Arc<dyn Logger>,
  _stats: Arc<dyn StatsRecorder>,
  _change_notifier: Arc<ChangeNotifier>,
  _set_data_fn: Arc<SetDataFn>,
  _provided_data: MultiMap<String, C5DataValue>,
}

impl C5StoreMgr {
  fn new(
    data_store: C5StoreRoot,
    logger: Arc<dyn Logger>,
    stats: Arc<dyn StatsRecorder>,
    change_notifier: Arc<ChangeNotifier>,
    set_data_fn: Arc<SetDataFn>,
    provided_data: MultiMap<String, C5DataValue>,
  ) -> C5StoreMgr {

    return C5StoreMgr {
      _value_providers: Arc::new(Mutex::new(HashMap::new())),
      _scheduled_thread_pool: ScheduledThreadPool::with_name("c5store_mgr", 8),
      _scheduled_provider_job_handles: vec![],
      _data_store: data_store,
      _logger: logger,
      _stats: stats,
      _change_notifier: change_notifier,
      _set_data_fn: set_data_fn,
      _provided_data: provided_data,
    }
  }

  pub fn set_value_provider<ValueProvider>(
    &mut self,
    name: &str,
    mut value_provider: ValueProvider,
    refresh_period_sec: u64,
  )
  where ValueProvider: 'static + C5ValueProvider
  {

    let hydrate_context = HydrateContext {
      logger: self._logger.clone(),
    };

    let provided_data_option = self._provided_data.get_vec(name);

    if provided_data_option.is_none() {

      self._logger.warn(format!("{} value provider has no data to provide. Either remove this value provider or add configuration it must provide.", name).as_str());
      return;
    }

    let provided_data = provided_data_option.unwrap();

    for p_data in provided_data {
      value_provider.register(p_data);
    }

    value_provider.hydrate(&*self._set_data_fn, true, &hydrate_context);

    self._value_providers.lock().insert(name.to_string(), Box::from(value_provider));

    if refresh_period_sec > 0 {
      // logger.debug(format!("Will refresh {} Value Provider every {} seconds.", name, refresh_period_sec));

      let refresh_period_duration = Duration::from_secs(refresh_period_sec);

      let value_providers_clone = self._value_providers.clone();
      let set_data_fn = self._set_data_fn.clone();
      let name_clone = name.to_string();
      let job = move || {

        let value_providers = value_providers_clone.clone();
        let value_providers_lock = value_providers.lock();
        let value_provider_result = value_providers_lock.get(&name_clone);

        if let Some(value_provider) = value_provider_result {
          value_provider.hydrate(&*set_data_fn, true, &hydrate_context);
        }
      };

      let job_handle = self._scheduled_thread_pool.execute_at_fixed_rate(
        refresh_period_duration.clone(),
        refresh_period_duration,
        job
      );

      self._scheduled_provider_job_handles.push(job_handle);
    } else {
      // logger.debug(format!("Will not be refreshing {} Value Provider", name));
    }
  }
}

impl Drop for C5StoreMgr {
  fn drop(&mut self) {

    self._logger.info("Stopping C5StoreMgr");

    while self._scheduled_provider_job_handles.len() > 0 {

      let job_handle = self._scheduled_provider_job_handles.pop().unwrap();
      job_handle.cancel();
    }

    self._logger.info("Stopped C5StoreMgr");
  }
}

pub fn create_c5store(
  config_file_paths: Vec<PathBuf>,
  mut options_option: Option<C5StoreOptions>
) -> (C5StoreRoot, C5StoreMgr) {

  if options_option.is_none() {
    options_option = Some(C5StoreOptions::default());
  }

  let mut secret_key_store = SecretKeyStore::new();
  if let Some(options) = &mut options_option {

    if options.stats.is_none() {
      options.stats = Some(Arc::new(StatsRecorderStub {}));
    }

    if options.logger.is_none() {
      options.logger = Some(Arc::new(ConsoleLogger {}));
    }

    if options.change_delay_period.is_none() {
      options.change_delay_period = Some(DEFAULT_CHANGE_DELAY_PERIOD);
    }
  
    if options.secret_opts.secret_key_store_configure_fn.is_some() {

      (options.secret_opts.secret_key_store_configure_fn.as_mut().unwrap())(&mut secret_key_store);
    }

    load_secret_key_files(options.secret_opts.secret_keys_path.as_ref(), &mut secret_key_store);
  }

  let secret_key_store =  Arc::new(secret_key_store);
  let options = options_option.as_ref().unwrap();
  let logger = options.logger.as_ref().unwrap().clone();
  let stats = options.stats.as_ref().unwrap().clone();

  let data_store =  C5DataStore::new(
    logger.clone(),
    stats.clone(),
    options.secret_opts.secret_key_path_segment.as_ref().unwrap().clone(),
    secret_key_store.clone(),
  );
  let subscriptions = C5StoreSubscriptions::new();
  let root = C5StoreRoot::new(data_store.clone(), subscriptions.clone());
  let change_notifier = Arc::new(ChangeNotifier::new(
    Duration::from_millis(options.change_delay_period.unwrap()),
    data_store.clone(),
    subscriptions.clone(),
  ));

  let set_fn_data_store_clone = data_store.clone();
  let set_fn_change_notifier_clone = change_notifier.clone();
  let set_data_fn = Arc::new(move |key: &str, value: C5DataValue| {

    let data_store = set_fn_data_store_clone.clone();
    let change_notifier = set_fn_change_notifier_clone.clone();
    let already_exists = data_store.exists(key);

    if !already_exists {
      data_store.set_data(key, value);
    } else {

      let old_value = data_store.get_data(key);

      if old_value.is_some() && old_value.as_ref().unwrap() != &value {

        data_store.set_data(key, value);
        change_notifier.notify_changed(key);
      }
    }
  });

  let mut provided_data: MultiMap<String, C5DataValue> = MultiMap::new();

  read_config_data(config_file_paths, set_data_fn.clone(), &mut provided_data);

  let c5store_mgr = C5StoreMgr::new(
    root.clone(),
    logger.clone(),
    stats.clone(),
    change_notifier.clone(),
    set_data_fn,
    provided_data,
  );

  return (root, c5store_mgr);
}

pub fn load_secret_key_files(
  secret_keys_path_str: Option<&PathBuf>,
  secret_key_store: & mut SecretKeyStore,
) {

  if secret_keys_path_str.is_none() {
    return;
  }

  let skpath_str= &*secret_keys_path_str.unwrap().clone();
  let secret_keys_path = Path::new(skpath_str);
  
  if !secret_keys_path.exists() {
    return;
  }

  let files = read_dir(secret_keys_path);

  for dir_entry in files.unwrap() {
    let entry_path = dir_entry.unwrap().path();

    if entry_path.is_dir() {
      continue;
    }

    let mut key = std::fs::read(&entry_path).unwrap();
    
    let file_ext = entry_path.extension().unwrap().to_str().unwrap();
    let file_name = entry_path.file_name().unwrap().to_str().unwrap();

    let key_name = &file_name[..(file_name.len() - file_ext.len() - 1)];

    if file_ext == "pem" {
      key = parse_openssl_25519_privkey(&key).unwrap().to_bytes().to_vec();
    }

    secret_key_store.set_key(key_name, key);
  }
}

pub fn read_config_data(
  config_file_paths: Vec<PathBuf>,
  set_data_fn: Arc<SetDataFn>,
  provided_data: &mut MultiMap<String, C5DataValue>,
) {

  let mut raw_config_data: HashMap<String, C5DataValue> = HashMap::new();
  let mut config_data: HashMap<String, C5DataValue> = HashMap::new();

  for config_file_path in config_file_paths.iter() {
    let config_file_reader_result = std::fs::File::open(config_file_path);

    if let Ok(config_file_reader) = config_file_reader_result {
      let config_value_result: Result<HashMap<String, Value>, serde_yaml::Error> = serde_yaml::from_reader(config_file_reader);

      if config_value_result.is_err() {
        continue;
      }

      let config_value = _map_from_serde_yaml_valuemap(config_value_result.unwrap());
      _merge(&mut raw_config_data, &config_value);
    }
  }

  _take_provided_data(&mut raw_config_data, &mut config_data, provided_data);

  for (key, value) in config_data {
    set_data_fn(key.as_str(), value);
  }
}

fn _take_provided_data(
  raw_config_data: &mut HashMap<String, C5DataValue>,
  config_data: &mut HashMap<String, C5DataValue>,
  provided_data: &mut MultiMap<String, C5DataValue>,
) {

  _take_provided_data_helper(raw_config_data, config_data, provided_data, String::new());
}

fn _take_provided_data_helper(
  raw_config_data: &mut HashMap<String, C5DataValue>,
  config_data: &mut HashMap<String, C5DataValue>,
  provided_data: &mut MultiMap<String, C5DataValue>,
  keypath: String,
) {
  let keys: Vec<String> = raw_config_data.keys().into_iter().cloned().collect();

  for key in keys {
    let mut value = raw_config_data.get_mut(&key).unwrap();
    let new_keypath: String;

    if keypath.is_empty() {
      new_keypath = key.clone();
    } else {
      new_keypath = keypath.clone() + "." + &key;
    }

    if let C5DataValue::Map(ref mut data_map) = &mut value {
      if !data_map.contains_key(CONFIG_KEY_PROVIDER) {
        _take_provided_data_helper(data_map, config_data, provided_data, new_keypath);

        if data_map.len() == 0 {
          raw_config_data.remove(&key);
        }
      } else {

        data_map.insert(CONFIG_KEY_KEYPATH.to_string(), C5DataValue::String(new_keypath));
        data_map.insert(CONFIG_KEY_KEYNAME.to_string(), C5DataValue::String(key.clone()));

        let provider_name_c5val = data_map.get(CONFIG_KEY_PROVIDER).unwrap();

        if let C5DataValue::String(provider_name) = provider_name_c5val {
          provided_data.insert(provider_name.clone(), value.clone());
        }

        raw_config_data.remove(&key);
      }
    } else {
      config_data.insert(new_keypath.clone(), value.clone());
    }
  }
}

fn _map_from_serde_yaml_valuemap(value_map: HashMap<String, Value>) -> HashMap<String, C5DataValue> {

  let mut result = HashMap::new();

  for (key, value) in value_map.iter() {
    result.insert(key.clone(), serde_yaml_val_to_c5_value(value.clone()));
  }

  return result;
}

fn _merge(dest: &mut HashMap<String, C5DataValue>, src: &HashMap<String, C5DataValue>) {

  for (src_key, src_value) in src {

    if dest.contains_key(src_key.as_str()) {

      let dest_value_option = dest.get_mut(src_key.as_str());
      let dest_value = dest_value_option.unwrap();

      if let C5DataValue::Map(ref mut dest_map) = dest_value {

        if let C5DataValue::Map(src_map) = src_value {
          // check dest key type
          _merge( dest_map, &src_map);
        } else {
          dest.insert(src_key.clone(), src_value.clone());
        }
      } else {
        dest.insert(src_key.clone(), src_value.clone());
      }

      continue;
    }

    dest.insert(src_key.clone(), src_value.clone());
  }
}

pub fn default_config_paths(config_dir: &str, release_env: &str, env: &str, region: &str) -> Vec<PathBuf> {

  let mut paths = vec![];

  paths.push(PathBuf::from(format!("{}/common.yaml", config_dir)));
  paths.push(PathBuf::from(format!("{}/{}.yaml", config_dir, release_env).as_str()));
  paths.push(PathBuf::from(format!("{}/{}.yaml", config_dir, env).as_str()));
  paths.push(PathBuf::from(format!("{}/{}.yaml", config_dir, region).as_str()));
  paths.push(PathBuf::from(format!("{}/{}-{}.yaml", config_dir, env, region).as_str()));

  return paths;
}

#[cfg(test)]
mod tests {
  use std::path::PathBuf;

  use ecies_25519::EciesX25519;

  use crate::secrets::{Base64SecretDecryptor, EciesX25519SecretDecryptor, SecretKeyStore};
  use crate::{C5StoreMgr, C5StoreOptions, SecretOptions, create_c5store, default_config_paths};
  use crate::C5Store;
  use crate::value::C5DataValue;

  #[test]
  fn test_config_contains_bill_bar_existence() {
    let (c5store, _c5store_mgr) = _create_c5store();

    assert_eq!(c5store.exists("bill.barr"), true);
    assert_eq!(c5store.exists("bill"), false);
    assert_eq!(c5store.path_exists("bill.barr"), true);
    assert_eq!(c5store.path_exists("bill.barr."), false);
    assert_eq!(c5store.path_exists("bill"), true);
  }

  #[test]
  fn test_config_contains_bill_bar() {
    let (c5store, _c5store_mgr) = _create_c5store();

    assert_eq!(c5store.get("bill.barr").unwrap(), C5DataValue::String(String::from("AG")));
  }

  #[test]
  fn test_config_contains_example_test_and() {
    let (c5store, _c5store_mgr) = _create_c5store();

    assert_eq!(c5store.get("example.test.and").unwrap(), C5DataValue::UInteger(1));
    assert_eq!(c5store.get_into::<u64>("example.test.and").unwrap(), 1u64);
  }

  #[test]
  fn test_config_secrets_decrypt() {

    let mut config_file_paths = vec![];
    config_file_paths.push(PathBuf::from("configs/secret_test/secret_config.yaml"));

    let mut config_opt = C5StoreOptions::default();
    config_opt.secret_opts = SecretOptions::default();
    config_opt.secret_opts.secret_keys_path = Some(PathBuf::from("configs/secret_test/secret_keys"));
    config_opt.secret_opts.secret_key_store_configure_fn = Some(Box::from(|secret_key_store: &mut SecretKeyStore| {

      secret_key_store.set_decryptor("base64", Box::from(Base64SecretDecryptor{}));
      secret_key_store.set_decryptor("ecies_x25519", Box::from(EciesX25519SecretDecryptor::new(EciesX25519::new())));
    }));

    let (c5store, _c5store_mgr) = create_c5store(config_file_paths, Some(config_opt));

    assert_eq!(c5store.get("a_secret").unwrap(), C5DataValue::Bytes("abcd".as_bytes().to_vec()));
    assert_eq!(c5store.get("hello_secret").unwrap(), C5DataValue::Bytes("Hello World".as_bytes().to_vec()));
  }

  #[test]
  fn test_bad_config_secrets_decrypt() {

    let mut config_file_paths = vec![];
    config_file_paths.push(PathBuf::from("configs/secret_test/secret_config_bad.yaml"));

    let mut config_opt = C5StoreOptions::default();
    config_opt.secret_opts = SecretOptions::default();
    config_opt.secret_opts.secret_keys_path = Some(PathBuf::from("configs/secret_test/secret_keys"));
    config_opt.secret_opts.secret_key_store_configure_fn = Some(Box::from(|secret_key_store: &mut SecretKeyStore| {

      secret_key_store.set_decryptor("base64", Box::from(Base64SecretDecryptor{}));
      secret_key_store.set_decryptor("ecies_x25519", Box::from(EciesX25519SecretDecryptor::new(EciesX25519::new())));
    }));

    let (c5store, _c5store_mgr) = create_c5store(config_file_paths, Some(config_opt));

    assert_eq!(c5store.get("a_secret"), None);
    assert_eq!(c5store.get("hello_secret"), None);
  }

  fn _create_c5store() -> (impl C5Store, C5StoreMgr) {
    let config_file_paths = default_config_paths("configs/test/config", "development", "local", "private");

    return create_c5store(config_file_paths, None);
  }
}
