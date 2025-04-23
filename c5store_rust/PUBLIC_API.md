# C5Store - Quick API Reference

This document provides a concise overview of the primary public API components for using the `c5store` library.

### Initialization

*   **`create_c5store(config_file_paths: Vec<PathBuf>, options: Option<C5StoreOptions>) -> (impl C5Store, C5StoreMgr)`**
    *   The main entry point to create and initialize the configuration store.
    *   Loads and merges configuration from YAML files specified in `config_file_paths` (order matters).
    *   Applies optional configuration via `C5StoreOptions`.
    *   Returns:
        *   An object implementing the `C5Store` trait for accessing configuration.
        *   A `C5StoreMgr` instance for managing value providers.

*   **`struct C5StoreOptions`**
    *   Used to configure store behavior (passed to `create_c5store`).
    *   Fields include: `logger`, `stats`, `change_delay_period`, `secret_opts: SecretOptions`.
    *   Use `C5StoreOptions::default()` for sensible defaults.

*   **`struct SecretOptions`**
    *   Configures secrets management within `C5StoreOptions`.
    *   Key Fields:
        *   `secret_key_path_segment: Option<String>` (Default: `Some(".c5encval".to_string())`) - The YAML key indicating an encrypted value.
        *   `secret_keys_path: Option<PathBuf>` - Path to a directory containing decryption key files.
        *   `secret_key_store_configure_fn: Option<Box<dyn FnMut(&mut SecretKeyStore)>>` - Closure to programmatically configure decryptors and keys.

### Core Store Access (`C5Store` Trait)

This trait defines the primary interface for interacting with the configuration store.

*   **`fn get(&self, key_path: &str) -> Option<C5DataValue>`**
    *   Retrieves the configuration value for a given dot-notation `key_path`.
    *   Returns the raw `C5DataValue` enum variant if found, otherwise `None`.

*   **`fn get_into<T>(&self, key_path: &str) -> Option<T>`**
    *   Retrieves the value for `key_path` and attempts to convert it into type `T`.
    *   Requires `C5DataValue: TryInto<T, Error = ()>`. Many standard Rust types are supported (see `C5DataValue` below).
    *   Returns `Some(T)` on successful retrieval and conversion, otherwise `None`.

*   **`fn exists(&self, key_path: &str) -> bool`**
    *   Checks if an *exact* key path exists in the store (after merging, provider loading, and decryption).

*   **`fn path_exists(&self, key_path: &str) -> bool`**
    *   Checks if the given `key_path` exists either as an exact key or as a prefix for other keys (e.g., `path_exists("database")` is true if `database.host` exists).

*   **`fn branch(&self, key_path: &str) -> impl C5Store`** _(Note: Actual return type `C5StoreBranch`)_
    *   Creates a view ("branch") of the store rooted at the given `key_path`.
    *   Subsequent calls (`get`, `get_into`, etc.) on the branch use paths relative to this root.

*   **`fn key_paths_with_prefix(&self, key_path: Option<&str>) -> Vec<String>`**
    *   Lists all fully-qualified key paths that start with the given prefix.
    *   If `key_path` is `None`, returns all keys in the store (or branch).
    *   On a branch, the prefix is relative to the branch's root.

*   **`fn subscribe(&self, key_path: &str, listener: Box<ChangeListener>)`**
    *   Registers a `listener` closure to be called when the value at `key_path` (or any key underneath it) changes.
    *   Notifications are debounced.

*   **`fn current_key_path(&self) -> &str`**
    *   Returns the key path prefix for the current store or branch (empty string `""` for the root).

### Value Representation

*   **`enum C5DataValue`**
    *   Represents any configuration value within the store.
    *   Variants: `Null`, `Bytes(Vec<u8>)`, `Boolean(bool)`, `Integer(i64)`, `UInteger(u64)`, `Float(f64)`, `String(String)`, `Array(Vec<C5DataValue>)`, `Map(HashMap<String, C5DataValue>)`.
    *   Implements `TryInto<T>` for many standard Rust types (`bool`, `String`, `Vec<u8>`, `i8`-`i64`, `u8`-`u64`, `f32`/`f64`, `Vec<T>`, `HashMap<String, C5DataValue>`), allowing easy conversion via `get_into`.

### Provider Management

*   **`struct C5StoreMgr`**
    *   Returned by `create_c5store`, used to manage value providers.
    *   **`fn set_value_provider<P: 'static + C5ValueProvider>(&mut self, name: &str, provider: P, refresh_period_sec: u64)`**
        *   Registers a value provider instance (`provider`) under a given `name` (which must match the `.provider` value in YAML configuration).
        *   `refresh_period_sec`: How often (in seconds) the provider's `hydrate` method should be called automatically (0 for no automatic refresh).

### Callbacks / Advanced Types

*   **`type ChangeListener = dyn Fn(&str, &str, &C5DataValue) -> () + Send + Sync`**
    *   Signature for closures passed to `subscribe`. Parameters are: `(subscribed_key_path, actual_changed_key_path, new_value)`.

*   **`trait C5ValueProvider: Send + Sync`** (For Implementers)
    *   Trait required for custom value provider implementations.
    *   Key method: `fn hydrate(&self, set_data_fn: &SetDataFn, force: bool, context: &HydrateContext)` - Logic to load/fetch data and push it into the store via `set_data_fn`.

*   **`type SetDataFn = dyn Fn(&str, C5DataValue) + Send + Sync`** (For Implementers)
    *   Callback function passed to provider's `hydrate` method, used to insert/update values in the store.