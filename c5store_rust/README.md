
# C5Store for Rust

[![License: MPL-2.0](https://img.shields.io/badge/License-MPL%202.0-brightgreen.svg)](https://opensource.org/licenses/MPL-2.0)
<!-- Add other badges here if you have them, e.g., Crates.io version, Build Status -->

C5Store is a Rust library providing a **unified store for configuration and secrets**. It aims to be a single point of access for your application's configuration needs, consolidating values from various sources (like YAML files), handling secrets securely via built-in decryption, and allowing dynamic loading through providers.

The core idea is to simplify configuration management in complex applications by offering a hierarchical, type-aware, and extensible configuration layer.

## Key Features

*   **Unified Access:** Retrieve configuration values using simple dot-notation key paths (e.g., `database.connection.pool_size`).
*   **Type-Safe Retrieval:** Get values converted directly into expected Rust types using `get_into::<T>()`.
*   **Configuration Loading & Merging:** Load configuration from multiple YAML files, merging them intelligently (later files override earlier ones).
*   **Integrated Secrets Management:**
    *   Transparently decrypt secrets defined within configuration files.
    *   Supports pluggable decryption algorithms (includes `base64` and `ecies_x25519`).
    *   Securely load decryption keys (e.g., from PEM files).
*   **Value Providers:** Defer loading of specific configuration sections to external sources (e.g., files, environment variables, remote services) using a provider system. Includes a built-in `C5FileValueProvider`.
*   **Periodic Refresh:** Value providers can be configured to automatically refresh their data at specified intervals.
*   **Change Notifications:** Subscribe to changes in configuration values at specific key paths or their ancestors. Notifications are debounced to prevent flooding.
*   **Hierarchical Structure:** Access nested configuration values easily and create "branches" for context-specific views of the configuration.
*   **Extensible:** Designed with traits for custom value providers and secret decryptors.
*   **Telemetry Hooks:** Basic interfaces for integrating custom logging and statistics recording.

## Getting Started

1.  **Add Dependency:** Add `c5store` to your `Cargo.toml`:

    ```toml
    [dependencies]
    c5store = "0.2.7" # Use the latest desired version
    # Other necessary dependencies like serde, serde_yaml, etc.
    ```

2.  **Basic Usage:**

    ```rust
    use c5store::{create_c5store, C5Store, C5StoreOptions};
    use std::path::PathBuf;

    fn main() {
        // 1. Define configuration file paths
        //    Paths are loaded and merged in the order provided.
        let config_paths = vec![
            PathBuf::from("config/common.yaml"),
            PathBuf::from("config/environment.yaml"), // e.g., development.yaml
            PathBuf::from("config/local.yaml"),      // Local overrides
        ];

        // 2. (Optional) Configure options (e.g., secrets)
        let options = C5StoreOptions::default(); // Use defaults or customize

        // 3. Create the store
        //    `create_c5store` returns the store interface and a manager
        //    for handling providers.
        let (store, mut store_mgr) = create_c5store(config_paths, Some(options));

        // 4. Retrieve values
        if let Some(db_host) = store.get("database.host") {
            println!("Database Host (C5DataValue): {:?}", db_host);
        }

        // Get directly as a specific type
        let pool_size: Option<u64> = store.get_into("database.pool_size");
        println!("Pool Size (u64): {:?}", pool_size.unwrap_or(10));

        // Check existence
        if store.exists("database.user") {
            println!("Database user is configured.");
        }

        // Check if a path prefix exists (e.g., if 'database' or any subkey exists)
        if store.path_exists("database") {
            println!("Database configuration section exists.");
        }

        // Access a branch
        let db_config = store.branch("database");
        let password: Option<String> = db_config.get_into("password"); // Relative path
        println!("Password from branch: {:?}", password);

        // (See below for Value Provider registration with store_mgr)

        println!("Current root path: {}", store.current_key_path()); // ""
        println!("Current branch path: {}", db_config.current_key_path()); // "database"

        // The store_mgr goes out of scope here, stopping provider refreshes.
        // Keep it alive if providers need to refresh.
    }
    ```

## Configuration Files

C5Store primarily loads configuration from YAML files.

*   Files are loaded in the order specified in the `create_c5store` call.
*   Values from later files **override** values from earlier files for the same key path.
*   Maps (objects) are merged recursively. Other types (strings, numbers, arrays) are replaced entirely.

**Example (`common.yaml`):**

```yaml
service:
  name: MyAwesomeApp
  port: 8080
database:
  host: prod-db.example.com
  pool_size: 50
```

**Example (`local.yaml`):**

```yaml
# Overrides common.yaml values
service:
  port: 9090 # Overrides port 8080
database:
  host: localhost # Overrides prod host
  user: dev_user # Adds a new key
# Note: service.name and database.pool_size are inherited from common.yaml
```

## Secrets Management

Secrets are defined using a special `.c5encval` key within your YAML configuration.

**Structure:**

```yaml
some_secret_key:
  .c5encval: ["<algorithm>", "<key_name>", "<base64_encrypted_data>"]
```

*   **`<algorithm>`:** The name of the registered `SecretDecryptor` (e.g., `"base64"`, `"ecies_x25519"`).
*   **`<key_name>`:** The name used to look up the decryption key in the `SecretKeyStore` (e.g., `"service_api_key"`).
*   **`<base64_encrypted_data>`:** The secret value, encrypted and then Base64 encoded.

**Example (`secrets.yaml`):**

```yaml
api_credentials:
  token:
    # This value will be decrypted using the 'ecies_x25519' algorithm
    # with the key named 'api_token_key'
    .c5encval: ["ecies_x25519", "api_token_key", "iQv4jO...VagBFPI="]
  simple_secret:
    # This value will be decoded using the 'base64' algorithm (key name often ignored)
    .c5encval: ["base64", "ignored", "YWJjZA=="] # Decodes to "abcd"
```

**Configuration:**

You configure secrets handling via `C5StoreOptions` and `SecretOptions`:

```rust
use c5store::{C5StoreOptions, SecretOptions, create_c5store};
use c5store::secrets::{SecretKeyStore, Base64SecretDecryptor, EciesX25519SecretDecryptor};
use ecies_25519::EciesX25519; // From the ecies_25519 crate
use std::path::PathBuf;
use std::sync::Arc;

// ... in your setup code ...

let mut options = C5StoreOptions::default();

// Configure Secret Options
options.secret_opts = SecretOptions {
    // Optional: Path to a directory containing decryption key files.
    // - '.pem' files assumed to be OpenSSL X25519 private keys.
    // - Other files treated as raw key bytes.
    // - Filename (without extension) becomes the key_name.
    secret_keys_path: Some(PathBuf::from("path/to/your/secret_keys")),

    // Optional: Override the special key name used to identify secrets.
    secret_key_path_segment: Some(".c5encval".to_string()), // Default

    // Optional: Programmatically configure the SecretKeyStore
    secret_key_store_configure_fn: Some(Box::new(|key_store: &mut SecretKeyStore| {
        // Register standard decryptors
        key_store.set_decryptor("base64", Box::new(Base64SecretDecryptor {}));
        key_store.set_decryptor(
            "ecies_x25519",
            Box::new(EciesX25519SecretDecryptor::new(EciesX25519::new()))
        );

        // You could also manually add keys here:
        // key_store.set_key("manual_key_name", vec![...bytes...]);
    })),
};

let config_paths = vec![/* ... */ PathBuf::from("secrets.yaml")];
let (store, mut store_mgr) = create_c5store(config_paths, Some(options));

// Retrieving the secret automatically decrypts it
let api_token: Option<Vec<u8>> = store.get_into("api_credentials.token");
let simple: Option<Vec<u8>> = store.get_into("api_credentials.simple_secret");

println!("Decrypted API Token: {:?}", api_token); // Should be the raw bytes
println!("Decoded Simple Secret: {:?}", simple); // Should be b"abcd"
```

## Value Providers

Value providers allow parts of your configuration to be loaded dynamically from external sources. You mark a section in your YAML to be handled by a provider using the `.provider` key.

**Example (`config/providers.yaml`):**

```yaml
external_data:
  # This whole 'file_content' section will be replaced by data
  # loaded by the 'resources' provider.
  file_content:
    .provider: resources # Name of the provider to use
    path: "data/my_external_config.json" # Provider-specific config: file path
    format: "json" # Provider-specific config: file format
  more_stuff:
    .provider: resources
    path: "secrets/raw_binary_data"
    # format: "raw" (default if omitted)
```

**Configuration & Usage:**

You need to register providers with the `C5StoreMgr` returned by `create_c5store`.

```rust
use c5store::{create_c5store, C5StoreOptions, C5Store};
use c5store::providers::C5FileValueProvider;
use std::path::PathBuf;
use std::time::Duration; // Needed if keeping mgr alive for refreshes

fn main() {
    let config_paths = vec![
        PathBuf::from("config/common.yaml"),
        PathBuf::from("config/providers.yaml"), // Contains provider definitions
    ];
    let options = C5StoreOptions::default();

    let (store, mut store_mgr) = create_c5store(config_paths, Some(options));

    // Register the 'resources' provider (must match the name in YAML)
    // C5FileValueProvider loads files relative to the provided base path.
    let file_provider_base_path = "path/to/your/resource/files";
    store_mgr.set_value_provider(
        "resources", // Name matching '.provider' in YAML
        C5FileValueProvider::default(file_provider_base_path), // The provider instance
        60 // Refresh interval in seconds (0 for no refresh)
    );

    // Now, values defined by the provider should be available:
    // Assuming 'data/my_external_config.json' contained {"key": "value"}
    let external_value: Option<String> = store.get_into("external_data.file_content.key");
    println!("External JSON Value: {:?}", external_value);

    // Assuming 'secrets/raw_binary_data' contained raw bytes
    let raw_data: Option<Vec<u8>> = store.get_into("external_data.more_stuff");
    println!("External Raw Data Length: {:?}", raw_data.map(|d| d.len()));

    // Keep store_mgr alive if you need providers to refresh automatically.
    // For example, run your main application logic here.
    // std::thread::sleep(Duration::from_secs(300)); // Example: Keep alive
}

```

## Change Notifications

Subscribe to changes on specific key paths. Listeners are called after a short debounce period when a value at or below the subscribed path changes.

```rust
use c5store::{C5Store, C5DataValue};
use std::sync::{Arc, Mutex};

// ... inside your setup where 'store' is available ...

let changed_ports = Arc::new(Mutex::new(Vec::new()));
let changed_ports_clone = changed_ports.clone();

// Subscribe to changes specifically on 'service.port'
store.subscribe("service.port", Box::new(move |notify_key, changed_key, new_value| {
    println!(
        "Listener notified via '{}': Key '{}' changed to {:?}",
        notify_key, // The key path this listener was registered for ("service.port")
        changed_key, // The exact key path that changed ("service.port")
        new_value
    );
    if let C5DataValue::UInteger(port) = new_value {
        changed_ports_clone.lock().unwrap().push(*port);
    }
}));

// Subscribe to any change within the 'database' section
store.subscribe("database", Box::new(|notify_key, changed_key, new_value| {
     println!(
        "Listener notified via '{}': Key '{}' changed to {:?}",
        notify_key, // "database"
        changed_key, // e.g., "database.host" or "database.pool_size"
        new_value
    );
    // React to any change under 'database'
}));

// Later, if something modifies "service.port" (e.g., a provider refresh
// or direct modification if the API allowed it), the first listener
// would be called after the debounce period. If "database.host" changed,
// the second listener would be called.

```

## License

This project is licensed under the **Mozilla Public License Version 2.0 (MPL-2.0)**. See the [LICENSE](LICENSE) file for details (or refer to standard MPL-2.0 text if the file isn't present).

## Contributing

Contributions are welcome! Please feel free to open an issue to discuss bugs or feature requests, or submit a pull request.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for a history of notable changes to this project.