# C5Store for Rust

[![License: MPL-2.0](https://img.shields.io/badge/License-MPL%202.0-brightgreen.svg)](https://opensource.org/licenses/MPL-2.0)
<!-- Add other badges here if you have them, e.g., Crates.io version, Build Status -->

C5Store is a Rust library providing a **unified store for configuration and secrets**. It aims to be a single point of access for your application's configuration needs, consolidating values from various sources (like YAML and TOML files or directories), handling environment variable overrides, managing secrets securely via built-in decryption, and allowing dynamic loading through providers.

The core idea is to simplify configuration management in complex applications by offering a hierarchical, type-aware, extensible, and environment-aware configuration layer.

## Key Features

*   **Unified Access:** Retrieve configuration values using simple dot-notation key paths (e.g., `database.connection.pool_size`).
*   **Multiple Sources & Merging:** Load configuration from YAML and TOML files, or entire directories containing such files. Configuration is intelligently merged based on load order.
*   **Environment Variable Overrides:** Seamlessly override any configuration value using environment variables (e.g., `C5_DATABASE__HOST=...`).
*   **Type-Safe Retrieval:** Get values converted directly into expected Rust types using `get_into::<T>()`, now returning a `Result` for robust error handling.
*   **Direct Struct Deserialization:** Deserialize entire configuration branches directly into your custom Rust structs using `get_into_struct::<T>()`.
*   **Integrated Secrets Management (Optional Feature):**
    *   Transparently decrypt secrets defined within configuration files using the `.c5encval` key.
    *   Supports pluggable decryption algorithms (includes `base64` and `ecies_x25519`).
    *   Securely load decryption keys from files (including `.pem`) or environment variables.
*   **Value Providers:** Defer loading of specific configuration sections to external sources (e.g., files) using a provider system. Includes a built-in `C5FileValueProvider`.
*   **Periodic Refresh:** Value providers can be configured to automatically refresh their data at specified intervals.
*   **Change Notifications:** Subscribe to changes in configuration values at specific key paths or their ancestors. Notifications are debounced to prevent flooding.
*   **Hierarchical Structure:** Access nested configuration values easily and create "branches" for context-specific views of the configuration.
*   **`.env` File Support (Optional Feature):** Load environment variables from `.env` files at startup.
*   **Extensible:** Designed with traits for custom value providers and secret decryptors.
*   **Telemetry Hooks:** Basic interfaces for integrating custom logging and statistics recording.

## Getting Started

1.  **Add Dependency:** Add `c5store` to your `Cargo.toml`. Enable optional features as needed:

    ```toml
    [dependencies]
    # Base library
    c5store = "0.3.0" # Use v0.3.0+ for new features/error handling

    # Example enabling .env file support (optional)
    # c5store = { version = "0.3.0", features = ["dotenv"] }

    # Example disabling default secrets support (optional, smaller binary)
    # c5store = { version = "0.3.0", default-features = false }

    # Other necessary dependencies like serde, etc.
    serde = { version = "1", features = ["derive"] }
    ```

2.  **Basic Usage:**

    ```rust
    use c5store::{create_c5store, C5Store, C5StoreOptions, ConfigError}; // Import ConfigError
    use std::path::PathBuf;
    use serde::Deserialize; // Needed for get_into_struct

    #[derive(Deserialize, Debug)] // Example struct for deserialization
    struct ServiceConfig {
        name: String,
        port: u16,
    }

    fn main() -> Result<(), Box<dyn std::error::Error>> { // Main can return Result now
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
        // #[cfg(feature = "dotenv")]
        // {
        //     options.dotenv_path = Some(PathBuf::from(".env.local"));
        // }

        // 3. Create the store (now returns Result)
        let (store, mut store_mgr) = create_c5store(config_paths, Some(options))?; // Use '?' operator

        // 4. Retrieve values
        if let Some(db_host) = store.get("database.host") {
            println!("Database Host (C5DataValue): {:?}", db_host);
        }

        // Get directly as a specific type (now returns Result)
        match store.get_into::<u64>("database.pool_size") {
            Ok(pool_size) => println!("Pool Size (u64): {}", pool_size),
            Err(ConfigError::KeyNotFound(_)) => println!("Pool Size: Using default (e.g., 10)"),
            Err(e) => println!("Error getting pool size: {}", e), // Handle other errors (e.g., TypeMismatch)
        }

        // Deserialize into a struct
        match store.get_into_struct::<ServiceConfig>("service") {
             Ok(service_config) => println!("Service Config: {:?}", service_config),
             Err(e) => println!("Error getting service config: {}", e),
        }


        // Check existence
        if store.exists("database.user") {
            println!("Database user is configured.");
        }

        // Check if a path prefix exists
        if store.path_exists("database") {
            println!("Database configuration section exists.");
        }

        // Access a branch
        let db_config = store.branch("database");
        match db_config.get_into::<String>("password") { // Relative path, returns Result
            Ok(password) => println!("Password from branch retrieved."), // Don't print the actual password!
            Err(_) => println!("Password not found or couldn't be read as string."),
        }

        // (See below for Value Provider registration with store_mgr)

        println!("Current root path: {}", store.current_key_path()); // ""
        println!("Current branch path: {}", db_config.current_key_path()); // "database"

        // The store_mgr goes out of scope here, stopping provider refreshes.
        // Keep it alive if providers need to refresh.
        Ok(())
    }
    ```

## Configuration Files & Directories

C5Store loads configuration from specified paths in the `create_c5store` call. These paths can be:

*   **YAML files** (`.yaml`, `.yml`)
*   **TOML files** (`.toml`)
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

# service.name and database.pool_size are inherited
```

## Environment Variables & Loading Priority

C5Store supports overriding configuration values using environment variables after all files have been loaded and merged.

*   **Prefix:** Variables starting with `C5_` (by default, can be configured later if needed) are processed.
*   **Separator:** Double underscore (`__`) is used to denote nesting levels (e.g., `C5_DATABASE__HOST` maps to `database.host`).
*   **Case:** The key derived from the environment variable is converted to lowercase (e.g., `C5_SERVICE__NAME` becomes `service.name`).
*   **Value:** Environment variable values are always treated as **strings**. Use `get_into` or `get_into_struct` to convert them to the desired type.

**Loading Priority (Highest to Lowest):**

1.  **Environment Variables** (e.g., `C5_...`)
2.  **Configuration Files/Directories** (processed in the order specified/discovered, with later files/directories overriding earlier ones).
3.  **(Future)** Default values set programmatically.

## Optional Features (`dotenv`, `secrets`)

C5Store uses Cargo features to enable optional functionality, keeping the core library lean if certain features aren't needed.

*   **`dotenv`**:
    *   Enables loading environment variables from a `.env` file at startup.
    *   Requires the `dotenvy` crate.
    *   Enable using `features = ["dotenv"]` in `Cargo.toml`.
    *   Specify the path to the `.env` file via `C5StoreOptions::dotenv_path`.
    *   `.env` files are loaded *before* process environment variables are read, allowing process variables to override `.env` variables.
*   **`secrets`**:
    *   Enables all secrets management functionality (loading `.c5encval`, `SecretOptions`, `SecretKeyStore`, decryptors).
    *   Requires crypto dependencies (`ecies_25519`, `curve25519-parser`, `sha2`).
    *   **Enabled by default.**
    *   Disable using `default-features = false` in `Cargo.toml` if secrets are not needed, resulting in a smaller binary.

```toml
[dependencies]
# Minimal - no .env, no secrets
# c5store = { version = "0.3.0", default-features = false }

# Default - secrets enabled
# c5store = "0.3.0"

# Secrets and .env support
# c5store = { version = "0.3.0", features = ["dotenv"] }
```

## Secrets Management (`secrets` feature)

*(This section requires the `secrets` feature, which is enabled by default).*

Secrets are defined using a special `.c5encval` key within your YAML/TOML configuration.

**Structure:**

```yaml
# YAML Example
some_secret_key:
  .c5encval: ["<algorithm>", "<key_name>", "<base64_encrypted_data>"]

# TOML Example (requires inline table or separate table)
# [some_secret_key]
# ".c5encval" = ["<algorithm>", "<key_name>", "<base64_encrypted_data>"]
```

*   **`<algorithm>`:** Name of registered `SecretDecryptor` (e.g., `"base64"`, `"ecies_x25519"`).
*   **`<key_name>`:** Name used to look up the decryption key in the `SecretKeyStore`.
*   **`<base64_encrypted_data>`:** The secret value, encrypted and then Base64 encoded.

**Configuration (`SecretOptions`):**

Configure secrets via the `secret_opts` field in `C5StoreOptions`.

```rust
use c5store::{C5StoreOptions, SecretOptions, create_c5store};
#[cfg(feature = "secrets")] // Only if using secrets explicitly
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
        // Filename (without extension) becomes the key_name.
        secret_keys_path: Some(PathBuf::from("path/to/your/secret_keys")),

        // Override the special key identifying secrets.
        secret_key_path_segment: Some(".c5encval".to_string()), // Default

        // Programmatically configure the SecretKeyStore.
        secret_key_store_configure_fn: Some(Box::new(|key_store: &mut SecretKeyStore| {
            // Register standard decryptors if needed (built-ins might be added automatically later)
            key_store.set_decryptor("base64", Box::new(Base64SecretDecryptor {}));
            key_store.set_decryptor(
                "ecies_x25519",
                Box::new(EciesX25519SecretDecryptor::new(EciesX25519::new()))
            );
            // key_store.set_key("manual_key", vec![...]);
        })),

        // --- New in 0.3.0 ---
        // Enable loading keys from environment variables.
        load_secret_keys_from_env: true,
        // Prefix for environment variables holding keys (e.g., C5_SECRETKEY_MYAPIKEY).
        // Value should be base64 encoded key bytes.
        secret_key_env_prefix: Some("C5_SECRETKEY_".to_string()), // Default prefix
    };
}

let config_paths = vec![/* ... */ PathBuf::from("secrets.yaml")];
let (store, mut store_mgr) = create_c5store(config_paths, Some(options))?;

// Retrieving the secret automatically attempts decryption if secrets feature enabled
match store.get_into::<Vec<u8>>("api_credentials.token") { // Use get_into for Vec<u8>
    Ok(token_bytes) => println!("Decrypted API Token retrieved."),
    Err(e) => println!("Failed to get/decrypt API token: {}", e),
}
```

## Value Providers

*(Functionality unchanged from previous version, see earlier examples)*

Value providers allow parts of your configuration to be loaded dynamically from external sources. Mark a section in YAML/TOML with `.provider`. Register providers using `C5StoreMgr::set_value_provider`.

## Change Notifications

*(Functionality unchanged from previous version, see earlier examples)*

Subscribe to changes using `C5Store::subscribe`. Listeners are called after a debounce period.

## License

This project is licensed under the **Mozilla Public License Version 2.0 (MPL-2.0)**.

## Contributing

Contributions welcome! Please open issues or PRs.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for a history of notable changes. (Remember to update this file!)