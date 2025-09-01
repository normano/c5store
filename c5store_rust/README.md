# C5Store for Rust

[![License: MPL-2.0](https://img.shields.io/badge/License-MPL%202.0-brightgreen.svg)](https://opensource.org/licenses/MPL-2.0)
[![Crates.io](https://img.shields.io/crates/v/c5store.svg)](https://crates.io/crates/c5store)

C5Store is a Rust library providing a **unified store for configuration and secrets**. It aims to be a single point of access for your application's configuration needs, consolidating values from various sources (like YAML and TOML files or directories), handling environment variable overrides, managing secrets securely via built-in decryption, and allowing dynamic loading through providers.

The core idea is to simplify configuration management in complex applications by offering a hierarchical, type-aware, extensible, and environment-aware configuration layer.

## Key Features

*   **Unified Access:** Retrieve configuration values using simple dot-notation key paths (e.g., `database.connection.pool_size`).
*   **Multiple Sources & Merging:** Load configuration from YAML and TOML files, or entire directories containing such files. Configuration is intelligently merged based on load order.
*   **Environment Variable Overrides:** Seamlessly override any configuration value using environment variables (e.g., `C5_DATABASE__HOST=...`). Values are intelligently parsed (bool, int, float, string).
*   **Type-Safe Retrieval:** Get values converted directly into expected Rust types using `get_into::<T>()`, returning a `Result` for robust error handling.
*   **Flexible Struct Deserialization:** Deserialize configuration sections directly into your custom Rust structs using `get_into_struct::<T>()`. Supports both nested maps (from files) and flattened key structures (e.g., from environment variables).
*   **Integrated Secrets Management (Optional Feature):**
    *   Transparently decrypt secrets defined within configuration files using the `.c5encval` key.
    *   Supports pluggable decryption algorithms (includes `base64` and `ecies_x25519`).
    *   Securely load decryption keys from files (including `.pem`), environment variables, or **`systemd` credentials** (`secrets_systemd` feature on Linux).
*   **Value Providers:** Defer loading of specific configuration sections to external sources (e.g., files) using a provider system. Includes a built-in `C5FileValueProvider`.
*   **Periodic Refresh:** Value providers can be configured to automatically refresh their data at specified intervals.
*   **Change Notifications:** Subscribe to changes in configuration values at specific key paths or their ancestors using `subscribe` (basic) or `subscribe_detailed` (includes old value). Notifications are debounced.
*   **Hierarchical Structure:** Access nested configuration values easily and create "branches" for context-specific views of the configuration using `branch()`.
*   **Source Tracking:** Identify the origin of any configuration value (File, Env Var, Provider) using `get_source()`.
*   **`.env` File Support (Optional Feature):** Load environment variables from `.env` files at startup.
*   **Extensible:** Designed with traits for custom value providers and secret decryptors.
*   **Telemetry Hooks:** Basic interfaces for integrating custom logging and statistics recording.
*   **Optional Feature Flags:** Fine-tune dependencies (`dotenv`, `toml`, `secrets`, `secrets_systemd`).

## Getting Started

1.  **Add Dependency:** Add `c5store` to your `Cargo.toml`. Enable optional features as needed:

    ```toml
    [dependencies]
    # Use the latest version
    # On Linux, "secrets_systemd" is enabled by default.
    c5store = "0.5.0"

    # Example enabling .env file support (optional)
    # c5store = { version = "0.5.0", features = ["dotenv"] }

    # Example disabling default secrets features (optional, smaller binary)
    # c5store = { version = "0.5.0", default-features = false }
    
    # On non-Linux, to use the systemd types for cross-compilation, enable it explicitly:
    # c5store = { version = "0.5.0", features = ["secrets_systemd"] }

    # Other necessary dependencies
    serde = { version = "1", features = ["derive"] }
    ```

2.  **Basic Usage:**

    ```rust
    use c5store::{create_c5store, C5Store, C5StoreOptions, ConfigError, ConfigSource}; // Import types
    use std::path::PathBuf;
    use serde::Deserialize; // Needed for get_into_struct

    #[derive(Deserialize, Debug, PartialEq)] // Example struct for deserialization
    struct ServiceConfig {
        name: String,
        port: u16,
        #[serde(default)] // Handle potentially missing fields
        threads: u32,
    }

    fn main() -> Result<(), Box<dyn std::error::Error>> { // Main can return Result
        // 1. Define configuration paths (can include files and directories)
        let config_paths = vec![
            PathBuf::from("config/common.yaml"),
            PathBuf::from("config/defaults.toml"),
            PathBuf::from("config/environment_specific/"), // Load all supported files in this dir
            PathBuf::from("config/local.yaml"),      // Local file overrides
        ];

        // 2. (Optional) Configure options
        let mut options = C5StoreOptions::default();
        // Example: Enable loading .env file if 'dotenv' feature is enabled
        #[cfg(feature = "dotenv")]
        {
            options.dotenv_path = Some(PathBuf::from(".env.local"));
        }

        // 3. Create the store (now returns Result)
        // store_mgr manages background tasks like provider refreshes. Keep it alive if needed.
        let (store, mut store_mgr) = create_c5store(config_paths, Some(options))?; // Use '?' operator

        // --- Retrieving Values ---

        // Get raw value (Option<C5DataValue>)
        if let Some(db_host) = store.get("database.host") {
            println!("Database Host (C5DataValue): {:?}", db_host);
            // Check its source
            if let Some(source) = store.get_source("database.host") {
                 println!(" -> Source: {}", source); // e.g., File("config/local.yaml") or EnvVar("C5_DATABASE__HOST")
            }
        }

        // Get directly as a specific type (returns Result)
        match store.get_into::<u64>("database.pool_size") {
            Ok(pool_size) => println!("Pool Size (u64): {}", pool_size),
            Err(ConfigError::KeyNotFound(_)) => println!("Pool Size: Using default (e.g., 10)"),
            Err(e @ ConfigError::TypeMismatch { .. }) => println!("Pool Size Error: {}", e), // Handle type mismatch
            Err(e) => println!("Error getting pool size: {}", e), // Handle other errors
        }

        // Deserialize into a struct (handles nested or flattened sources)
        match store.get_into_struct::<ServiceConfig>("service") {
             Ok(service_config) => println!("Service Config: {:?}", service_config),
             Err(ConfigError::KeyNotFound(_)) => println!("Service config section not found."),
             Err(e @ ConfigError::DeserializationError { .. }) => println!("Service config deserialization error: {}", e),
             Err(e) => println!("Error getting service config: {}", e),
        }

        // --- Checking Existence ---

        // Check exact key existence
        if store.exists("database.user") {
            println!("Database user key exists.");
        }

        // Check if a path prefix exists (implies children exist)
        if store.path_exists("database") {
            println!("Database configuration section exists.");
        }

        // --- Using Branches ---

        let db_config = store.branch("database");
        match db_config.get_into::<String>("password") { // Relative path "password" -> absolute "database.password"
            Ok(password) => println!("Password from branch retrieved (use securely!)."),
            Err(_) => println!("Password not found or couldn't be read as string."),
        }
        println!("Current branch path: {}", db_config.current_key_path()); // "database"

        // --- Value Providers ---
        // (Provider registration happens via store_mgr, not shown here for brevity)
        // Example (conceptual):
        // let file_provider = C5FileValueProvider::default("path/to/resources");
        // store_mgr.set_value_provider("files", file_provider, 60); // Refresh every 60s

        // Keep store_mgr alive if background refreshes are needed.
        // drop(store_mgr); // Explicitly drop to stop refreshes

        Ok(())
    }
    ```

## Configuration Files & Directories

C5Store loads configuration from specified paths in the `create_c5store` call. These paths can be:

*   **YAML files** (`.yaml`, `.yml`)
*   **TOML files** (`.toml`) - Requires `toml` feature.
*   **Directories:** All files within the directory with supported extensions (`.yaml`, `.yml`, `.toml`) will be loaded and merged **alphabetically**.

Configuration sources are merged in the order they are processed (files listed explicitly first, then files within directories alphabetically). Values from later sources **override** values from earlier sources for the same key path. Maps (objects/tables) are merged recursively; other types are replaced entirely.

**Example (`config/common.yaml`):**

```yaml
service:
  name: MyAwesomeApp
  port: 8080
database:
  host: prod-db.example.com
  pool_size: 50
```

**Example (`config/local.toml`):**

```toml
# Overrides common.yaml values
# Assumes local.toml is processed after common.yaml
service.port = 9090 # Overrides port 8080

[database]
host = "localhost" # Overrides prod host
user = "dev_user" # Adds a new key

# service.name and database.pool_size are inherited from common.yaml
```

## Environment Variables & Loading Priority

C5Store supports overriding configuration values using environment variables after all files have been loaded and merged.

*   **Prefix:** Variables starting with `C5_` (by default) are processed.
*   **Separator:** Double underscore (`__`) is used to denote nesting levels (e.g., `C5_DATABASE__HOST` maps to `database.host`).
*   **Case:** The key derived from the environment variable is converted to lowercase (e.g., `C5_SERVICE__NAME` becomes `service.name`).
*   **Value Parsing:** Environment variable values are parsed into the most appropriate `C5DataValue` type (Boolean, Integer, Float, String).

**Loading Priority (Highest to Lowest):**

1.  **Environment Variables** (e.g., `C5_...`)
2.  **Configuration Files/Directories** (processed in the order specified/discovered, with later files/directories overriding earlier ones).

## Optional Features (`dotenv`, `toml`, `secrets`, `secrets_systemd`)

C5Store uses Cargo features to enable optional functionality:

*   **`dotenv`**:
    *   Enables loading environment variables from a `.env` file at startup using `C5StoreOptions::dotenv_path`.
    *   Requires the `dotenvy` crate.
    *   `.env` files are loaded *before* process environment variables are read, allowing process variables to override `.env` variables.
*   **`toml`**:
    *   Enables parsing of `.toml` configuration files.
    *   Requires the `toml` crate.
*   **`secrets`**:
    *   Enables all secrets management functionality (loading `.c5encval`, `SecretOptions`, `SecretKeyStore`, decryptors).
    *   Requires crypto dependencies (`ecies_25519`, `curve25519-parser`, `sha2`).
    *   **Enabled by default.**
*   **`secrets_systemd`**:
    *   Enables loading of decryption keys from `systemd`'s secure credential store.
    *   Depends on the `secrets` feature.
    *   **Enabled by default on Linux targets.**
*   **`full`**:
    *   Convenience feature to enable `dotenv`, `toml`, `secrets`, and `secrets_systemd`.

```toml
[dependencies]
# Minimal - no .env, no secrets, no toml
# c5store = { version = "0.5.0", default-features = false }

# Default - secrets and yaml enabled. On Linux, also enables secrets_systemd.
# c5store = "0.5.0"

# Enable all common features
c5store = { version = "0.5.0", features = ["full"] }

# Just enable .env support
# c5store = { version = "0.5.0", default-features = false, features = ["dotenv"] }
```

## Secrets Management (`secrets` feature)

*(Requires the `secrets` feature, enabled by default).*

Secrets are defined using a special `.c5encval` key (configurable via `SecretOptions::secret_key_path_segment`) within your configuration.

**Structure:**

```yaml
# YAML Example
some_secret_key:
  .c5encval: ["<algorithm>", "<ref_key_name>", "<base64_encrypted_data>"]

# TOML Example
# [some_secret_key]
# ".c5encval" = ["<algorithm>", "<ref_key_name>", "<base64_encrypted_data>"]
```

*   **`<algorithm>`:** Name of registered `SecretDecryptor` (e.g., `"base64"`, `"ecies_x25519"`).
*   **`<ref_key_name>`:** Name used to look up the decryption key in the `SecretKeyStore`.
*   **`<base64_encrypted_data>`:** The secret value, encrypted and then Base64 encoded.

**Key Loading Methods:**

You can load decryption keys into `c5store` from three sources, configured via `SecretOptions`.

1.  **From a Directory (`secret_keys_path`)**:
    *   Loads key files from a specified directory. The filename (without extension) becomes the `ref_key_name`.
2.  **From Environment Variables (`load_secret_keys_from_env`)**:
    *   Loads keys from environment variables. The variable name (minus a prefix) becomes the `ref_key_name`.
3.  **From `systemd` Credentials (`load_credentials_from_systemd`)**:
    *   **(Linux-Only, `secrets_systemd` feature)** Securely loads keys that have been provisioned by `systemd`. This is the recommended method for production. See the dedicated section below.

**Example Configuration (`SecretOptions`):**

```rust
use c5store::{C5StoreOptions, SecretOptions, create_c5store};
#[cfg(feature = "secrets")]
use c5store::secrets::{SecretKeyStore, Base64SecretDecryptor, EciesX25519SecretDecryptor};
#[cfg(feature = "secrets")]
use ecies_25519::EciesX25519;
use std::path::PathBuf;

// ... inside setup code ...

let mut options = C5StoreOptions::default();

#[cfg(feature = "secrets")] // Gate configuration if secrets might be disabled
{
    options.secret_opts = SecretOptions {
        // Path to directory containing decryption key files (e.g., .pem or raw bytes).
        secret_keys_path: Some(PathBuf::from("path/to/your/secret_keys")),

        // Override the special key identifying secrets. Default is ".c5encval"
        secret_key_path_segment: None, // Keep default

        // Programmatically configure the SecretKeyStore.
        secret_key_store_configure_fn: Some(Box::new(|key_store: &mut SecretKeyStore| {
            key_store.set_decryptor("base64", Box::new(Base64SecretDecryptor {}));
            key_store.set_decryptor(
                "ecies_x25519",
                Box::new(EciesX25519SecretDecryptor::new(EciesX25519::new()))
            );
            // key_store.set_key("manual_key", vec![...]); // Manually add keys
        })),

        // Enable loading keys from environment variables.
        load_secret_keys_from_env: true,
        // Prefix for environment variables holding keys (e.g., C5_SECRETKEY_MYAPIKEY).
        // Value should be base64 encoded key bytes. Default is "C5_SECRETKEY_"
        secret_key_env_prefix: None, // Keep default

        // No systemd credentials in this example, so the default empty Vec is used.
        ..Default::default()
    };
}

let config_paths = vec![/* ... */ PathBuf::from("secrets.yaml")];
let (store, mut store_mgr) = create_c5store(config_paths, Some(options))?;

// Retrieving the secret automatically attempts decryption
match store.get_into::<Vec<u8>>("some_secret_key") { // Key is now the one *without* .c5encval
    Ok(token_bytes) => println!("Decrypted secret retrieved ({:?} bytes).", token_bytes.len()),
    Err(e) => println!("Failed to get/decrypt secret: {}", e),
}
```

### Secure Key Loading with `systemd` (`secrets_systemd` feature)

*(Requires the `secrets_systemd` feature, enabled by default on Linux).*

For production deployments on Linux, `c5store` can securely load its decryption key directly from the `systemd` credential store. This avoids having plaintext private keys on the filesystem.

**Administrator Workflow:**

1.  **Encrypt the Private Key for `systemd`**: On the target server, use `systemd-creds` to encrypt your private key file (e.g., `my_app.c5.key.pem`). The name `myapp.private.key` is the **credential name**.
    ```bash
    cat my_app.c5.key.pem | systemd-creds encrypt - /etc/credstore.encrypted/myapp.private.key
    ```
2.  **Configure the `systemd` Service**: Edit your application's service file to use the `LoadCredential=` directive. The name must match the one used above.
    ```ini
    # /etc/systemd/system/myapp.service
    [Service]
    DynamicUser=yes
    LoadCredential=myapp.private.key
    ExecStart=/usr/bin/myapp-server
    ```
    Run `systemctl daemon-reload` after saving.

**Application Configuration:**

Enable the feature in your application's `C5StoreOptions`.

```rust
use c5store::secrets::systemd::SystemdCredential;

// ... inside setup code ...
let mut options = C5StoreOptions::default();
options.secret_opts.load_credentials_from_systemd = vec![
  SystemdCredential {
    // This MUST match the name in LoadCredential=
    credential_name: "myapp.private.key".to_string(),
    
    // This MUST match the <ref_key_name> in your config.yaml
    ref_key_name: "my_app".to_string(),
  }
];

let (store, _mgr) = create_c5store(config_paths, Some(options))?;
```

At runtime, `systemd` will securely provide the decrypted key to `c5store`, which will then use it to decrypt any secrets in your configuration that reference the `"my_app"` key name.

## Value Providers

Value providers allow parts of your configuration to be loaded dynamically from external sources (like files, databases, or remote services). Mark a section in YAML/TOML with a `.provider` key specifying the provider's name. Register providers using `C5StoreMgr::set_value_provider`. C5Store includes a `C5FileValueProvider` for loading content from files specified in the configuration.

**Example (`config/providers.yaml`):**

```yaml
files:
  large_config:
    .provider: resource # Name matches registered provider
    path: large_data.json # Path relative to provider base or absolute
    format: json # Instruct provider to parse as JSON
  raw_template:
    .provider: resource
    path: template.txt
    # format: raw (default)
    # encoding: utf8 (default)
```

**Registration:**

```rust
use c5store::providers::C5FileValueProvider;
// ... inside main after create_c5store ...

// Create provider, setting its base path for relative 'path' values
let file_provider = C5FileValueProvider::default("data_files/"); // Use built-in JSON/YAML deserializers

// Register with the store manager, optionally enable refresh
store_mgr.set_value_provider(
    "resource",      // Name used in .provider key
    file_provider,
    300             // Refresh interval in seconds (0 for no refresh)
);

// Now access values loaded by the provider
match store.get_into::<String>("files.raw_template") {
    Ok(template) => println!("Loaded template."),
    Err(e) => println!("Failed to load template: {}", e),
}
```

## Change Notifications

Subscribe to configuration changes using `subscribe` (new value only) or `subscribe_detailed` (new and old value). Listeners are called after a configurable debounce period (`C5StoreOptions::change_delay_period`).

```rust
// Subscribe to changes under the 'database' prefix
store.subscribe_detailed("database", Box::new(|notify_key, changed_key, new_val, old_val| {
    println!(
        "[CHANGE] Notify Key: '{}', Changed Key: '{}', New: {:?}, Old: {:?}",
        notify_key, changed_key, new_val, old_val
    );
}));

// Programmatic changes (or provider refreshes) will trigger notifications later
// store.set("database.pool_size", 100.into()); // Example change
```

## License

This project is licensed under the **Mozilla Public License Version 2.0 (MPL-2.0)**. See [LICENSE](LICENSE) file for details.

## Contributing

Contributions welcome! Please open issues or PRs on the project repository.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for a history of notable changes.