mod data;
pub mod error;
mod internal;
pub mod providers;
#[cfg(feature = "secrets")]
pub mod secrets;
#[cfg(not(feature = "secrets"))]
pub mod secrets_dummy;
pub mod serialization;
pub mod telemetry;
pub mod value;
pub mod util;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::{env, fs};
use std::fs::read_dir;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use curve25519_parser::parse_openssl_25519_privkey;
#[cfg(feature = "dotenv")]
use dotenvy;
use error::ConfigError;
use multimap::MultiMap;
use parking_lot::Mutex;
use scheduled_thread_pool::{JobHandle, ScheduledThreadPool};
use serde::de::DeserializeOwned;
use serialization::map_from_serde_yaml_valuemap;
#[cfg(feature = "toml")]
use serialization::map_from_toml_value_map;
use util::build_flat_map;
use value::c5_value_to_serde_json;

use crate::data::HashsetMultiMap;
use crate::internal::{C5DataStore, C5StoreDataValueRef, C5StoreSubscriptions};
use crate::providers::{C5ValueProvider, CONFIG_KEY_KEYNAME, CONFIG_KEY_KEYPATH, CONFIG_KEY_PROVIDER};
#[cfg(feature = "secrets")]
use crate::secrets::SecretKeyStore;
#[cfg(not(feature = "secrets"))]
use crate::secrets_dummy::{SecretKeyStore, SecretKeyStoreConfiguratorFn};
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
#[cfg(feature = "secrets")]
pub type SecretKeyStoreConfiguratorFn = dyn FnMut(&mut SecretKeyStore);

#[cfg(feature = "secrets")]
pub struct SecretOptions {
  pub secret_key_path_segment: Option<String>,
  pub secret_keys_path: Option<PathBuf>,
  pub secret_key_store_configure_fn: Option<Box<SecretKeyStoreConfiguratorFn>>,
  pub load_secret_keys_from_env: bool,
  pub secret_key_env_prefix: Option<String>, // e.g., "C5_SECRETKEY_"
}

impl Default for SecretOptions {
  fn default() -> Self {
    return Self {
      secret_key_path_segment: Some(".c5encval".to_string()),
      secret_keys_path: None,
      secret_key_store_configure_fn: None,
      load_secret_keys_from_env: false,
      secret_key_env_prefix: Some("C5_SECRETKEY_".to_string()),
    };
  }
}

#[cfg(not(feature = "secrets"))]
#[derive(Default)]
pub struct SecretOptions {}

pub struct C5StoreOptions {
  pub logger: Option<Arc<dyn Logger>>,
  pub stats: Option<Arc<dyn StatsRecorder>>,
  pub change_delay_period: Option<u64>,
  pub secret_opts: SecretOptions,
  #[cfg(feature = "dotenv")]
  pub dotenv_path: Option<PathBuf>, // Path to .env file
}

