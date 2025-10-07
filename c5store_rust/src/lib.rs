#[cfg(feature = "bootstrapper")]
pub mod bootstrapper;
mod c5_serde;
mod config_source;
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
pub mod util;
pub mod value;

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::read_dir;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::{env, fs};

use c5_serde::de::C5SerdeValueDeserializer;
use config_source::ConfigSource;
use curve25519_parser::parse_openssl_25519_privkey;
#[cfg(feature = "dotenv")]
use dotenvy;
use error::ConfigError;

use log::{debug, error, warn};
use multimap::MultiMap;
use parking_lot::Mutex;
use scheduled_thread_pool::{JobHandle, ScheduledThreadPool};
use serde::de::DeserializeOwned;
use serialization::map_from_serde_yaml_valuemap;
#[cfg(feature = "toml")]
use serialization::map_from_toml_value_map;
use util::build_flat_map;

use crate::data::HashsetMultiMap;
use crate::internal::{C5DataStore, C5StoreDataValueRef, C5StoreSubscriptions};
use crate::providers::{C5ValueProvider, CONFIG_KEY_KEYNAME, CONFIG_KEY_KEYPATH, CONFIG_KEY_PROVIDER};
#[cfg(feature = "secrets")]
use crate::secrets::SecretKeyStore;
#[cfg(feature = "secrets")]
use crate::secrets::systemd::SystemdCredential;
#[cfg(feature = "secrets")]
use crate::secrets::systemd::load_systemd_credentials;
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
// params: notify key path, key path, new value, old value (Option)
pub type DetailedChangeListener = dyn Fn(&str, &str, &C5DataValue, Option<&C5DataValue>) -> () + Send + Sync;
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
  pub load_credentials_from_systemd: Vec<SystemdCredential>,
}

impl Default for SecretOptions {
  fn default() -> Self {
    return Self {
      secret_key_path_segment: Some(".c5encval".to_string()),
      secret_keys_path: None,
      secret_key_store_configure_fn: None,
      load_secret_keys_from_env: false,
      secret_key_env_prefix: Some("C5_SECRETKEY_".to_string()),
      load_credentials_from_systemd: Vec::new(),
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
    };
  }
}

/// Define a struct to hold pending change info
struct PendingChange {
  old_value: Option<C5DataValue>,
  new_value: C5DataValue,
}

struct ChangeNotifier {
  debounce_job_handle: Arc<Mutex<RefCell<Option<JobHandle>>>>,
  thread_pool: Arc<ScheduledThreadPool>,
  delay_period: Duration,
  pending_changes: Arc<Mutex<HashMap<String, PendingChange>>>, // Key: changed_key_path
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
          .build(),
      ),
      delay_period,
      pending_changes: Arc::new(Mutex::new(HashMap::new())),
      _data_store: data_store,
      _subscriptions: subscriptions,
    };
  }

  pub fn notify_changed(
    &self,
    key: &str,
    old_value: Option<C5DataValue>, // Pass owned Option<C5DataValue>
    new_value: C5DataValue,         // Pass owned C5DataValue
  ) {
    let debounce_job_lock = self.debounce_job_handle.lock();

    self
      .pending_changes
      .lock()
      .insert(key.to_string(), PendingChange { old_value, new_value });

    let should_schedule = debounce_job_lock.borrow().is_none();
    if should_schedule {
      let debounce_mut = self.debounce_job_handle.clone();
      let pending_changes_arc = self.pending_changes.clone();
      let subscriptions = self._subscriptions.clone();

      let job = move || {
        let changes_to_process: HashMap<String, PendingChange> = pending_changes_arc.lock().drain().collect();

        let debounce_job_lock_inner = debounce_mut.lock();
        let mut job_handle_borrow_inner = debounce_job_lock_inner.borrow_mut(); // Mutable borrow here is fine
        job_handle_borrow_inner.take(); // Clear the handle
        drop(job_handle_borrow_inner); // Release mutable borrow
        drop(debounce_job_lock_inner); // Release lock

        // Process the collected changes
        if !changes_to_process.is_empty() {
          // Build map of ancestors to notify for each actual change
          let mut notifications_to_send: HashsetMultiMap<String, String> = HashsetMultiMap::new();
          for changed_key in changes_to_process.keys() {
            notifications_to_send.insert(changed_key.clone(), changed_key.clone());
            let mut key_ancestor_path = String::new();
            for part in changed_key.split('.') {
              if !key_ancestor_path.is_empty() {
                key_ancestor_path.push('.');
              }
              key_ancestor_path.push_str(part);
              if &key_ancestor_path != changed_key {
                // Don't add self as ancestor for notification map
                notifications_to_send.insert(changed_key.clone(), key_ancestor_path.clone());
              }
            }
          }

          // Iterate through actual changed keys and their corresponding ancestor paths to notify
          for (changed_key, notify_paths) in notifications_to_send.iter() {
            if let Some(change_detail) = changes_to_process.get(changed_key) {
              for notify_path in notify_paths {
                subscriptions.notify_value_change(
                  notify_path,
                  changed_key,
                  &change_detail.new_value,         // Pass reference to stored new value
                  change_detail.old_value.as_ref(), // Pass reference to stored Option<old value>
                );
              }
            }
          }
        }
      };

      debounce_job_lock.replace(Some(self.thread_pool.execute_after(self.delay_period.clone(), job)));
    }
  }
}

pub trait C5Store {
  fn get(&self, key_path: &str) -> Option<C5DataValue>;

  fn get_ref(&self, key_path: &str) -> Option<C5StoreDataValueRef>;

  fn get_into<Val>(&self, key_path: &str) -> Result<Val, ConfigError>
  where
    C5DataValue: TryInto<Val, Error = ConfigError>;

  fn get_into_struct<Val>(&self, key_path: &str) -> Result<Val, ConfigError>
  where
    Val: DeserializeOwned;

  fn exists(&self, key_path: &str) -> bool;

  fn path_exists(&self, key: &str) -> bool;

  //
  // Listens to changes to the given keyPath. keyPath can be any the entire path or ancestors.
  // By listening to an ancestor, one will receive one change event even if two children change.
  //
  fn subscribe(&self, key_path: &str, listener: Box<ChangeListener>);

  fn subscribe_detailed(&self, key_path: &str, listener: Box<DetailedChangeListener>);

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

  fn get_source(&self, key_path: &str) -> Option<ConfigSource>;
}

#[derive(Clone)]
pub struct C5StoreRoot {
  _data_store: C5DataStore,
  _subscriptions: C5StoreSubscriptions,
}

impl C5StoreRoot {
  pub(crate) fn new(c5data_store: C5DataStore, subscriptions: C5StoreSubscriptions) -> C5StoreRoot {
    return C5StoreRoot {
      _data_store: c5data_store,
      _subscriptions: subscriptions,
    };
  }
}

impl C5Store for C5StoreRoot {
  fn get(&self, key_path: &str) -> Option<C5DataValue> {
    return self._data_store.get_data(key_path);
  }

  fn get_into<Val>(&self, key_path: &str) -> Result<Val, ConfigError>
  where
    C5DataValue: TryInto<Val, Error = ConfigError>,
  {
    self
      ._data_store
      .get_data(key_path)
      .ok_or_else(|| ConfigError::KeyNotFound(key_path.to_string()))
      .and_then(|val| val.try_into())
  }

