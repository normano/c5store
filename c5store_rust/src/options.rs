#[cfg(feature = "secrets")]
use std::path::PathBuf;
use std::{
  collections::HashMap,
  env,
  ffi::OsStr,
  fs::{self, read_dir},
  sync::Arc,
};

use log::{debug, warn};
use multimap::MultiMap;

#[cfg(feature = "secrets")]
use crate::secrets::{SecretKeyStore, systemd::SystemdCredential};
#[cfg(feature = "toml")]
use crate::serialization::map_from_toml_value_map;
use crate::{config_source::ConfigSource, serialization::map_from_serde_yaml_valuemap, util};
use crate::{
  error::ConfigError,
  internal::C5DataStore,
  telemetry::{Logger, StatsRecorder},
  value::C5DataValue,
};
use crate::{
  providers::{CONFIG_KEY_KEYNAME, CONFIG_KEY_KEYPATH, CONFIG_KEY_PROVIDER},
  util::convert_case,
};

pub(crate) const DEFAULT_CHANGE_DELAY_PERIOD: u64 = 500;

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

/// Defines the case style to apply when converting environment variables to config keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Case {
  /// Converts `VAR_NAME` to `varName`. Recommended for use with `serde(rename_all = "camelCase")`.
  Camel,
  /// Converts `VAR_NAME` to `var_name`. Recommended for use with `serde(rename_all = "snake_case")`.
  Snake,
  /// Converts `VAR_NAME` to `var-name`. Recommended for use with `serde(rename_all = "kebab-case")`.
  Kebab,
  /// Converts `VAR_NAME` to `varname`. The original, simple lowercasing behavior.
  Lower,
}

pub struct C5StoreOptions {
  pub logger: Option<Arc<dyn Logger>>,
  pub stats: Option<Arc<dyn StatsRecorder>>,
  pub change_delay_period: Option<u64>,
  pub secret_opts: SecretOptions,
  /// The case style to use for environment variable keys. Defaults to `Case::Camel`.
  pub env_case: Case,
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
      env_case: Case::Camel, // New default for better serde interop
      #[cfg(feature = "dotenv")]
      dotenv_path: None,
    };
  }
}

// Reads configuration from specified paths (files/directories), merges them,
/// applies environment variable overrides, separates provider configurations,
/// and applies the final values to the store via the provided setter function.
///
/// Handles YAML and TOML file formats. Reads environment variables starting
/// with "C5_" using "__" as a separator (e.g., C5_DATABASE__HOST becomes database.host).
///
/// Order of precedence: Environment Variables > Last File Read > First File Read.
pub(crate) fn read_config_data(
  config_file_paths: &[PathBuf],
  data_store: &C5DataStore,
  provided_data: &mut MultiMap<String, C5DataValue>,
  env_case: Case,
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

      // New logic: Split by path separator, convert case for each part, then join.
      let c5_key = trimmed_key
        .split(SEPARATOR)
        .map(|part| convert_case(part, env_case))
        .collect::<Vec<String>>()
        .join(".");

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
