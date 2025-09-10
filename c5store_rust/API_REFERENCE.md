# **`c5store` - Quick API Reference (v0.4.5+)**

This document provides a concise overview of the primary public API components for using the `c5store` library.

## Initialization

*   **`create_c5store(config_paths: Vec<PathBuf>, options: Option<C5StoreOptions>) -> Result<(C5StoreRoot, C5StoreMgr), ConfigError>`**
    *   The main entry point. Loads/merges config from files (YAML/TOML), directories, and environment variables (`C5_*`). See README for priority. Applies options.
    *   Returns `Ok((store, manager))` on success, where `store` implements `C5Store`. `manager` handles background tasks (keep it alive if needed).
    *   Returns `Err(ConfigError)` on loading/parsing failures.

*   **`struct C5StoreOptions`**
    *   Configures store behavior. Passed to `create_c5store`. Derive `Default`.
    *   Fields: `logger`, `stats`, `change_delay_period`, `secret_opts: SecretOptions`.
    *   **`dotenv` feature:** `dotenv_path: Option<PathBuf>` - Optional path to a `.env` file.
    *   Use `C5StoreOptions::default()` or construct manually.

*   **`struct SecretOptions` (`secrets` feature)**
    *   Configures secrets management (part of `C5StoreOptions`). Derive `Default`. Requires `secrets` feature.
    *   Key Fields:
        *   `secret_key_path_segment: Option<String>` (Default: `".c5encval"`) - Key indicating encrypted value.
        *   `secret_keys_path: Option<PathBuf>` - Path to directory of decryption key files.
        *   `secret_key_store_configure_fn: Option<Box<dyn FnMut(&mut SecretKeyStore)>>` - Closure for programmatic key/decryptor setup.
        *   `load_secret_keys_from_env: bool` (Default: `false`) - Load keys from env vars?
        *   `secret_key_env_prefix: Option<String>` (Default: `"C5_SECRETKEY_"`) - Prefix for env vars with base64 keys.
        *   **(New)** **`secrets_systemd` feature:** `load_credentials_from_systemd: Vec<SystemdCredential>` - Configures loading of secret keys provided by `systemd`. See `SystemdCredential` struct below.

*   **`struct SystemdCredential` (`secrets_systemd` feature)**

    *   Defines the mapping between a key provided by `systemd` and its logical name within `c5store`. Used within `SecretOptions`.
    *   Requires the `secrets_systemd` feature, which is enabled by default on Linux.

    *   **Fields:**
        *   `credential_name: String`: The name of the credential file as configured in the `systemd` service unit's `LoadCredential=` or `LoadCredentialEncrypted=` directive (e.g., `"myapp.private.key"`).
        *   `ref_key_name: String`: The logical name this key will be known by within `c5store`, which must match the key name used in the `.c5encval` array in your YAML files (e.g., `"my_app"`).
        *   `format: KeyFormat`: (**Optional**, defaults to `Raw`) Specifies the format of the key material provided by `systemd`. This tells `c5store` if any further processing is needed after the credential is read.

    *   **Enum `KeyFormat`:**
        *   `Raw`: The credential data is a raw binary key and will be used as-is. This is the default.
        *   `PemX25519`: The credential data is a PEM-encoded X25519 private key. `c5store` will parse this text format to extract the raw 32-byte secret key before use.

## Error Handling

*   **`enum ConfigError`**
    *   Error type returned by fallible operations. Implements `std::error::Error` (via `thiserror`).
    *   Variants cover: `KeyNotFound`, `TypeMismatch`, `ConversionError`, `DeserializationError`, `IoError`, `YamlParseError`, `TomlParseError` (`toml` feature), `DotEnvLoadError` (`dotenv` feature), `SecretKeyNotFound` (`secrets` feature), `DecryptionError` (`secrets` feature), etc.

## Core Store Access (`C5Store` Trait)

The primary interface for configuration access. Implemented by `C5StoreRoot` and `C5StoreBranch`.

*   **`fn get(&self, key_path: &str) -> Option<C5DataValue>`**
    *   Retrieves the raw `C5DataValue` for an *exact* key path. Returns `None` if not found.

*   **`fn get_ref(&self, key_path: &str) -> Option<C5StoreDataValueRef>`**
    *   Gets a temporary reference wrapper (`C5StoreDataValueRef`) containing references to the value and its source for an *exact* key path, avoiding cloning. Returns `None` if not found. Use `.value()` or `.source()` on the result.