  fn get_into_struct<Val>(&self, key_path: &str) -> Result<Val, ConfigError>
  where
    Val: DeserializeOwned,
  {
    if let Some(direct_c5_value) = self.get(key_path) {
      // Attempt to deserialize this direct C5DataValue
      // We need to check if it's a Map or Array, as structs usually deserialize from these.
      // Primitive types might deserialize if the struct is a newtype struct.
      match direct_c5_value {
        C5DataValue::Map(_)
        | C5DataValue::Array(_)
        | C5DataValue::String(_)
        | C5DataValue::Integer(_)
        | C5DataValue::UInteger(_)
        | C5DataValue::Float(_)
        | C5DataValue::Boolean(_)
        | C5DataValue::Bytes(_) => {
          // It's a potentially deserializable type.
          let deserializer = C5SerdeValueDeserializer::from_c5(&direct_c5_value);
          match Val::deserialize(deserializer) {
            Ok(result) => return Ok(result), // Success with direct value!
            Err(direct_err) => {
              // It existed directly, but didn't deserialize.
              // This *might* mean it wasn't the intended struct map,
              // OR it could be a genuine partial map where flattened keys should complete it.
              // Let's proceed to Strategy 2.
              // If it's not a Map, prefix fetch is unlikely to help unless the prefix itself IS the struct.
              if !matches!(direct_c5_value, C5DataValue::Map(_)) && !key_path.is_empty() {
                // If the direct value wasn't a map (and not at root), deserialization likely failed
                // because the type was wrong (e.g., trying to deserialize a struct from a C5 String).
                // The original error `direct_err` should be informative.
                // We still fall through to prefix fetch, as the prefix itself might contain the map.
              }
              // Log potential issue or decision to fallback?
              // self._data_store._logger.debug(format!("Direct value at '{}' failed to deserialize fully ({:?}), trying prefix fetch.", key_path, direct_err));
            }
          }
        }
        C5DataValue::Null => {
          // If direct value is Null, it won't deserialize into a typical struct.
          // Fall through to prefix search, as children might exist.
        }
      }
    }

    // --- Strategy 2: Fetch children using the key as a prefix and reconstruct a C5DataValue::Map or C5DataValue::Array ---
    // This handles flattened keys (env vars, flat files) or completes partial direct maps.
    match self._data_store.fetch_children_as_c5_value(key_path) {
      Ok(C5DataValue::Null) => {
        // No direct value (handled above) and no children found via prefix.
        // This could also mean the prefix *was* the target and we already tried and failed above.
        // If we are here, and a direct value was found but failed to deserialize, that error might be more relevant.
        // However, `KeyNotFound` is the common case if nothing was found at all.
        Err(ConfigError::KeyNotFound(key_path.to_string()))
      }
      Ok(reconstructed_c5_value) => {
        // Attempt to deserialize the C5DataValue reconstructed from children
        let deserializer = C5SerdeValueDeserializer::from_c5(&reconstructed_c5_value);
        Val::deserialize(deserializer).map_err(|e| {
          // The error `e` here is already a ConfigError from our C5ValueDeserializer
          // We might want to wrap it to add more context if needed, but often it's fine.
          // Example: if `e` is TypeMismatch, we might want to add the key_path here.
          match e {
            ConfigError::TypeMismatch {
              key: _,
              expected_type,
              found_type,
            } => ConfigError::TypeMismatch {
              key: key_path.to_string(),
              expected_type,
              found_type,
            },
            ConfigError::DeserializationError { key: _, source } => {
              // Should not happen if C5ValueDeserializer is correct
              ConfigError::DeserializationError {
                key: key_path.to_string(),
                source,
              }
            }
            other_err => other_err, // Propagate other errors like Message, KeyNotFound (from within MapAccess etc.)
          }
        })
      }
      Err(e) => Err(e), // Propagate errors from fetch_children_as_c5_value
    }
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

  fn subscribe_detailed(&self, key_path: &str, listener: Box<DetailedChangeListener>) {
    self._subscriptions.add_detailed(key_path, listener);
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

  fn get_source(&self, key_path: &str) -> Option<ConfigSource> {
    return self._data_store.get_source_info(key_path);
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
  where
    C5DataValue: TryInto<Val, Error = ConfigError>,
  {
    return self._root.get_into(&self._merge_key_path(key_path));
  }

  fn get_into_struct<Val>(&self, key_path: &str) -> Result<Val, ConfigError>
  where
    Val: DeserializeOwned,
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

  fn subscribe_detailed(&self, key_path: &str, listener: Box<DetailedChangeListener>) {
    self._root.subscribe_detailed(&self._merge_key_path(key_path), listener);
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
      }
      None => self._root.key_paths_with_prefix(None),
    };
  }

  fn current_key_path(&self) -> &str {
    return &self._key_path;
  }

  fn get_source(&self, key_path: &str) -> Option<ConfigSource> {
    self._root.get_source(&self._merge_key_path(key_path))
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
      _scheduled_thread_pool: ScheduledThreadPool::builder()
        .num_threads(8)
        .thread_name_pattern("c5store_mgr")
        .build(),
      _scheduled_provider_job_handles: vec![],
      _data_store: data_store,
      _logger: logger,
      _stats: stats,
      _change_notifier: change_notifier,
      _set_data_fn: set_data_fn,
      _provided_data: provided_data,
    };
  }

  pub fn set_value_provider<ValueProvider>(
    &mut self,
    name: &str,
    mut value_provider: ValueProvider,
    refresh_period_sec: u64,
  ) where
    ValueProvider: 'static + C5ValueProvider,
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

