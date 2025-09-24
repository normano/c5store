# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.0] - 2025-09-24

### Added
- Updated skiplist dep to briong in rand-09

## [0.4.7] - 2025-09-10

### Added
- Support for secrets decryption in arrays.

## [0.4.6] - 2025-09-10

### Changed
- get_into() now supports automatic bytes to string conversion

## [0.4.5] - 2025-09-09

### Changed
- Added KeyFormat to SystemdCredential

## [0.4.4] - 2025-09-09

### Changed
- undefined CREDENTIALS_DIRECTORY for systemd feature is now non fatal

## [0.4.3] - 2025-09-02

### Added
- Transparent deserialization of decrypted secrets when using get_into_struct

## [0.4.2] - 2025-09-01

### Added
- Support to provide secret keys via SystemD

## [0.4.0] - 2025-05-11

### Added
- Introduced ConfigBootstrapper to ensure config files exist, sourcing from local paths, HTTP URLs, or Git repos.

## [0.3.1] - 2025-04-26

### Changed
*   Enhanced `C5Store::get_into_struct<T>()` to support deserialization from configurations where values are provided as flattened keys (e.g., `database.host`, `database.port`) in addition to nested map structures. This allows structs to be populated correctly from environment variables or flattened config files.

## [0.3.0] - 2025-04-23

### Added

*   **Environment Variable Overrides:** Configuration values can now be overridden by setting environment variables (e.g., `C5_DATABASE__HOST=localhost` overrides `database.host`). Env vars have higher priority than files.
*   **Struct Deserialization:** Added `C5Store::get_into_struct<T>(key_path)` method to deserialize a configuration branch directly into a Rust struct using `serde`.
*   **Configuration Source Tracking:**
    *   Added `ConfigSource` enum to represent the origin of a value (File, Environment Variable, Provider, etc.).
    *   Added `C5Store::get_source(key_path)` method to retrieve the source of a configuration value.
    *   Internal storage now tracks the source alongside the value.
*   **Directory Loading:** `create_c5store` now accepts directory paths in `config_paths`. It will load and merge all supported files (`.yaml`, `.yml`, `.toml`) found within, processed alphabetically.
*   **TOML Configuration Files:** Added support for loading configuration from `.toml` files alongside `.yaml` (requires `toml` feature, enabled by default via `full` feature or explicitly).
*   **Optional Feature Flags:**
    *   `dotenv`: Enables loading environment variables from a `.env` file at startup using `C5StoreOptions::dotenv_path`.
    *   `toml`: Enables parsing of `.toml` configuration files.
    *   `secrets`: Enables all secrets management functionality (enabled by default). Can be disabled via `default-features = false` for smaller builds if secrets are not needed.
    *   `full`: Convenience feature to enable `dotenv`, `toml`, and `secrets`.
*   **Secrets Key Loading from Environment:** Added `SecretOptions::load_secret_keys_from_env` and `secret_key_env_prefix` to load base64-encoded secret keys from environment variables (requires `secrets` feature).
*   **Detailed Change Notifications:**
    *   Added `DetailedChangeListener` type alias.
    *   Added `C5Store::subscribe_detailed(key_path, listener)` method. Listeners receive both the new value and the previous value (`Option<&C5DataValue>`).
*   **Natural/Lexicographical Sorting:** Added internal utilities and `NatLexOrderedString` for improved key sorting within the store, prioritizing lexicographical sorting for same-length keys (like ULIDs) and natural sorting otherwise.

### Changed

*   **BREAKING:** `C5Store::get_into<T>(key_path)` now returns `Result<T, ConfigError>` instead of `Option<T>` for better error handling. Callers must update to handle `Result`.
*   **BREAKING:** `create_c5store` now returns `Result<(impl C5Store, C5StoreMgr), ConfigError>` to propagate errors during loading (IO, parsing, secrets, etc.). Callers must update to handle `Result`.
*   **Error Handling:** Introduced `ConfigError` enum for specific error reporting across the library (KeyNotFound, TypeMismatch, IO errors, Parse errors, Deserialization errors, Secrets errors, etc.).
*   **Internal:** `C5DataStore` now stores configuration values internally as `(C5DataValue, ConfigSource)`.
*   **Internal:** `read_config_data` now encapsulates all loading (files, dirs, env vars), merging, provider separation, and initial application of values to the store.
*   **Dependencies:** Added `thiserror`. Made `toml`, `dotenvy`, `ecies_25519`, `curve25519-parser`, `sha2` optional based on features.

### Fixed

*   Resolved potential `RefCell` double-borrow panic within `ChangeNotifier`'s debouncing logic.

## [0.2.7]

### Changed
- Added multiple C5DataValue TryInto and From data type support for (i|u)8-64. Using macros to generate this code.
- Added C5DataValue ref TryInto and From for all types
- C5DataValue::*Integer TryInto can now support conversion from base int or uint when appropriate. No more having to use i64 as the base type to convert to another one. Ideally i64 is used for negative numbers while u64 is used for 0 to u64::max.

### Fixed
- Fix notify_value_change so that it notifies all subscribers on a key.

## [0.2.3]

### Changed
- create_c5store returns C5StoreRoot struct rather than impl trait

## [0.2.2]

### Added
- build_flat_map function and is public for any value providers to use to smash down objects into dot notation
- HydrateContext.push_value_to_data_store is public so value providers can send their deserialized objects to the data store for merging

### Changed
- File Value Provider now merges objects into the data store. Functionality before this was that an object would be put into the data store which get would return an C5Value::Map.

## [0.2.1]

### Changed
- Set SecretOptions fields to public

## [0.2.0]

### Added
- Secrets decryption with ECIES 25519 library.

### Changed
- Tags are now <string, TagValue> instead of <string, string> to reflect the idea that tags can be different datatypes.