impl Default for C5StoreOptions {
  fn default() -> Self {
    return Self {
      logger: None,
      stats: None,
      change_delay_period: Some(DEFAULT_CHANGE_DELAY_PERIOD),
      secret_opts: SecretOptions::default(),
      #[cfg(feature = "dotenv")]
      dotenv_path: None,
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
      thread_pool: Arc::new(
        ScheduledThreadPool::builder()
        .num_threads(1)
        .thread_name_pattern("c5Store_change_notifier")
        .build()
      ),
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

  fn get_into<Val>(&self, key_path: &str) -> Result<Val, ConfigError>
  where C5DataValue: TryInto<Val, Error = ConfigError>;

  fn get_into_struct<Val>(&self, key_path: &str) -> Result<Val, ConfigError>
  where Val: DeserializeOwned;

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
  
  fn get_into<Val>(&self, key_path: &str) -> Result<Val, ConfigError>
    where C5DataValue: TryInto<Val, Error = ConfigError>
  {
    self._data_store.get_data(key_path)
      .ok_or_else(|| ConfigError::KeyNotFound(key_path.to_string()))
      .and_then(|val| val.try_into())
  }
  
  fn get_into_struct<Val>(&self, key_path: &str) -> Result<Val, ConfigError>
    where Val: DeserializeOwned
  {
    let value_option = self.get(key_path);

    let c5_value = value_option.ok_or_else(|| ConfigError::KeyNotFound(key_path.to_string()))?;

    // Convert C5DataValue to serde_json::Value for deserialization
    let json_value = c5_value_to_serde_json(c5_value).map_err(|e| ConfigError::Internal(format!("Failed C5->JSON conversion: {}", e)))?; // Add helper below

    serde_json::from_value(json_value).map_err(|e| ConfigError::DeserializationError {
      key: key_path.to_string(),
      source: e,
    })
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

  fn get_into<Val>(&self, key_path: &str) -> Result<Val, ConfigError>
    where C5DataValue: TryInto<Val, Error = ConfigError>
  {
    return self._root.get_into(&self._merge_key_path(key_path));
  }

  fn get_into_struct<Val>(&self, key_path: &str) -> Result<Val, ConfigError>
    where Val: DeserializeOwned
  {
    return self._root.get_into_struct(&self._merge_key_path(key_path));
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
      _scheduled_thread_pool: ScheduledThreadPool::builder().num_threads(8).thread_name_pattern("c5store_mgr").build(),
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
) -> Result<(C5StoreRoot, C5StoreMgr), ConfigError> {

  if options_option.is_none() {
    options_option = Some(C5StoreOptions::default());
  }

  let mut options = options_option.unwrap(); 

  #[cfg(feature = "dotenv")]
  {
    if let Some(dotenv_path) = &options.dotenv_path {
      println!("[dotenv] Loading environment from {:?}", dotenv_path); // Optional log
       match dotenvy::from_path(dotenv_path) {
         Ok(_) => {},
         Err(e) if e.not_found() => {}, // Ignore if file not found, common case
         Err(e) => return Err(ConfigError::DotEnvLoadError { path: dotenv_path.clone(), source: e }),
       }
    } else {
      // Maybe try loading default .env path? Or require explicit path?
      // Let's require explicit path for now via C5StoreOptions.
    }
  }

  #[cfg(not(feature = "secrets"))]
  let mut secret_key_store = SecretKeyStore::default(); 

  #[cfg(feature = "secrets")]
  let secret_key_store = {

    let mut secret_key_store = SecretKeyStore::new();

    if let Some(mut configure_fn) = options.secret_opts.secret_key_store_configure_fn {

      (configure_fn)(&mut secret_key_store);
    }

    load_secret_key_files(options.secret_opts.secret_keys_path.as_ref(), &mut secret_key_store)?;
    
    if options.secret_opts.load_secret_keys_from_env {
      let prefix = options.secret_opts.secret_key_env_prefix.as_deref().unwrap_or("C5_SECRETKEY_");
      load_secret_keys_from_env(prefix, &mut secret_key_store);
    }

    secret_key_store
  };


  if options.stats.is_none() {
    options.stats = Some(Arc::new(StatsRecorderStub {}));
  }

  if options.logger.is_none() {
    options.logger = Some(Arc::new(ConsoleLogger {}));
  }

  if options.change_delay_period.is_none() {
    options.change_delay_period = Some(DEFAULT_CHANGE_DELAY_PERIOD);
  }

  let secret_key_store =  Arc::new(secret_key_store);
  let logger = options.logger.as_ref().unwrap().clone();
  let stats = options.stats.as_ref().unwrap().clone();

  let secret_segment = {
     #[cfg(feature = "secrets")] { options.secret_opts.secret_key_path_segment.clone().unwrap_or(".c5encval".to_string()) }
     #[cfg(not(feature = "secrets"))] { ".c5encval".to_string() }
  };

  let data_store =  C5DataStore::new(
    logger.clone(),
    stats.clone(),
    secret_segment,
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

  read_config_data(&config_file_paths, set_data_fn.clone(), &mut provided_data)?;
  let c5store_mgr = C5StoreMgr::new(
    root.clone(),
    logger.clone(),
    stats.clone(),
    change_notifier.clone(),
    set_data_fn,
    provided_data,
  );

  return Ok((root, c5store_mgr));
}

// Helper function to read environment variables
fn process_environment_variables(set_data_fn: Arc<SetDataFn>) {
  const PREFIX: &str = "C5_";
  const SEPARATOR: &str = "__";

  for (key, value) in env::vars() {
    if key.starts_with(PREFIX) {
      let trimmed_key = key.trim_start_matches(PREFIX);
      let c5_key = trimmed_key.replace(SEPARATOR, ".").to_lowercase(); // Convert C5_DB__HOST to db.host

      // PHASE 1 CHANGE: Treat env vars as strings initially.
      // Let get_into/get_into_struct handle final conversion.
      // More complex parsing could be added later if needed.
      println!("[EnvVar] Setting '{}' from env var '{}'", c5_key, key); // Optional: Add logging
      set_data_fn(&c5_key, C5DataValue::String(value));
    }
  }
}

#[cfg(feature = "secrets")]
pub fn load_secret_key_files(
  secret_keys_path_str: Option<&PathBuf>,
  secret_key_store: &mut SecretKeyStore,
) -> Result<(), ConfigError> {

  if secret_keys_path_str.is_none() {
    return Ok(());
  }

  let skpath= secret_keys_path_str.unwrap();
  
  if !skpath.exists() {
     println!("[Secrets] Warning: Secret keys path {:?} does not exist.", skpath);
     return Ok(()); // Don't error if path doesn't exist
  }

  if !skpath.is_dir() {
    return Err(ConfigError::Message(format!("Secret keys path {:?} is not a directory", skpath)));
  }

  let files = read_dir(skpath)
    .map_err(|e| ConfigError::IoError { path: skpath.clone(), source: e })?;

    for dir_entry_result in files {
      let dir_entry = dir_entry_result.map_err(|e| ConfigError::IoError { path: skpath.clone(), source: e })?;
     let entry_path = dir_entry.path();
 
     if entry_path.is_dir() {
       continue;
     }
 
      let key_result = fs::read(&entry_path)
          .map_err(|e| ConfigError::IoError { path: entry_path.clone(), source: e });
      if key_result.is_err() {
          eprintln!("[Secrets] Error reading key file {:?}: {:?}", entry_path, key_result.err());
          continue; // Skip file on read error? Or return Err? Let's skip for now.
      }
      let mut key = key_result.unwrap();
 
     let file_ext_os = entry_path.extension();
     let file_name_os = entry_path.file_name();
 
     if file_ext_os.is_none() || file_name_os.is_none() {
        eprintln!("[Secrets] Skipping file with missing name or extension: {:?}", entry_path);
        continue;
     }
     let file_ext = file_ext_os.unwrap().to_str().unwrap_or("");
     let file_name = file_name_os.unwrap().to_str().unwrap_or("");
 
     if file_name.is_empty() || file_name.len() <= file_ext.len() + 1 {
         eprintln!("[Secrets] Skipping file with invalid name format: {:?}", entry_path);
         continue;
     }
 
     // Robustly get key name
     let key_name = match file_name.rfind('.') {
         Some(dot_index) => &file_name[..dot_index],
         None => file_name, // Should not happen if extension exists, but handle defensively
     };
 
 
     if file_ext == "pem" {
        // Handle potential parsing errors
        match parse_openssl_25519_privkey(&key) {
           Ok(parsed_key) => key = parsed_key.to_bytes().to_vec(),
           Err(e) => {
              eprintln!("[Secrets] Error parsing PEM key file {:?}: {}", entry_path, e);
              continue; // Skip invalid PEM files
            }
        }
     }
 
     println!("[Secrets] Loading key '{}' from file {:?}", key_name, entry_path); // Optional log
     secret_key_store.set_key(key_name, key);
   }
   Ok(())
}

#[cfg(feature = "secrets")]
fn load_secret_keys_from_env(prefix: &str, secret_key_store: &mut SecretKeyStore) {
  use base64::Engine;
  for (key, value) in env::vars() {
    if key.starts_with(prefix) {
      let key_name = key.trim_start_matches(prefix).to_lowercase();
      // Assume value is base64 encoded key bytes
      match base64::engine::general_purpose::STANDARD.decode(&value) {
        Ok(key_bytes) => {
          println!("[Secrets] Loading key '{}' from env var '{}'", key_name, key); // Optional log
          secret_key_store.set_key(&key_name, key_bytes);
        }
        Err(e) => {
          eprintln!("[Secrets] Error base64 decoding secret key from env var '{}': {}", key, e);
        }
      }
    }
  }
}

/// Reads configuration from specified paths (files/directories), merges them,
/// applies environment variable overrides, separates provider configurations,
/// and applies the final values to the store via the provided setter function.
///
/// Handles YAML and TOML file formats. Reads environment variables starting
/// with "C5_" using "__" as a separator (e.g., C5_DATABASE__HOST becomes database.host).
///
/// Order of precedence: Environment Variables > Last File Read > First File Read.
pub fn read_config_data(
  config_file_paths:  &[PathBuf],
  set_data_fn: Arc<SetDataFn>,
  provided_data: &mut MultiMap<String, C5DataValue>,
) -> Result<(), ConfigError> {

  let mut merged_config: HashMap<String, C5DataValue> = HashMap::new();
  let mut files_to_process: Vec<PathBuf> = Vec::new();

  // --- 1. Expand directories and collect all individual files ---
  for path in config_file_paths {
    if path.is_dir() {
      match read_dir(path) {
        Ok(entries) => {
          let mut dir_files: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|p| p.is_file())
            .collect();
          // Sort files alphabetically within directory for deterministic order
          dir_files.sort();
          files_to_process.extend(dir_files);
        }
        Err(e) => return Err(ConfigError::IoError { path: path.clone(), source: e }),
      }
    } else if path.is_file() {
      files_to_process.push(path.clone());
    } else {
      // Log or handle non-existent initial paths if needed
      println!("[Config] Warning: Initial path {:?} does not exist or is not a file/directory.", path);
    }
  }

  // --- 2. Load and merge each eligible file (YAML or TOML) ---
  for file_path in &files_to_process { // Borrow files_to_process
    let extension = file_path.extension().and_then(OsStr::to_str);

    // Define parser function type alias for clarity
    type ParserFn = fn(&str, &PathBuf) -> Result<HashMap<String, C5DataValue>, ConfigError>;

    let parser: Option<ParserFn> = match extension {
      Some("yaml") | Some("yml") => Some(|content, path| {
        serde_yaml::from_str::<HashMap<String, serde_yaml::Value>>(content)
          .map_err(|e| ConfigError::YamlParseError { path: path.clone(), source: e })
          .map(map_from_serde_yaml_valuemap) // Use existing helper
      }),
      #[cfg(feature = "toml")]
      Some("toml") => Some(|content, path| {
        toml::from_str::<HashMap<String, toml::Value>>(content)
          .map_err(|e| ConfigError::TomlParseError { path: path.clone(), source: e })
          .map(map_from_toml_value_map) // Use TOML helper
      }),
      _ => None, // Skip unsupported file types
    };

    if let Some(parse_fn) = parser {
      match fs::read_to_string(&file_path) {
        Ok(content) => {
          match parse_fn(&content, file_path) {
            Ok(config_value) => {
              println!("[Config] Merging config from file {:?}", file_path);
              _merge(&mut merged_config, &config_value); // Merge file config
            }
            Err(e) => return Err(e), // Propagate parse error
          }
        }
        Err(e) => {
          // Handle IO errors during read
          if e.kind() == std::io::ErrorKind::NotFound {
            // This case might be less likely if we check is_file earlier, but handle defensively
            println!("[Config] Warning: File {:?} not found during read (unexpected).", file_path);
          } else {
            return Err(ConfigError::IoError { path: file_path.clone(), source: e });
          }
        }
      }
    } else {
      // Optionally log skipped files with unsupported extensions
      // println!("[Config] Skipping file with unsupported extension: {:?}", file_path);
    }
  }

  // --- 3. Read and merge environment variables (OVERWRITING file values) ---
  const PREFIX: &str = "C5_";
  const SEPARATOR: &str = "__";
  let mut env_config: HashMap<String, C5DataValue> = HashMap::new();

  for (key, value) in env::vars() {
    if key.starts_with(PREFIX) {
      let trimmed_key = key.trim_start_matches(PREFIX);
      // Convert C5_DATABASE__HOST to database.host (lowercase)
      let c5_key = trimmed_key.replace(SEPARATOR, ".").to_lowercase();

      println!("[Config] Reading '{}' from env var '{}'", c5_key, key);

      // Build nested map structure from dot notation for merging
      let mut current_level = &mut env_config;
      let key_parts: Vec<&str> = c5_key.split('.').collect();

      // Check for empty key parts resulting from separators at start/end or double separators
       if key_parts.iter().any(|&part| part.is_empty()) {
        eprintln!("[Config] Warning: Skipping env var '{}' due to invalid key format '{}' (empty segments)", key, c5_key);
        continue;
       }


      for (i, part) in key_parts.iter().enumerate() {
        if i == key_parts.len() - 1 {
          // Last part, insert the value
          current_level.insert(part.to_string(), C5DataValue::String(value.clone()));
        } else {
          // Navigate or create nested map entry
           let entry = current_level
               .entry(part.to_string())
               .or_insert_with(|| C5DataValue::Map(HashMap::new()));

           // Check if the entry is actually a map before proceeding
           match entry {
              C5DataValue::Map(map) => current_level = map,
              _ => return Err(ConfigError::Message(format!(
              "Env var key conflict: Cannot create nested map for '{}' because part '{}' conflicts with an existing non-map value.",
              c5_key, part
            ))),
           }
        }
      }
    }
  }
  // Merge env vars over file config
  _merge(&mut merged_config, &env_config);
  // --- End Environment Variable Processing ---

  // --- 4. Separate provider configuration from the final merged map ---
  let mut config_map_for_store_intermediate = HashMap::new(); // Temporary map needed by current _take_provided_data signature
  // _take_provided_data modifies merged_config IN PLACE, removing provider sections
  // It puts provider config into `provided_data`
  // It puts non-provider leaf values into `config_map_for_store_intermediate` (which we ignore)
   _take_provided_data(
      &mut merged_config,
      &mut config_map_for_store_intermediate, // Argument required by current signature, but result ignored
      provided_data
   );
   // After this call, `merged_config` contains only the final, non-provider values/maps

  // --- 5. Apply the final non-provider values to the store ---
  // Flatten the remaining nested structure in merged_config and apply using set_data_fn
  let mut final_flat_map = HashMap::new();
  build_flat_map(&mut merged_config, &mut final_flat_map, String::new()); // Use util helper

   for (key, value) in final_flat_map {
      set_data_fn(&key, value);
   }

  Ok(()) // Signal success

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
    let (c5store, _c5store_mgr) = _create_c5store_test();

    assert_eq!(c5store.exists("bill.barr"), true);
    assert_eq!(c5store.exists("bill"), false);
    assert_eq!(c5store.path_exists("bill.barr"), true);
    assert_eq!(c5store.path_exists("bill.barr."), false);
    assert_eq!(c5store.path_exists("bill"), true);
  }

  #[test]
  fn test_config_contains_bill_bar() {
    let (c5store, _c5store_mgr) = _create_c5store_test();

    assert_eq!(c5store.get("bill.barr").unwrap(), C5DataValue::String(String::from("AG")));
  }

  #[test]
  fn test_config_contains_example_test_and() {
    let (c5store, _c5store_mgr) = _create_c5store_test();

    assert_eq!(c5store.get("example.test.and").unwrap(), C5DataValue::UInteger(1));
    assert_eq!(c5store.get_into::<u64>("example.test.and").unwrap(), 1u64);
  }

  #[test]
  #[cfg(feature = "secrets")]
  fn test_config_secrets_decrypt() {
    use crate::secrets::{Base64SecretDecryptor, EciesX25519SecretDecryptor};
    use ecies_25519::EciesX25519;

    let mut config_file_paths = vec![];
    config_file_paths.push(PathBuf::from("configs/secret_test/secret_config.yaml"));

    let mut config_opt = C5StoreOptions::default();
    config_opt.secret_opts = SecretOptions {
       secret_keys_path: Some(PathBuf::from("configs/secret_test/secret_keys")),
       secret_key_store_configure_fn: Some(Box::new(|secret_key_store: &mut SecretKeyStore| {
          secret_key_store.set_decryptor("base64", Box::from(Base64SecretDecryptor {}));
          secret_key_store.set_decryptor("ecies_x25519", Box::from(EciesX25519SecretDecryptor::new(EciesX25519::new())));
       })),
       load_secret_keys_from_env: false,
       secret_key_env_prefix: None,
       ..Default::default()
    };


    let (c5store, _c5store_mgr) = create_c5store(config_file_paths, Some(config_opt)).expect("Secrets test store creation failed");

    assert_eq!(c5store.get("a_secret").unwrap(), C5DataValue::Bytes("abcd".as_bytes().to_vec()));
    assert_eq!(c5store.get("hello_secret").unwrap(), C5DataValue::Bytes("Hello World".as_bytes().to_vec()));
  }

  #[test]
   #[cfg(feature = "secrets")]
   fn test_bad_config_secrets_decrypt() {
        use crate::secrets::{Base64SecretDecryptor, EciesX25519SecretDecryptor};
        use ecies_25519::EciesX25519;

       let mut config_file_paths = vec![];
       config_file_paths.push(PathBuf::from("configs/secret_test/secret_config_bad.yaml"));

       let mut config_opt = C5StoreOptions::default();
       config_opt.secret_opts = SecretOptions {
            secret_keys_path: Some(PathBuf::from("configs/secret_test/secret_keys")),
            secret_key_store_configure_fn: Some(Box::new(|secret_key_store: &mut SecretKeyStore| {
               secret_key_store.set_decryptor("base64", Box::from(Base64SecretDecryptor {}));
               secret_key_store.set_decryptor("ecies_x25519", Box::from(EciesX25519SecretDecryptor::new(EciesX25519::new())));
            })),
            load_secret_keys_from_env: false,
            secret_key_env_prefix: None,
            ..Default::default()
        };

       let (c5store, _c5store_mgr) = create_c5store(config_file_paths, Some(config_opt)).expect("Bad secrets test store creation failed");

       // Behavior might change with better error handling, maybe secrets just aren't loaded
       // Let's assume `get` still returns None if decryption failed during set_data
       assert_eq!(c5store.get("bad_secret"), None);
   }

   fn _create_c5store_test() -> (impl C5Store, C5StoreMgr) {
    let config_file_paths = default_config_paths("configs/test/config", "development", "local", "private");
    create_c5store(config_file_paths, None).expect("Test store creation failed")
  }
}