    self
      ._value_providers
      .lock()
      .insert(name.to_string(), Box::from(value_provider));

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
        job,
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
  mut options_option: Option<C5StoreOptions>,
) -> Result<(C5StoreRoot, C5StoreMgr), ConfigError> {
  if options_option.is_none() {
    options_option = Some(C5StoreOptions::default());
  }

  let mut options = options_option.unwrap();

  #[cfg(feature = "dotenv")]
  {
    if let Some(dotenv_path) = &options.dotenv_path {
      debug!("[dotenv] Loading environment from {:?}", dotenv_path); // Optional log
      match dotenvy::from_path(dotenv_path) {
        Ok(_) => {}
        Err(e) if e.not_found() => {} // Ignore if file not found, common case
        Err(e) => {
          return Err(ConfigError::DotEnvLoadError {
            path: dotenv_path.clone(),
            source: e,
          });
        }
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

    if let Some(ref mut configure_fn) = options.secret_opts.secret_key_store_configure_fn {
      (configure_fn)(&mut secret_key_store);
    }

    load_secret_key_files(options.secret_opts.secret_keys_path.as_ref(), &mut secret_key_store)?;

    if options.secret_opts.load_secret_keys_from_env {
      let prefix = options
        .secret_opts
        .secret_key_env_prefix
        .as_deref()
        .unwrap_or("C5_SECRETKEY_");
      load_secret_keys_from_env(prefix, &mut secret_key_store);
    }

    load_systemd_credentials(&options.secret_opts, &mut secret_key_store)?;

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

  let secret_key_store = Arc::new(secret_key_store);
  let logger = options.logger.as_ref().unwrap().clone();
  let stats = options.stats.as_ref().unwrap().clone();

  let secret_segment = {
    #[cfg(feature = "secrets")]
    {
      options
        .secret_opts
        .secret_key_path_segment
        .clone()
        .unwrap_or(".c5encval".to_string())
    }
    #[cfg(not(feature = "secrets"))]
    {
      ".c5encval".to_string()
    }
  };

  let data_store = C5DataStore::new(logger.clone(), stats.clone(), secret_segment, secret_key_store.clone());
  let subscriptions = C5StoreSubscriptions::new();
  let root = C5StoreRoot::new(data_store.clone(), subscriptions.clone());
  let change_notifier = Arc::new(ChangeNotifier::new(
    Duration::from_millis(options.change_delay_period.unwrap()),
    data_store.clone(),
    subscriptions.clone(),
  ));

  let set_data_fn = {
    let data_store_clone = data_store.clone();
    let change_notifier_clone = change_notifier.clone();

    Arc::new(move |key: &str, value: C5DataValue| {
      let data_store = data_store_clone.clone();
      let change_notifier = change_notifier_clone.clone();

      // Check *before* setting the data
      let old_value = data_store.get_data(key); // Get current value

      let needs_update = match &old_value {
        Some(ov) => ov != &value, // Update if value differs
        None => true,             // Update if key didn't exist
      };

      if needs_update {
        // Set the data (which might decrypt secrets)
        // Use internal setter to avoid infinite loop if set_data called set_data
        // And pass a relevant source if possible (tricky here)
        let source = ConfigSource::SetProgrammatically; // Or determine source if possible
        let _prev_val = data_store._set_data_internal(key, value.clone(), source); // Use internal setter

        // Notify AFTER setting the data, passing old and new values
        change_notifier.notify_changed(key, old_value, value); // Pass owned values
      }
    })
  };

  let mut provided_data: MultiMap<String, C5DataValue> = MultiMap::new();

  read_config_data(&config_file_paths, &data_store, &mut provided_data)?;

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

#[cfg(feature = "secrets")]
pub fn load_secret_key_files(
  secret_keys_path_str: Option<&PathBuf>,
  secret_key_store: &mut SecretKeyStore,
) -> Result<(), ConfigError> {
  if secret_keys_path_str.is_none() {
    return Ok(());
  }

  let skpath = secret_keys_path_str.unwrap();

  if !skpath.exists() {
    use log::warn;

    warn!("[Secrets] Warning: Secret keys path {:?} does not exist.", skpath);
    return Ok(()); // Don't error if path doesn't exist
  }

  if !skpath.is_dir() {
    return Err(ConfigError::Message(format!(
      "Secret keys path {:?} is not a directory",
      skpath
    )));
  }

  let files = read_dir(skpath).map_err(|e| ConfigError::IoError {
    path: skpath.clone(),
    source: e,
  })?;

  for dir_entry_result in files {
    let dir_entry = dir_entry_result.map_err(|e| ConfigError::IoError {
      path: skpath.clone(),
      source: e,
    })?;
    let entry_path = dir_entry.path();

    if entry_path.is_dir() {
      continue;
    }

    let key_result = fs::read(&entry_path).map_err(|e| ConfigError::IoError {
      path: entry_path.clone(),
      source: e,
    });
    if key_result.is_err() {
      error!(
        "[Secrets] Error reading key file {:?}: {:?}",
        entry_path,
        key_result.err()
      );
      continue; // Skip file on read error? Or return Err? Let's skip for now.
    }
    let mut key = key_result.unwrap();

    let file_ext_os = entry_path.extension();
    let file_name_os = entry_path.file_name();

    if file_ext_os.is_none() || file_name_os.is_none() {
      error!(
        "[Secrets] Skipping file with missing name or extension: {:?}",
        entry_path
      );
      continue;
    }
    let file_ext = file_ext_os.unwrap().to_str().unwrap_or("");
    let file_name = file_name_os.unwrap().to_str().unwrap_or("");

    if file_name.is_empty() || file_name.len() <= file_ext.len() + 1 {
      warn!("[Secrets] Skipping file with invalid name format: {:?}", entry_path);
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
          warn!("[Secrets] Error parsing PEM key file {:?}: {}", entry_path, e);
          continue; // Skip invalid PEM files
        }
      }
    }

    debug!("[Secrets] Loading key '{}' from file {:?}", key_name, entry_path); // Optional log
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
          debug!("[Secrets] Loading key '{}' from env var '{}'", key_name, key); // Optional log
          secret_key_store.set_key(&key_name, key_bytes);
        }
        Err(e) => {
          error!(
            "[Secrets] Error base64 decoding secret key from env var '{}': {}",
            key, e
          );
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
  config_file_paths: &[PathBuf],
  data_store: &C5DataStore, // Expecting the internal data store
  provided_data: &mut MultiMap<String, C5DataValue>,
) -> Result<(), ConfigError> {
  let mut file_config_merged: HashMap<String, C5DataValue> = HashMap::new(); // Holds NESTED structure from files
  let mut files_to_process: Vec<PathBuf> = Vec::new();
  let mut file_source_map: HashMap<String, PathBuf> = HashMap::new(); // Tracks top-level key source file

  // --- 1. Expand directories ---
  for path in config_file_paths {
    if path.is_dir() {
      match read_dir(path) {
        Ok(entries) => {
          let mut dir_files: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|p| p.is_file())
            .collect();
          dir_files.sort();
          files_to_process.extend(dir_files);
        }
        Err(e) => {
          return Err(ConfigError::IoError {
            path: path.clone(),
            source: e,
          });
        }
      }
    } else if path.is_file() {
      files_to_process.push(path.clone());
    } else if path.exists() {
      warn!(
        "[Config] Warning: Path {:?} exists but is not a file or directory.",
        path
      );
    } else {
      // Only warn if it *doesn't* exist
      debug!("[Config] Info: Optional config path {:?} not found.", path);
    }
  }

  // --- 2. Load, Merge Files, and Extract Provider Configs (ONCE) ---
  for file_path in &files_to_process {
    let extension = file_path.extension().and_then(OsStr::to_str);
    type ParserFn = fn(&str, &PathBuf) -> Result<HashMap<String, C5DataValue>, ConfigError>;
    let parser: Option<ParserFn> = match extension {
      Some("yaml") | Some("yml") => Some(|content, path| {
        serde_yaml::from_str::<HashMap<String, serde_yaml::Value>>(content)
          .map_err(|e| ConfigError::YamlParseError {
            path: path.clone(),
            source: e,
          })
          .map(map_from_serde_yaml_valuemap)
      }),
      #[cfg(feature = "toml")]
      Some("toml") => Some(|content, path| {
        toml::from_str::<HashMap<String, toml::Value>>(content)
          .map_err(|e| ConfigError::TomlParseError {
            path: path.clone(),
            source: e,
          })
          .map(map_from_toml_value_map)
      }),
      _ => None,
    };

    if let Some(parse_fn) = parser {
      match fs::read_to_string(&file_path) {
        Ok(content) => {
          match parse_fn(&content, file_path) {
            Ok(mut config_from_file) => {
              // Make mutable
              debug!("[Config] Processing config from file {:?}", file_path);

              // Track file source for top-level keys BEFORE extraction/merging
              for key in config_from_file.keys() {
                file_source_map.insert(key.clone(), file_path.clone());
              }

              // --- >>> Extract Provider Configs from this file's data <<< ---
              // Note: This modifies config_from_file IN PLACE, removing provider sections
              _take_provided_data(&mut config_from_file, provided_data);

              // Merge remaining non-provider file data into the main nested accumulator
              _merge(&mut file_config_merged, &config_from_file);
            }
            Err(e) => return Err(e),
          }
        }
        Err(e) => {
          if e.kind() == std::io::ErrorKind::NotFound {
            warn!("[Config] Warning: File {:?} not found during read.", file_path);
          } else {
            return Err(ConfigError::IoError {
              path: file_path.clone(),
              source: e,
            });
          }
        }
      }
    }
  }
  // `file_config_merged` now holds the merged NESTED, non-provider structure from all files.
  // `provided_data` holds provider configs extracted from files.

  // --- 3. Merge Environment Variables into the Nested Structure ---
  const PREFIX: &str = "C5_";
  const SEPARATOR: &str = "__";
  let mut env_source_flat_map: HashMap<String, ConfigSource> = HashMap::new(); // Tracks flat sources for env vars

  for (env_key_name, value_str) in env::vars() {
    if env_key_name.starts_with(PREFIX) {
      let trimmed_key = env_key_name.trim_start_matches(PREFIX);
      let c5_key = trimmed_key.replace(SEPARATOR, ".").to_lowercase();

      if c5_key.split('.').any(|part| part.is_empty()) {
        warn!(
          "[Config] Warning: Skipping env var '{}' due to invalid key format '{}'",
          env_key_name, c5_key
        );
        continue;
      }

      debug!("[Config] Processing env var '{}' for key '{}'", env_key_name, c5_key);

      // Store flat source info immediately
      env_source_flat_map.insert(c5_key.clone(), ConfigSource::EnvironmentVariable(env_key_name.clone()));

      // Use helper to merge this env var into the nested structure (`file_config_merged`)
      if let Err(e) = merge_env_var_nested(&mut file_config_merged, &c5_key, &value_str) {
        return Err(e); // Propagate conflict errors
      }
    }
  }
  // `file_config_merged` now holds the final combined NESTED structure (Files + Env Vars Merged, non-provider).

  // --- 4. Flatten the Final Nested Structure ---
  let mut final_flat_map = HashMap::new();
  util::build_flat_map(&file_config_merged, &mut final_flat_map, String::new());
  // `final_flat_map` now contains all config keys (e.g., "database.host", "database.port")

  // --- 5. Apply to Store with Correct Sources ---
  for (key, value) in final_flat_map {
    // Determine source: Check env source map first, then file source map
    let final_source = match env_source_flat_map.get(&key) {
      Some(env_source) => env_source.clone(), // Env var took precedence
      None => {
        // Must have come from a file
        let top_level_key = key.split('.').next().unwrap_or(&key);
        file_source_map
          .get(top_level_key)
          .map(|path| ConfigSource::File(path.clone()))
          .unwrap_or(ConfigSource::Unknown) // Fallback
      }
    };
    // Set the flattened key-value pair in the actual data store
    data_store._set_data_internal(&key, value, final_source);
  }

  Ok(())
}

// Helper function to attempt parsing env var strings into C5 types
fn parse_env_var_value(value_str: &str) -> C5DataValue {
  // Try bool
  if value_str.eq_ignore_ascii_case("true") {
    return C5DataValue::Boolean(true);
  }
  if value_str.eq_ignore_ascii_case("false") {
    return C5DataValue::Boolean(false);
  }
  // Try integer (signed first) - use i64 as base
  if let Ok(i) = value_str.parse::<i64>() {
    return C5DataValue::Integer(i);
  }
  // Try unsigned integer - use u64 as base
  if let Ok(u) = value_str.parse::<u64>() {
    // Only use UInteger if it *didn't* parse as i64 (e.g., > i64::MAX)
    // or perhaps prefer UInteger if non-negative? Let's stick to i64 if possible.
    // If parsing as i64 succeeded, we use that. If not, try u64.
    // A check could be added: if u <= i64::MAX as u64, return Integer(u as i64)?
    // For simplicity now, if it parses as u64 *after* failing i64, use UInteger.
    return C5DataValue::UInteger(u);
  }
  // Try float
  if let Ok(f) = value_str.parse::<f64>() {
    return C5DataValue::Float(f);
  }
  // Fallback to string
  C5DataValue::String(value_str.to_string())
}

// Helper to merge a single environment variable into the nested structure
fn merge_env_var_nested(
  target_map: &mut HashMap<String, C5DataValue>,
  c5_key: &str,
  value_str: &str,
) -> Result<(), ConfigError> {
  let mut current_level_map = target_map; // Start with the root map
  let key_parts: Vec<&str> = c5_key.split('.').collect();

  for (i, part) in key_parts.iter().enumerate() {
    if part.is_empty() {
      // Check for invalid empty parts like a..b
      return Err(ConfigError::Message(format!(
        "Invalid key format: Encountered empty segment in env var key '{}'",
        c5_key
      )));
    }

    if i == key_parts.len() - 1 {
      // --- Last part: Insert the final value ---
      // `current_level_map` points to the correct parent map here.
      current_level_map.insert(part.to_string(), parse_env_var_value(value_str));
      return Ok(()); // Done
    } else {
      // --- Intermediate part: Ensure map exists and prepare descent ---
      let entry = current_level_map.entry(part.to_string());

      match entry {
        std::collections::hash_map::Entry::Occupied(occ_entry) => {
          // Entry exists, check if it's a map.
          // We don't need to keep the borrow from occ_entry.
          if !matches!(occ_entry.get(), C5DataValue::Map(_)) {
            // Conflict: Entry exists but isn't a map
            return Err(ConfigError::Message(format!(
              "Env var key conflict: Cannot create nested structure for '{}' because part '{}' conflicts with an existing non-map value.",
              c5_key, part
            )));
          }
          // It is a map, allow occ_entry borrow to expire here.
        }
        std::collections::hash_map::Entry::Vacant(vac_entry) => {
          // Entry doesn't exist, insert a new map.
          vac_entry.insert(C5DataValue::Map(HashMap::new()));
          // The borrow from vac_entry expires here.
        }
      }
      // --- Borrow derived from `entry` ends here ---

      // Now, we are guaranteed that current_level_map[*part] exists and is a Map.
      // Get the mutable reference *from current_level_map* to descend for the *next* iteration.
      // This borrow is valid as it's derived from `current_level_map` itself.
      if let Some(C5DataValue::Map(next_map)) = current_level_map.get_mut(*part) {
        // Update `current_level_map` to point to the nested map for the next loop iteration.
        current_level_map = next_map;
      } else {
        // This case should be impossible if the match logic above is correct.
        unreachable!(
          "Map for part '{}' should exist here but wasn't found or wasn't a Map",
          part
        );
      }
    } // end intermediate part
  } // end loop

  // This point should be unreachable because the last part is handled inside the loop.
  unreachable!("Loop should handle all parts or return early");
}

// Helper to recursively merge hashmaps, src overwrites dest
// Ensures nested maps are merged correctly.
fn _merge(dest: &mut HashMap<String, C5DataValue>, src: &HashMap<String, C5DataValue>) {
  for (src_key, src_value) in src.iter() {
    // Use iter()
    match dest.entry(src_key.clone()) {
      // Use entry API
      std::collections::hash_map::Entry::Occupied(mut entry) => {
        // Key exists in destination, get mutable ref to existing value
        let dest_val = entry.get_mut();
        // Check if both are maps
        if let (C5DataValue::Map(dest_map), C5DataValue::Map(src_map)) = (dest_val, src_value) {
          // Both are maps, recurse
          _merge(dest_map, src_map);
        } else {
          // Not both maps (or different types), source overwrites destination value
          // This handles cases like: dest=Map, src=String -> dest becomes String
          // And: dest=String, src=Map -> dest becomes Map
          *entry.into_mut() = src_value.clone(); // Use entry.into_mut() for direct replacement
        }
      }
      std::collections::hash_map::Entry::Vacant(entry) => {
        // Key doesn't exist in destination, insert clone from source
        entry.insert(src_value.clone());
      }
    }
  }
}

// fn _merge(dest: &mut HashMap<String, C5DataValue>, src: &HashMap<String, C5DataValue>) {
//   for (src_key, src_value) in src.iter() {
//     // Check if the key exists in the destination and if both values are maps.
//     if let Some(C5DataValue::Map(dest_map)) = dest.get_mut(src_key) {
//       if let C5DataValue::Map(src_map) = src_value {
//         // Both are maps, so recurse to merge them deeply.
//         _merge(dest_map, src_map);
//         // Continue to the next key, skipping the overwrite below.
//         continue;
//       }
//     }

//     // If the key doesn't exist in the destination, OR if one of the values
//     // is not a map (e.g., an array, string, number), then the source value
//     // completely overwrites the destination value.
//     dest.insert(src_key.clone(), src_value.clone());
//   }
// }

// Helper to extract provider configurations (no changes needed inside, just signature)
fn _take_provided_data(
  raw_config_data: &mut HashMap<String, C5DataValue>,
  provided_data: &mut MultiMap<String, C5DataValue>,
) {
  _take_provided_data_helper(raw_config_data, provided_data, String::new());
}

// Recursive helper for _take_provided_data (no changes needed)
fn _take_provided_data_helper(
  current_map: &mut HashMap<String, C5DataValue>,
  provided_data: &mut MultiMap<String, C5DataValue>,
  current_keypath: String,
) {
  let keys: Vec<String> = current_map.keys().cloned().collect();

  for key in keys {
    let new_keypath = if current_keypath.is_empty() {
      key.clone()
    } else {
      format!("{}.{}", current_keypath, key)
    };

    let is_provider_config = if let Some(C5DataValue::Map(data_map)) = current_map.get(&key) {
      data_map.contains_key(CONFIG_KEY_PROVIDER)
    } else {
      false
    };

    if is_provider_config {
      if let Some(C5DataValue::Map(mut data_map)) = current_map.remove(&key) {
        data_map.insert(CONFIG_KEY_KEYPATH.to_string(), C5DataValue::String(new_keypath.clone()));
        data_map.insert(CONFIG_KEY_KEYNAME.to_string(), C5DataValue::String(key.clone()));
        if let Some(C5DataValue::String(provider_name)) = data_map.get(CONFIG_KEY_PROVIDER) {
          provided_data.insert(provider_name.clone(), C5DataValue::Map(data_map));
        } else {
          warn!(
            "[Config] Error: Provider config at '{}' has non-string value for '.provider'",
            new_keypath
          );
        }
      }
    } else if let Some(C5DataValue::Map(sub_map)) = current_map.get_mut(&key) {
      _take_provided_data_helper(sub_map, provided_data, new_keypath);
      if sub_map.is_empty() {
        current_map.remove(&key);
      }
    }
  }
}

pub fn default_config_paths(config_dir: &str, release_env: &str, env: &str, region: &str) -> Vec<PathBuf> {
  let mut paths = vec![];

  paths.push(PathBuf::from(format!("{}/common.yaml", config_dir)));
  paths.push(PathBuf::from(format!("{}/{}.yaml", config_dir, release_env).as_str()));
  paths.push(PathBuf::from(format!("{}/{}.yaml", config_dir, env).as_str()));
  paths.push(PathBuf::from(format!("{}/{}.yaml", config_dir, region).as_str()));
  paths.push(PathBuf::from(
    format!("{}/{}-{}.yaml", config_dir, env, region).as_str(),
  ));

  return paths;
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;
  use std::env;
  use std::fs::File;
  use std::io::Write;
  use std::path::PathBuf;

  use log::info;
  use serde::Deserialize;
  use serial_test::serial;

  use crate::C5Store;
  use crate::error::ConfigError;
  use crate::secrets::{Base64SecretDecryptor, SecretKeyStore};
  use crate::value::C5DataValue;
  use crate::{C5StoreMgr, C5StoreOptions, SecretOptions, create_c5store, default_config_paths};

  // Helper struct for get_into_struct tests
  #[derive(Deserialize, Debug, PartialEq)]
  struct DbConfig {
    host: String,
    port: u16,
    user: Option<String>, // Make fields optional if they might not exist
    #[serde(default)] // Example: provide default for missing fields
    timeout: u32,
  }

  #[derive(Deserialize, Debug, PartialEq)]
  struct FeatureFlags {
    new_dashboard: bool,
    api_v2: bool,
    #[serde(default = "default_retries")]
    retries: u8,
  }

  fn default_retries() -> u8 {
    3
  }

  fn _create_c5store_test() -> (impl C5Store, C5StoreMgr) {
    init_logger();
    let config_file_paths = default_config_paths("configs/test/config", "development", "local", "private");
    create_c5store(config_file_paths, None).expect("Test store creation failed")
  }

  use std::sync::Once;

  static INIT: Once = Once::new();

  /// Initializes the logger for tests. This function is safe to call multiple times,
  /// but it will only initialize the logger on the first call.
  fn init_logger() {
    // The `call_once` method ensures that the closure is executed at most once,
    // even if `init_logger` is called from multiple test threads.
    INIT.call_once(|| {
      env_logger::builder()
        // .is_test(true) formats the output for tests and directs it to stderr
        .is_test(true)
        .filter_level(log::LevelFilter::Trace)
        // .try_init() returns an error if the logger is already initialized,
        // which `Once` should prevent. .ok() silently ignores the error.
        .try_init()
        .ok();
    });
  }

  #[test]
  #[serial]
  fn test_config_contains_bill_bar_existence() {
    let (c5store, _c5store_mgr) = _create_c5store_test();

    assert_eq!(c5store.exists("bill.barr"), true);
    assert_eq!(c5store.exists("bill"), false);
    assert_eq!(c5store.path_exists("bill.barr"), true);
    assert_eq!(c5store.path_exists("bill.barr."), false);
    assert_eq!(c5store.path_exists("bill"), true);
  }

  #[test]
  #[serial]
  fn test_config_contains_bill_bar() {
    let (c5store, _c5store_mgr) = _create_c5store_test();

    assert_eq!(
      c5store.get("bill.barr").unwrap(),
      C5DataValue::String(String::from("AG"))
    );
  }

  #[test]
  #[serial]
  fn test_config_contains_example_test_and() {
    let (c5store, _c5store_mgr) = _create_c5store_test();

    assert_eq!(c5store.get("example.test.and").unwrap(), C5DataValue::UInteger(1));
    assert_eq!(c5store.get_into::<u64>("example.test.and").unwrap(), 1u64);
  }

  #[test]
  #[serial]
  fn test_get_into_struct_nested() {
    // Uses the standard config files which have a nested structure
    let (c5store, _c5store_mgr) = _create_c5store_test();

    // Assuming DbConfig struct is defined as above
    let db_conf_res = c5store.get_into_struct::<DbConfig>("database");

    assert!(
      db_conf_res.is_ok(),
      "Failed to deserialize DbConfig: {:?}",
      db_conf_res.err()
    );
    let db_conf = db_conf_res.unwrap();

    assert_eq!(db_conf.host, "db.local.com"); // from local.yaml
    assert_eq!(db_conf.port, 5433); // from local.yaml
    assert_eq!(db_conf.user, Some("local_user".to_string())); // from local.yaml
    assert_eq!(db_conf.timeout, 0); // uses serde default
  }

  #[test]
  #[serial]
  fn test_get_into_struct_flattened() {
    unsafe {
      // Create a store specifically with flattened keys
      env::set_var("C5_FLATDB__HOST", "flat-host.com");
      env::set_var("C5_FLATDB__PORT", "9999");
      env::set_var("C5_FLATDB__USER", "flat_user");
      env::set_var("C5_FLATDB__TIMEOUT", "5000"); // Env vars are strings
    }

    // Use an empty config file path list, relying only on env vars
    let (c5store, _c5store_mgr) = create_c5store(vec![], None).expect("Store creation from env failed");

    let db_conf_res = c5store.get_into_struct::<DbConfig>("flatdb"); // Use lowercase prefix

    assert!(
      db_conf_res.is_ok(),
      "Failed to deserialize flattened DbConfig: {:?}",
      db_conf_res.err()
    );
    let db_conf = db_conf_res.unwrap();

    assert_eq!(db_conf.host, "flat-host.com");
    // Note: Serde handles string-to-number conversion for basic types if possible
    assert_eq!(db_conf.port, 9999);
    assert_eq!(db_conf.user, Some("flat_user".to_string()));
    assert_eq!(db_conf.timeout, 5000);

    unsafe {
      // Clean up env vars
      env::remove_var("C5_FLATDB__HOST");
      env::remove_var("C5_FLATDB__PORT");
      env::remove_var("C5_FLATDB__USER");
      env::remove_var("C5_FLATDB__TIMEOUT");
    }
  }

  #[test]
  #[serial]
  fn test_get_into_struct_partial_flattened() {
    unsafe {
      // Mix flattened env vars with file values
      env::set_var("C5_DATABASE__HOST", "env-host.com"); // Override host from file
    }

    let (c5store, _c5store_mgr) = _create_c5store_test();

    let db_conf_res = c5store.get_into_struct::<DbConfig>("database");

    assert!(
      db_conf_res.is_ok(),
      "Failed to deserialize partially flattened DbConfig: {:?}",
      db_conf_res.err()
    );
    let db_conf = db_conf_res.unwrap();

    assert_eq!(db_conf.host, "env-host.com"); // Env var overrides file
    assert_eq!(db_conf.port, 5433); // From local.yaml
    assert_eq!(db_conf.user, Some("local_user".to_string())); // From local.yaml
    assert_eq!(db_conf.timeout, 0); // default

    unsafe {
      env::remove_var("C5_DATABASE__HOST");
    }
  }

  #[test]
  #[serial]
  fn test_get_into_struct_array_inference() {
    unsafe {
      // Test reconstruction of arrays from numeric keys
      env::set_var("C5_WEB__SERVERS__0__IP", "1.1.1.1");
      env::set_var("C5_WEB__SERVERS__0__PORT", "80");
      env::set_var("C5_WEB__SERVERS__1__IP", "2.2.2.2");
      env::set_var("C5_WEB__SERVERS__1__PORT", "8080");
      env::set_var("C5_WEB__LOADBALANCER", "lb.site.com");
    }

    #[derive(Deserialize, Debug, PartialEq)]
    struct Server {
      ip: String,
      port: u16,
    }
    #[derive(Deserialize, Debug, PartialEq)]
    struct WebConfig {
      servers: Vec<Server>,
      loadbalancer: String,
    }

    let (c5store, _c5store_mgr) = create_c5store(vec![], None).expect("Store creation failed");

    let web_conf_res = c5store.get_into_struct::<WebConfig>("web");

    assert!(
      web_conf_res.is_ok(),
      "Failed to deserialize WebConfig: {:?}",
      web_conf_res.err()
    );
    let web_conf = web_conf_res.unwrap();

    assert_eq!(web_conf.loadbalancer, "lb.site.com");
    assert_eq!(web_conf.servers.len(), 2);
    assert_eq!(
      web_conf.servers[0],
      Server {
        ip: "1.1.1.1".to_string(),
        port: 80
      }
    );
    assert_eq!(
      web_conf.servers[1],
      Server {
        ip: "2.2.2.2".to_string(),
        port: 8080
      }
    );

    unsafe {
      env::remove_var("C5_WEB__SERVERS__0__IP");
      env::remove_var("C5_WEB__SERVERS__0__PORT");
      env::remove_var("C5_WEB__SERVERS__1__IP");
      env::remove_var("C5_WEB__SERVERS__1__PORT");
      env::remove_var("C5_WEB__LOADBALANCER");
    }
  }

  #[test]
  #[serial]
  fn test_get_into_struct_key_not_found() {
    let (c5store, _c5store_mgr) = _create_c5store_test();
    let res = c5store.get_into_struct::<DbConfig>("non_existent_prefix");
    assert!(matches!(res, Err(ConfigError::KeyNotFound(_))));
  }

  #[test]
  #[serial]
  fn test_get_into_struct_deserialization_error() {
    unsafe {
      // Set env vars that won't deserialize correctly into FeatureFlags (e.g., string for bool)
      env::set_var("C5_FEATURES__NEW_DASHBOARD", "maybe");
      env::set_var("C5_FEATURES__API_V2", "false");
    }

    let (c5store, _c5store_mgr) = create_c5store(vec![], None).expect("Store creation failed");

    let res = c5store.get_into_struct::<FeatureFlags>("features");
    assert!(
      match &res {
        Err(ConfigError::ConversionError { key, message }) => {
          // The key from C5SerdeValueDeserializer is often empty or the direct field name.
          // The message should be specific.
          (key.is_empty() || key == "features" || key == "features.new_dashboard")
            && message.contains("'maybe' could not be converted to boolean")
        }
        _ => false,
      },
      "Expected ConversionError for 'maybe' string with specific message, got {:?}",
      res
    );

    unsafe {
      env::remove_var("C5_FEATURES__NEW_DASHBOARD");
      env::remove_var("C5_FEATURES__API_V2");
    }
  }

  #[test]
  #[serial]
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
        secret_key_store.set_decryptor(
          "ecies_x25519",
          Box::from(EciesX25519SecretDecryptor::new(EciesX25519::new())),
        );
      })),
      load_secret_keys_from_env: false,
      secret_key_env_prefix: None,
      ..Default::default()
    };

    let (c5store, _c5store_mgr) =
      create_c5store(config_file_paths, Some(config_opt)).expect("Secrets test store creation failed");

    assert_eq!(
      c5store.get("a_secret").unwrap(),
      C5DataValue::Bytes("abcd".as_bytes().to_vec())
    );
    assert_eq!(
      c5store.get("hello_secret").unwrap(),
      C5DataValue::Bytes("Hello World".as_bytes().to_vec())
    );
  }

  #[test]
  #[serial]
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
        secret_key_store.set_decryptor(
          "ecies_x25519",
          Box::from(EciesX25519SecretDecryptor::new(EciesX25519::new())),
        );
      })),
      load_secret_keys_from_env: false,
      secret_key_env_prefix: None,
      ..Default::default()
    };

    let (c5store, _c5store_mgr) =
      create_c5store(config_file_paths, Some(config_opt)).expect("Bad secrets test store creation failed");

    assert_eq!(c5store.get("bad_secret"), Some(C5DataValue::Null));
  }

  #[test]
  #[serial]
  #[cfg(feature = "secrets")]
  fn test_decryption_pipeline_populates_store_correctly() {
    // --- STAGE 1: Test that the full file->decrypt->store pipeline works ---

    info!("\n--- TEST: Verifying decryption pipeline populates the store ---");

    // 1. Configure the store with the real decryptor
    let mut options = C5StoreOptions::default();
    options.secret_opts.secret_key_store_configure_fn = Some(Box::new(|store| {
      store.set_decryptor("base64", Box::new(Base64SecretDecryptor {}));
      store.set_key("dummy_key", vec![1, 2, 3]);
    }));

    // 2. Load the store from the correctly formatted test file
    let config_path = PathBuf::from("resources/test_e2e_secrets.yaml");
    let (c5store, _mgr) = create_c5store(vec![config_path], Some(options)).expect("Store creation failed");

    // 3. Assert the final state of the store after decryption
    info!("\n--- Asserting final store state ---");

    // Assert plaintext values were loaded
    assert_eq!(
      c5store.get("database.host").unwrap(),
      C5DataValue::String("db.prod.com".to_string())
    );

    // Assert that the DECRYPTED values are in the store with the correct type (Bytes)
    assert_eq!(
      c5store.get("secrets.api_key").unwrap(),
      C5DataValue::Bytes("secret-key-123".as_bytes().to_vec())
    );
    assert_eq!(
      c5store.get("secrets.app_id").unwrap(),
      C5DataValue::Bytes(55_u32.to_be_bytes().to_vec())
    );
    assert_eq!(
      c5store.get("secrets.timeout").unwrap(),
      C5DataValue::Bytes(2.0_f64.to_be_bytes().to_vec())
    );
    assert_eq!(
      c5store.get("secrets.raw_key").unwrap(),
      C5DataValue::Bytes("byte-data".as_bytes().to_vec())
    );

    // Assert that the ORIGINAL ENCRYPTED VALUES ARE GONE
    assert!(!c5store.exists("secrets.api_key.c5encval"));

    info!("✅ Stage 1 Passed: Store is populated correctly from decrypted secrets.");
  }

  #[test]
  #[serial]
  #[cfg(feature = "secrets")]
  fn test_end_to_end_deserialization_with_secrets() {
    use crate::secrets::Base64SecretDecryptor;

    // --- 1. Define the Target Structs ---
    #[derive(Deserialize, Debug, PartialEq)]
    struct FullConfig {
      database: DatabaseConfig,
      secrets: SecretsConfig,
    }
    #[derive(Deserialize, Debug, PartialEq)]
    struct DatabaseConfig {
      host: String,
      port: u16,
    }
    #[derive(Deserialize, Debug, PartialEq)]
    struct SecretsConfig {
      api_key: String,
      app_id: u32,
      timeout: f64,
      raw_key: Vec<u8>,
    }

    // --- 2. Configure C5StoreOptions with the REAL Base64SecretDecryptor ---
    let mut options = C5StoreOptions::default();
    options.secret_opts.secret_key_store_configure_fn = Some(Box::new(|store| {
      store.set_decryptor("base64", Box::new(Base64SecretDecryptor {}));
      store.set_key("dummy_key", vec![1, 2, 3]);
    }));

    // --- 3. Load the Store from our correctly formatted test file ---
    let config_path = PathBuf::from("resources/test_e2e_secrets.yaml");
    let (c5store, _mgr) = create_c5store(vec![config_path], Some(options)).expect("Store creation failed");

    // --- 4. Perform Deserialization and Assertions ---
    let config = c5store
      .get_into_struct::<FullConfig>("")
      .expect("Deserialization failed");

    // Assert plaintext values are correct
    assert_eq!(config.database.host, "db.prod.com");
    assert_eq!(config.database.port, 5432);

    // Assert that all secrets were decrypted and deserialized correctly
    assert_eq!(config.secrets.api_key, "secret-key-123");
    assert_eq!(config.secrets.app_id, 55);
    assert_eq!(config.secrets.timeout, 2.0);
    assert_eq!(config.secrets.raw_key, "byte-data".as_bytes());
  }

  #[test]
  #[serial]
  #[cfg(feature = "secrets")]
  fn test_get_into_string_from_decrypted_bytes() {
    // --- 1. Prepare Test Configuration ---
    // The expected string is "Hello, Secret World!"
    // Its base64 representation is "SGVsbG8sIFNlY3JldCBXb3JsZCE="
    //
    // For the invalid UTF-8 test, we use the byte sequence [0xC3, 0x28],
    // which is an invalid 2-byte UTF-8 sequence. Its base64 is "wyg="
    let config_content = r#"
my_secret_string:
  ".c5encval":
    - "base64"
    - "test_key"
    - "SGVsbG8sIFNlY3JldCBXb3JsZCE="

my_bad_utf8_secret:
  ".c5encval":
    - "base64"
    - "test_key"
    - "wyg="
"#;

    let mut temp_config_file = tempfile::Builder::new()
      .prefix("c5store-test-")
      .suffix(".yaml")
      .tempfile()
      .unwrap();
    write!(temp_config_file, "{}", config_content).unwrap();

    // Read the file's content directly from the disk to verify it.
    let file_path = temp_config_file.path();
    let content_on_disk = std::fs::read_to_string(file_path).unwrap();
    assert_eq!(
      content_on_disk, config_content,
      "The content on disk did not match the expected content!"
    );

    let config_path = temp_config_file.path().to_path_buf();

    // --- 2. Configure C5Store for Secrets ---
    let mut options = C5StoreOptions::default();
    options.secret_opts.secret_key_store_configure_fn = Some(Box::new(|store| {
      // Use a simple decryptor that just decodes base64
      store.set_decryptor("base64", Box::new(Base64SecretDecryptor {}));
      // Key content doesn't matter for this decryptor, but it must exist
      store.set_key("test_key", vec![]);
    }));

    // --- 3. Create the Store ---
    let (c5store, _mgr) = create_c5store(vec![config_path], Some(options)).expect("Store creation for test failed");

    // --- 4. Test the Success Case (Valid UTF-8) ---
    let result = c5store.get_into::<String>("my_secret_string");

    assert!(
      result.is_ok(),
      "get_into::<String> failed for valid UTF-8 bytes: {:?}",
      result.err()
    );
    let secret_string = result.unwrap();
    assert_eq!(secret_string, "Hello, Secret World!");

    // --- 5. Test the Failure Case (Invalid UTF-8) ---
    let bad_result = c5store.get_into::<String>("my_bad_utf8_secret");

    assert!(
      bad_result.is_err(),
      "get_into::<String> should have failed for invalid UTF-8 bytes"
    );
    assert!(
      matches!(bad_result, Err(ConfigError::ConversionError { .. })),
      "Expected a ConversionError for invalid UTF-8, but got {:?}",
      bad_result
    );
  }

  // In c5store_rust/src/lib.rs -> mod tests { ... }

  #[test]
  #[serial]
  fn test_array_overwrite_during_merge() {
    // --- 1. Prepare Test Configuration Files ---
    let config1_content = r#"
    test:
      key1:
        key1_2: []
      key2: "from config1"
    "#;
    let config2_content = r#"
    test:
      key1:
        key1_2:
        - "a"
        - "b"
      key3: "from config2"
    "#;

    let mut file1 = tempfile::Builder::new().suffix(".yaml").tempfile().unwrap();
    write!(file1, "{}", config1_content).unwrap();
    file1.flush().unwrap();

    let mut file2 = tempfile::Builder::new().suffix(".yaml").tempfile().unwrap();
    write!(file2, "{}", config2_content).unwrap();
    file2.flush().unwrap();

    // The order is important: file2 should overwrite file1
    let config_paths = vec![file1.path().to_path_buf(), file2.path().to_path_buf()];

    // --- 2. Create the Store ---
    let (c5store, _mgr) = create_c5store(config_paths, None).expect("Store creation failed");

    // --- 3. Assert Final State ---

    // Assert that the empty array was correctly overwritten by the full one.
    let expected_array = C5DataValue::Array(vec![
      C5DataValue::String("a".to_string()),
      C5DataValue::String("b".to_string()),
    ]);
    assert_eq!(
      c5store.get("test.key1.key1_2").unwrap(),
      expected_array,
      "The empty array from the first file was not overwritten."
    );

    // Assert that other keys were merged correctly.
    assert_eq!(
      c5store.get("test.key2").unwrap(),
      C5DataValue::String("from config1".to_string()),
      "Key only present in the first file should exist."
    );
    assert_eq!(
      c5store.get("test.key3").unwrap(),
      C5DataValue::String("from config2".to_string()),
      "Key only present in the second file should exist."
    );
  }

  #[test]
  #[serial]
  #[cfg(feature = "secrets")]
  fn test_array_of_objects_overwrite_with_secrets() {
    // --- 1. Define Target Structs for Deserialization ---
    #[derive(Deserialize, Debug, PartialEq)]
    struct Endpoint {
      name: String,
      api_key: String, // Target is a String, will require Bytes -> String conversion
    }

    #[derive(Deserialize, Debug, PartialEq)]
    struct ServicesConfig {
      endpoints: Vec<Endpoint>,
    }

    // --- 2. Prepare Test Configuration Files ---
    // Config 1 has an empty array. This will be overwritten.
    let config1_content = r#"
services:
  endpoints: []
"#;

    // Config 2 has a full array of objects, one of which contains a secret.
    // The secret "super-secret-auth-key" is base64 encoded as "c3VwZXItc2VjcmV0LWF1dGgta2V5"
    let config2_content = r#"
services:
  endpoints:
    - name: "user-service"
      api_key: "plain-key-123"
    - name: "auth-service"
      api_key:
        .c5encval:
        - "base64"
        - "test_key"
        - "c3VwZXItc2VjcmV0LWF1dGgta2V5"
"#;

    // Create temporary files with .yaml extension
    let mut file1 = tempfile::Builder::new().suffix(".yaml").tempfile().unwrap();
    write!(file1, "{}", config1_content).unwrap();
    file1.flush().unwrap();

    let mut file2 = tempfile::Builder::new().suffix(".yaml").tempfile().unwrap();
    write!(file2, "{}", config2_content).unwrap();
    file2.flush().unwrap();

    // The order is important: file2 should overwrite file1
    let config_paths = vec![file1.path().to_path_buf(), file2.path().to_path_buf()];

    // --- 3. Configure C5Store for Secrets ---
    let mut options = C5StoreOptions::default();
    options.secret_opts.secret_key_store_configure_fn = Some(Box::new(|store| {
      store.set_decryptor("base64", Box::new(Base64SecretDecryptor {}));
      store.set_key("test_key", vec![]);
    }));

    // --- 4. Create the Store ---
    let (c5store, _mgr) = create_c5store(config_paths, Some(options)).expect("Store creation failed");

    // --- 5. Perform Deserialization and Assertions ---
    let result = c5store.get_into_struct::<ServicesConfig>("services");

    assert!(
      result.is_ok(),
      "Failed to deserialize ServicesConfig: {:?}",
      result.err()
    );
    let config = result.unwrap();

    // Define the final, expected state of the struct after merging and decryption.
    let expected_config = ServicesConfig {
      endpoints: vec![
        Endpoint {
          name: "user-service".to_string(),
          api_key: "plain-key-123".to_string(),
        },
        Endpoint {
          name: "auth-service".to_string(),
          api_key: "super-secret-auth-key".to_string(), // The decrypted value
        },
      ],
    };

    // Assert that the final struct matches the expected state.
    assert_eq!(config, expected_config);
  }

  // Add these to your `use` statements at the top of the test module
  use crate::providers::C5FileValueProvider;
  use tempfile::{NamedTempFile, tempdir};

  // The struct definitions from step 1 go here...
  #[derive(Deserialize, Debug, PartialEq)]
  #[serde(rename_all = "camelCase")]
  struct CommodityWeights {
    #[serde(flatten)]
    weights: HashMap<u32, u32>,
  }

  #[derive(Deserialize, Debug, PartialEq)]
  #[serde(rename_all = "camelCase")]
  struct Sector {
    id: u32,
    commodity_weights: CommodityWeights,
  }

  #[derive(Deserialize, Debug, PartialEq)]
  struct RegionData {
    region: u32,
    sectors: Vec<Sector>,
  }

  #[test]
  #[serial]
  fn test_get_into_struct_from_file_provider_with_root_array() {
    init_logger();

    // --- 1. Create a controlled temporary directory for all test files ---
    let temp_dir = tempdir().expect("Failed to create temp directory");
    let base_path = temp_dir.path();

    // --- 2. Prepare the data file inside the temp directory ---
    let data_yaml_content = r#"
- region: 2198
  sectors: 
  - id: 1
    commodityWeights:
      120204: 225
      120235: 665
- region: 2199
  sectors:
  - id: 2
    commodityWeights:
      120877: 75
"#;
    let data_file_path = base_path.join("data_to_load.yaml");
    let mut data_file = File::create(&data_file_path).unwrap();
    write!(data_file, "{}", data_yaml_content).unwrap();

    // --- 3. Prepare the main config file, using a RELATIVE path ---
    // The provider will combine its base_path with this relative path.
    let main_config_content = r#"
market:
  regions:
    .provider: "resources"
    path: "data_to_load.yaml"  # <-- Use the simple relative path
    format: "yaml"
"#;
    let main_config_path = base_path.join("main_config.yaml");
    let mut main_config_file = File::create(&main_config_path).unwrap();
    write!(main_config_file, "{}", main_config_content).unwrap();

    // The files are flushed and closed when `data_file` and `main_config_file` go out of scope here.
    // This is more reliable than relying on an active handle.

    // --- 4. Initialize C5Store from the main config file ---
    let (c5store, mut c5store_mgr) = create_c5store(
      vec![main_config_path], // Load from the main config
      None,
    )
    .expect("Test store creation failed");

    // Register the C5FileValueProvider. The base path doesn't matter here since
    // we provided an absolute path in the config.
    c5store_mgr.set_value_provider(
      "resources",
      C5FileValueProvider::default(base_path.to_str().unwrap()), // Base path is our temp dir
      0,
    );

    // --- 4. Perform Deserialization and Assertions ---
    // The key is "market.regions", which is where the provider placed the array.
    // The target type is Vec<RegionData> because the root of the data file is an array.
    let result = c5store.get_into_struct::<Vec<RegionData>>("market.regions");

    // Assert that the operation was successful
    assert!(
      result.is_ok(),
      "Failed to deserialize struct from file provider: {:?}",
      result.err()
    );

    let regions = result.unwrap();

    // Assert the content is correct
    assert_eq!(regions.len(), 2, "Should have loaded two regions from the array");

    // Check the first region
    assert_eq!(regions[0].region, 2198);
    assert_eq!(regions[0].sectors.len(), 1);
    assert_eq!(regions[0].sectors[0].id, 1);
    assert_eq!(
      regions[0].sectors[0].commodity_weights.weights.get(&120235),
      Some(&665)
    );

    // Check the second region
    assert_eq!(regions[1].region, 2199);
    assert_eq!(regions[1].sectors[0].commodity_weights.weights.get(&120877), Some(&75));
  }
}