*   **`fn get_into<T>(&self, key_path: &str) -> Result<T, ConfigError>`**
    *   Retrieves value at *exact* key path and attempts conversion to type `T`.
    *   Requires `C5DataValue: TryInto<T, Error = ConfigError>`.
    *   Returns `Ok(T)` or `ConfigError` (e.g., `KeyNotFound`, `TypeMismatch`, `ConversionError`).

*   **`fn get_into_struct<'de, T>(&self, key_path: &str) -> Result<T, ConfigError>`**
    *   Deserializes a configuration *section* rooted at `key_path` into struct `T` (requires `serde::Deserialize`).
    *   Works correctly whether the underlying data uses nested maps (from files) or flattened keys (e.g., from env vars like `key_path.field`).
    *   Returns `Ok(T)` or `ConfigError` (e.g., `KeyNotFound` if no keys match prefix, `DeserializationError`).

*   **`fn exists(&self, key_path: &str) -> bool`**
    *   Checks for an *exact* key path match.

*   **`fn path_exists(&self, key_path: &str) -> bool`**
    *   Checks if the path exists exactly *or* as a prefix for other keys (e.g., returns true for `"database"` if `"database.host"` exists).

*   **`fn branch(&self, key_path: &str) -> C5StoreBranch`**
    *   Creates a branched view rooted at `key_path`. Subsequent calls on the branch use relative paths.

*   **`fn key_paths_with_prefix(&self, key_path: Option<&str>) -> Vec<String>`**
    *   Lists keys starting with the given prefix (relative to branch if applicable). If `key_path` is `None`, lists all keys under the current branch/root.

*   **`fn subscribe(&self, key_path: &str, listener: Box<ChangeListener>)`**
    *   Registers a listener for changes at or below `key_path`. Listener receives `(notify_key, changed_key, new_value)`.

*   **`fn subscribe_detailed(&self, key_path: &str, listener: Box<DetailedChangeListener>)`**
    *   Registers a listener receiving `(notify_key, changed_key, new_value, old_value)`.

*   **`fn current_key_path(&self) -> &str`**
    *   Returns the root path of the current view (`""` for root store, prefix for branches).

*   **`fn get_source(&self, key_path: &str) -> Option<ConfigSource>`**
    *   Retrieves the origin (`ConfigSource`) of the value at the *exact* `key_path`.

## Value & Source Representation

*   **`enum C5DataValue`**
    *   Represents config values: `Null`, `Bytes`, `Boolean`, `Integer(i64)`, `UInteger(u64)`, `Float(f64)`, `String`, `Array(Vec<C5DataValue>)`, `Map(HashMap<String, C5DataValue>)`.
    *   Implements `From<T>` for many standard types.
    *   Implements `TryInto<T>` returning `Result<T, ConfigError>` for many standard types.

*   **`enum ConfigSource`**
    *   Represents value origin: `File(PathBuf)`, `EnvironmentVariable(String)`, `Provider(String)`, `SetProgrammatically`, `Unknown`.
    *   Implements `Display`.

## Provider Management

*   **`struct C5StoreMgr`**
    *   Manages value providers and background tasks (e.g., refresh). Returned by `create_c5store`. Keep alive if refreshes needed.
    *   **`fn set_value_provider<P>(&mut self, name: &str, provider: P, refresh_period_sec: u64)`**
        *   Registers a provider instance implementing `C5ValueProvider`. `name` matches `.provider` key in config. `refresh_period_sec = 0` for no refresh.

## Callbacks / Advanced Types

*   **`type ChangeListener = dyn Fn(&str, &str, &C5DataValue) -> () + Send + Sync`**
    *   Listener for `subscribe`. Params: `(notify_key, changed_key, new_value)`.

*   **`type DetailedChangeListener = dyn Fn(&str, &str, &C5DataValue, Option<&C5DataValue>) -> () + Send + Sync`**
    *   Listener for `subscribe_detailed`. Params: `(notify_key, changed_key, new_value, old_value)`.

*   **`trait C5ValueProvider: Send + Sync`** (For Implementers)
    *   Requires implementing `register(&mut self, data: &C5DataValue)`, `unregister(&mut self, key: &str)`, `hydrate(&self, set_data_fn: &SetDataFn, force: bool, context: &HydrateContext)`.
    *   Note: `hydrate` receives `set_data_fn: &SetDataFn` (reference to the function).

*   **`type SetDataFn = dyn Fn(&str, C5DataValue) + Send + Sync`** (For Implementers)
    *   Callback function passed (as a reference) to `hydrate`. Implementers call this to push data into the store.

*   **`struct HydrateContext`** (For Implementers)
    *   Passed to `hydrate`. Contains shared resources like `logger`.
    *   Helper `push_value_to_data_store(&SetDataFn, &str, C5DataValue)` simplifies adding nested maps.