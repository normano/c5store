## C5Store - Quick API Reference (v0.3.0+)

This document provides a concise overview of the primary public API components for using the `c5store` library (version 0.3.0 and later).

### Initialization

*   **`create_c5store(config_paths: Vec<PathBuf>, options: Option<C5StoreOptions>) -> Result<(impl C5Store, C5StoreMgr), ConfigError>`**
    *   The main entry point. Loads/merges config from files (YAML/TOML), directories, and environment variables (`C5_*`). See README for priority. Applies options.
    *   **Changed:** Returns `Result` for error handling during load.
    *   Returns `Ok((store, manager))` on success.

*   **`struct C5StoreOptions`**
    *   Configures store behavior. Passed to `create_c5store`.
    *   Fields: `logger`, `stats`, `change_delay_period`, `secret_opts: SecretOptions`.
    *   **New (`dotenv` feature):** `dotenv_path: Option<PathBuf>` - Optional path to a `.env` file to load before reading process environment variables.
    *   Use `C5StoreOptions::default()`.

*   **`struct SecretOptions` (`secrets` feature)**
    *   Configures secrets management (part of `C5StoreOptions`). Requires `secrets` feature (default: on).
    *   Key Fields:
        *   `secret_key_path_segment: Option<String>` (Default: `".c5encval"`) - Key indicating encrypted value.
        *   `secret_keys_path: Option<PathBuf>` - Path to directory of decryption key files.
        *   `secret_key_store_configure_fn: Option<Box<dyn FnMut(&mut SecretKeyStore)>>` - Closure for programmatic key/decryptor setup.
        *   **New:** `load_secret_keys_from_env: bool` (Default: `false`) - Load keys from env vars?
        *   **New:** `secret_key_env_prefix: Option<String>` (Default: `"C5_SECRETKEY_"`) - Prefix for env vars with base64 keys.

### Error Handling

*   **`enum ConfigError`**
    *   Error type returned by fallible operations. Use `thiserror::Error` for details.
    *   Variants cover: `KeyNotFound`, `TypeMismatch`, `ConversionError`, `DeserializationError`, `IoError`, `YamlParseError`, `TomlParseError` (`toml` feature), `DotEnvLoadError` (`dotenv` feature), `SecretKeyNotFound` (`secrets` feature), `DecryptionError` (`secrets` feature), etc.

### Core Store Access (`C5Store` Trait)

The primary interface for configuration access.

*   **`fn get(&self, key_path: &str) -> Option<C5DataValue>`**
    *   Retrieves the raw `C5DataValue` for a key path. Returns `None` if not found.

*   **`fn get_ref(&self, key_path: &str) -> Option<C5StoreDataValueRef>`**
    *   Gets a temporary reference wrapper (`C5StoreDataValueRef`) containing references to the value and its source, avoiding cloning. Returns `None` if not found. Use `.value()` or `.source()` on the result.

*   **`fn get_into<T>(&self, key_path: &str) -> Result<T, ConfigError>`**
    *   **Changed:** Retrieves and attempts conversion to type `T`. Returns `Result`.
    *   Requires `C5DataValue: TryInto<T, Error = ConfigError>`.
    *   Returns `Ok(T)` or `ConfigError` (e.g., `KeyNotFound`, `TypeMismatch`).

*   **`fn get_into_struct<'de, T>(&self, key_path: &str) -> Result<T, ConfigError>`**
    *   **New:** Deserializes a branch into struct `T` (requires `serde::Deserialize`).
    *   Returns `Ok(T)` or `ConfigError` (e.g., `KeyNotFound`, `DeserializationError`).

*   **`fn exists(&self, key_path: &str) -> bool`**
    *   Checks for an *exact* key path match.

*   **`fn path_exists(&self, key_path: &str) -> bool`**
    *   Checks if the path exists exactly or as a prefix for other keys.

*   **`fn branch(&self, key_path: &str) -> C5StoreBranch`**
    *   Creates a branched view rooted at `key_path`.

*   **`fn key_paths_with_prefix(&self, key_path: Option<&str>) -> Vec<String>`**
    *   Lists keys starting with the given prefix (relative to branch if applicable).

*   **`fn subscribe(&self, key_path: &str, listener: Box<ChangeListener>)`**
    *   Registers a listener for changes at or below `key_path`. Listener receives `(notify_key, changed_key, new_value)`.

*   **`fn subscribe_detailed(&self, key_path: &str, listener: Box<DetailedChangeListener>)`**
    *   **New:** Registers a listener receiving `(notify_key, changed_key, new_value, old_value)`.

*   **`fn current_key_path(&self) -> &str`**
    *   Returns the root path (`""` for root store, prefix for branches).

*   **`fn get_source(&self, key_path: &str) -> Option<ConfigSource>`**
    *   **New:** Retrieves the origin (`ConfigSource`) of the value at `key_path`.

### Value & Source Representation

*   **`enum C5DataValue`**
    *   Represents config values: `Null`, `Bytes`, `Boolean`, `Integer`, `UInteger`, `Float`, `String`, `Array`, `Map`.
    *   **Changed:** `TryInto<T>` now returns `Result<T, ConfigError>`.

*   **`enum ConfigSource`**
    *   **New:** Represents value origin: `File(PathBuf)`, `EnvironmentVariable(String)`, `Provider(String)`, `SetProgrammatically`, `Unknown`.

### Provider Management

*   **`struct C5StoreMgr`**
    *   Manages value providers. Returned by `create_c5store`.
    *   **`fn set_value_provider<P>(&mut self, name: &str, provider: P, refresh_period_sec: u64)`**
        *   Registers a provider instance implementing `C5ValueProvider`.

### Callbacks / Advanced Types

*   **`type ChangeListener = dyn Fn(&str, &str, &C5DataValue) -> () + Send + Sync`**
    *   Listener for `subscribe`. Params: `(notify_key, changed_key, new_value)`.

*   **`type DetailedChangeListener = dyn Fn(&str, &str, &C5DataValue, Option<&C5DataValue>) -> () + Send + Sync`**
    *   **New:** Listener for `subscribe_detailed`. Params: `(notify_key, changed_key, new_value, old_value)`.

*   **`trait C5ValueProvider: Send + Sync`** (For Implementers)
    *   Requires implementing `register`, `unregister`, `hydrate`.
    *   `fn hydrate(&self, set_data_fn: Arc<SetDataFn>, force: bool, context: &HydrateContext)`
        *   **Changed (Breaking):** `set_data_fn` parameter is now `Arc<SetDataFn>`.

*   **`type SetDataFn = dyn Fn(&str, C5DataValue) + Send + Sync`** (For Implementers)
    *   Callback function passed (as an `Arc`) to `hydrate`.