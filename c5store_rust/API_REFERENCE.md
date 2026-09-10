# API Reference: c5store

A unified store for configuration and secrets, merging files, environment variables, inline encrypted values and deferred provider sections behind one dot-notation key path.

## Contents

* [1. Initialization](#1-initialization)
  * [create_c5store](#create_c5store)
  * [default_config_paths](#default_config_paths)
  * [C5StoreOptions](#c5storeoptions)
  * [Case](#case)
* [2. Reading Configuration](#2-reading-configuration)
  * [C5Store](#c5store)
  * [C5StoreRoot](#c5storeroot)
  * [C5StoreBranch](#c5storebranch)
  * [C5StoreDataValueRef](#c5storedatavalueref)
* [3. Values and Sources](#3-values-and-sources)
  * [C5DataValue](#c5datavalue)
  * [ConfigSource](#configsource)
* [4. Change Notification](#4-change-notification)
  * [ChangeListener](#changelistener)
  * [DetailedChangeListener](#detailedchangelistener)
* [5. Value Providers](#5-value-providers)
  * [C5StoreMgr](#c5storemgr)
  * [C5ValueProvider](#c5valueprovider)
  * [C5FileValueProvider](#c5filevalueprovider)
  * [C5ValueProviderSchema](#c5valueproviderschema)
  * [C5FileValueProviderSchema](#c5filevalueproviderschema)
  * [ProviderSchemaError](#providerschemaerror)
  * [HydrateContext](#hydratecontext)
  * [SetDataFn](#setdatafn)
  * [C5RawValue](#c5rawvalue)
* [6. Secrets](#6-secrets)
  * [SecretOptions](#secretoptions)
  * [SecretKeyStore](#secretkeystore)
  * [SecretDecryptor](#secretdecryptor)
  * [Base64SecretDecryptor](#base64secretdecryptor)
  * [EciesX25519SecretDecryptor](#eciesx25519secretdecryptor)
  * [SystemdCredential](#systemdcredential)
  * [KeyFormat](#keyformat)
  * [load_secret_key_files](#load_secret_key_files)
* [7. Serialization](#7-serialization)
  * [Deserializer functions](#deserializer-functions)
  * [Conversion functions](#conversion-functions)
  * [SerializationError](#serializationerror)
* [8. Telemetry](#8-telemetry)
  * [Logger](#logger)
  * [ConsoleLogger](#consolelogger)
  * [StatsRecorder](#statsrecorder)
  * [StatsRecorderStub](#statsrecorderstub)
  * [TagValue](#tagvalue)
  * [GaugeValue](#gaugevalue)
* [9. Utilities](#9-utilities)
  * [expand_vars](#expand_vars)
* [10. Bootstrapping](#10-bootstrapping)
  * [ConfigBootstrapper](#configbootstrapper)
  * [BootstrapItem](#bootstrapitem)
  * [ConfigSource (bootstrapper)](#configsource-bootstrapper)
  * [GitSourceDetails](#gitsourcedetails)
  * [GitHost](#githost)
* [11. Error Handling](#11-error-handling)
  * [ConfigError](#configerror)
  * [BootstrapError](#bootstraperror)

## 1. Initialization

### create_c5store

Builds the store. Reads and merges every config file, applies `C5_` environment variables over the result, decrypts secrets and registers nothing yet.

* `fn create_c5store(config_file_paths: Vec<PathBuf>, options: Option<C5StoreOptions>) -> Result<(C5StoreRoot, C5StoreMgr), ConfigError>`

`None` for `options` uses `C5StoreOptions::default()`. Paths may be files or directories; a directory contributes every `.yaml`, `.yml` and (with the `toml` feature) `.toml` file in it, in alphabetical order. Files with other extensions are skipped without warning, and a path that does not exist is not an error. Later paths override earlier ones; maps merge recursively, all other types are replaced whole. Environment variables override every file.

The returned `C5StoreMgr` owns the provider refresh threads. Dropping it stops them.

### default_config_paths

Builds the conventional five-path list. Does not check that any of them exist.

* `fn default_config_paths(config_dir: &str, release_env: &str, env: &str, region: &str) -> Vec<PathBuf>`

Returns, in this order: `{dir}/common.yaml`, `{dir}/{release_env}.yaml`, `{dir}/{env}.yaml`, `{dir}/{region}.yaml`, `{dir}/{env}-{region}.yaml`.

### C5StoreOptions

Store configuration, passed to `create_c5store`. Implements `Default`.

* `logger: Option<Arc<dyn Logger>>` (default `None`, meaning `ConsoleLogger`)
* `stats: Option<Arc<dyn StatsRecorder>>` (default `None`, meaning `StatsRecorderStub`)
* `change_delay_period: Option<u64>` (default `Some(500)`): change-notification debounce **in milliseconds**. `None` is replaced with the default during `create_c5store`.
* `secret_opts: SecretOptions`
* `env_case: Case` (default `Case::Camel`): case applied to each environment variable path segment
* `dotenv_path: Option<PathBuf>` (`dotenv` feature, default `None`): there is no fallback path; `None` loads no `.env` file at all

### Case

Case conversion applied to environment variable name segments. Never applied to keys read from files.

* `Camel`: `USER_NAME` becomes `userName`
* `Snake`: `USER_NAME` becomes `user_name`
* `Kebab`: `USER_NAME` becomes `user-name`
* `Lower`: `USER_NAME` becomes `username`

## 2. Reading Configuration

### C5Store

The read interface, implemented by `C5StoreRoot` and `C5StoreBranch`. On a branch every `key_path` is relative to the branch root.

* `fn get(&self, key_path: &str) -> Option<C5DataValue>`: exact key only, clones the value
* `fn get_ref(&self, key_path: &str) -> Option<C5StoreDataValueRef<'_>>`: exact key only, clones nothing, holds a read lock on the store for the lifetime of the returned value
* `fn get_into<T>(&self, key_path: &str) -> Result<T, ConfigError>` where `C5DataValue: TryInto<T, Error = ConfigError>`: exact key only
* `fn get_into_struct<T>(&self, key_path: &str) -> Result<T, ConfigError>` where `T: DeserializeOwned`: reconstructs a nested value from every key under the path and deserializes it; `""` deserializes the whole store
* `fn exists(&self, key_path: &str) -> bool`: exact key match only
* `fn path_exists(&self, key_path: &str) -> bool`: true if the exact key exists **or** any key sits beneath it
* `fn branch(&self, key_path: &str) -> C5StoreBranch`
* `fn key_paths_with_prefix(&self, key_path: Option<&str>) -> Vec<String>`: `None` lists every key under the current root
* `fn subscribe(&self, key_path: &str, listener: Box<ChangeListener>)`
* `fn subscribe_detailed(&self, key_path: &str, listener: Box<DetailedChangeListener>)`
* `fn current_key_path(&self) -> &str`: `""` on the root
* `fn get_source(&self, key_path: &str) -> Option<ConfigSource>`: exact key only

`get_into_struct` returns `KeyNotFound` only when nothing exists at or under the path. Numeric map keys in the source are parsed, so `HashMap<u32, _>` deserializes from YAML keys written as bare integers. Boolean fields accept `true`/`false`, `yes`/`no`, `on`/`off` and `1`/`0`, case-insensitively. A `String` field accepts decrypted `Bytes` when they are valid UTF-8 and returns `ConversionError` when they are not.

Sibling keys reconstruct as an array only when they are sequential integers starting at `0`; otherwise they become a map. Appending `#map` to the parent key forces a map regardless. The suffix is stripped from the resulting key.

### C5StoreRoot

The root view. `Clone`; clones share the same underlying data.

* Created only by `create_c5store`.
* Implements `C5Store` with absolute key paths.

### C5StoreBranch

A view rooted at a prefix. `Clone`.

* Created by `C5Store::branch`. Branches nest.
* Implements `C5Store` with paths relative to the branch root.

### C5StoreDataValueRef

Borrowed view of a value and its source, returned by `get_ref`. Holds a read guard on the store, so it blocks writes until dropped.

* `fn value(&self) -> Option<&C5DataValue>`
* `fn source(&self) -> Option<&ConfigSource>`

## 3. Values and Sources

### C5DataValue

The dynamic value type every config entry is stored as.

* Variants: `Null`, `Bytes(Vec<u8>)`, `Boolean(bool)`, `Integer(i64)`, `UInteger(u64)`, `Float(f64)`, `String(String)`, `Array(Vec<C5DataValue>)`, `Map(HashMap<String, C5DataValue>)`
* `Integer` holds signed values, `UInteger` unsigned. An environment variable value is parsed as boolean, then `i64`, then `u64`, then `f64`, then left a string, so a non-negative number in range lands in `Integer`, not `UInteger`.
* Implements `From<T>` for `()`, `bool`, `String`, `&str`, `Box<str>`, the integer and float primitives, `Vec<u8>`, `Vec<C5DataValue>` and `HashMap<String, C5DataValue>`
* Implements `TryInto<T>` with `Error = ConfigError` for those types, plus `Vec<T>` for `Vec<u8>`, `bool`, `String`, `Box<str>`, `i64`, `u64`, `f64`, `i32`, `u32` and `f32`

**Which integer variant a value lands in depends on its source, and the narrow conversions do not paper over it.** YAML and TOML store a non-negative integer as `UInteger` and a negative one as `Integer`. An environment variable stores any integer as `Integer`, because parsing tries `i64` before `u64`. So the same key takes a different variant depending on whether a file or an env var set it:

| Call | Value from YAML (`UInteger`) | Same key overridden by env var (`Integer`) |
|---|---|---|
| `get_into::<u64>` | `Ok` | `Ok`, a non-negative `Integer` is accepted |
| `get_into::<i64>` | `Ok`, a `UInteger` in range is accepted | `Ok` |
| `get_into::<u16>` and the other narrow types | `Ok` | **`TypeMismatch`**, expected `UInteger`, found `Integer` |
| `get_into_struct` | `Ok` | `Ok`, the serde path accepts either variant |

Reach for `get_into_struct`, or for `u64` and `i64`, on anything an environment variable may override. The narrow conversions (`i8` through `i32`, `u8` through `u32`, `f32`) also check the variant but **not** the range, so an `Integer(1000)` converted to `i8` wraps silently.

### ConfigSource

Where a value came from. Implements `Display`.

* Variants: `File(PathBuf)`, `EnvironmentVariable(String)`, `Provider(String)`, `SetProgrammatically`, `Unknown`
* `File` names the file that contributed the value's **top-level** key, not necessarily the file that set that exact leaf.

## 4. Change Notification

Listeners registered at a key fire for changes at that key and at any descendant. Notifications are debounced by `change_delay_period`; repeated changes to one key inside the window collapse to a single notification carrying the latest value. Setting a value equal to the current one notifies nothing.

### ChangeListener

* `type ChangeListener = dyn Fn(&str, &str, &C5DataValue) -> () + Send + Sync`
* Arguments: subscribed key path, key that actually changed, new value.

### DetailedChangeListener

* `type DetailedChangeListener = dyn Fn(&str, &str, &C5DataValue, Option<&C5DataValue>) -> () + Send + Sync`
* Arguments: subscribed key path, key that actually changed, new value, previous value. The previous value is `None` when the key did not exist before.

## 5. Value Providers

### C5StoreMgr

Owns registered providers and their refresh threads. Returned by `create_c5store`.

* `fn set_value_provider<P: C5ValueProvider + 'static>(&mut self, name: &str, provider: P, refresh_period_sec: u64)`

`name` must match the `.provider` value in the config section. Registration hydrates the provider once immediately. `refresh_period_sec` of `0` registers no timer. Dropping the manager stops every timer.

### C5ValueProvider

Implement this to fill config sections from a source of your own. `Send + Sync`.

* `fn register(&mut self, data: &C5DataValue)`: called once per config section naming this provider, with the section's map
* `fn unregister(&mut self, key: &str)`
* `fn hydrate(&self, set_data_fn: &SetDataFn, force: bool, context: &HydrateContext)`: called at registration and on every refresh

The map handed to `register` always carries `.provider`, `.key` and `.keyPath`, injected by the loader alongside whatever keys the section itself declared.

### C5FileValueProvider

Fills sections from files on disk.

* `fn new(base_path: &str) -> C5FileValueProvider`: no deserializers registered, so any `format` other than `raw` is skipped with a warning
* `fn default(base_path: &str) -> C5FileValueProvider`: registers the `json` and `yaml` deserializers

Section keys it reads: one of `path` or `paths` (required), `format` (default `"raw"`, meaning the file is stored as `Bytes`) and `encoding` (default `"utf8"`, currently parsed and then unused).

`paths` is a list read in order, each file's keys written over the last one's, so a section can be a ladder of its own. `path` is the single-file form and the two are **mutually exclusive**: a section naming both is refused. They are exclusive because a section is assembled from every config file that mentions it, so accepting both would make the effective order depend on which files happened to contribute which key.

A section the provider cannot read is logged at error and not registered, so the keys it would have filled stay as whatever the files and the environment set. The cases are: neither `path` nor `paths`; both of them; an empty `paths`; a `paths` that is not a list of strings; a `path`, `format` or `encoding` that is not a string.

Constraints worth knowing before you rely on it:

* A relative path that does not resolve **panics** during `hydrate`.
* An absolute path that does not exist sets the key to `Null` and then returns, abandoning every remaining entry and every remaining section this provider was registered for. So every entry in a `paths` must exist.
* A `format` naming an unregistered deserializer logs a warning and skips only that section.
* A path that exists but cannot be read **panics**.

### C5ValueProviderSchema

The three loader-injected keys of a provider section.

* `value_provider: String` (from `.provider`), `value_key_path: String` (from `.keyPath`), `value_key: String` (from `.key`)
* `fn from_map(map: &HashMap<String, C5DataValue>) -> Result<C5ValueProviderSchema, ProviderSchemaError>`: answers `Missing` for a key that is absent and `NotAString` for one that is present but not a string

### C5FileValueProviderSchema

One registered file section.

* `value_schema: C5ValueProviderSchema`, `paths: Vec<String>`, `encoding: String`, `format: String`
* `fn new_raw_utf8(value_schema: C5ValueProviderSchema, path: &str) -> C5FileValueProviderSchema`: one entry in `paths`

### ProviderSchemaError

Why a provider section could not be read. Every variant names the section's key path, since a section is assembled from every config file that mentions it and the offending key is often not in the file being edited.

* Variants: `Missing { key_path, key }`, `NotAString { key_path, key }`, `PathAndPaths { key_path }`, `NoPath { key_path }`, `PathsNotStrings { key_path }`, `PathsEmpty { key_path }`
* Implements `std::error::Error` and `Display`

### HydrateContext

Passed to `hydrate`.

* `logger: Arc<dyn Logger>`
* `fn push_value_to_data_store(set_data_fn: &SetDataFn, key: &str, value: C5DataValue)`: associated function, not a method. Flattens a `Map` to one `set_data_fn` call per leaf; passes anything else through unchanged.

### SetDataFn

* `type SetDataFn = dyn Fn(&str, C5DataValue) + Send + Sync`
* Passed to `hydrate` by reference. Each call writes one key. Writing a value equal to the current one is a no-op and notifies nothing.

### C5RawValue

Undecoded provider payload handed to a deserializer.

* Variants: `Bytes(Vec<u8>)`, `String(String)`
* `C5FileValueProvider` always constructs `Bytes`.

## 6. Secrets

*Requires the `secrets` feature, enabled by default.*

A secret is a `.c5encval` key holding a three-element array: decryptor name, key name, base64 ciphertext. The store decrypts during load and replaces the enclosing map with `Bytes`, so the `.c5encval` key does not exist in the loaded store.

**Every secret failure is silent.** A malformed array, an unregistered decryptor, a missing key or a decryptor error all log a warning, store `C5DataValue::Null` and let the load succeed. No `ConfigError` is returned for any of them.

Decryption results are cached by a hash of the three array elements, so an unchanged secret is decrypted once even across provider refreshes.

### SecretOptions

Part of `C5StoreOptions`. Implements `Default`. Under `#[cfg(not(feature = "secrets"))]` this is an empty struct.

* `secret_key_path_segment: Option<String>` (default `Some(".c5encval")`)
* `secret_keys_path: Option<PathBuf>` (default `None`): directory of key files
* `secret_key_store_configure_fn: Option<Box<dyn FnMut(&mut SecretKeyStore)>>` (default `None`): runs before any key loading, and is where decryptors get registered
* `load_secret_keys_from_env: bool` (default `false`)
* `secret_key_env_prefix: Option<String>` (default `Some("C5_SECRETKEY_")`): values must be base64; the name after the prefix is **lowercased** to form the key name
* `load_credentials_from_systemd: Vec<SystemdCredential>` (default empty): **accepted and silently ignored without the `secrets_systemd` feature**

No decryptor is registered by default. A config using `base64` or `ecies_x25519` without a `secret_key_store_configure_fn` that registers it decrypts to `Null`.

Keys load in a fixed order: `secret_key_store_configure_fn`, then `secret_keys_path`, then environment variables, then `systemd` credentials. Later sources overwrite earlier ones filed under the same key name.

### SecretKeyStore

Registry of decryptors and key material.

* `fn new() -> Self`
* `fn get_decryptor(&self, name: &str) -> Option<&Box<dyn SecretDecryptor>>`
* `fn set_decryptor(&mut self, name: &str, decryptor: Box<dyn SecretDecryptor>)`
* `fn get_key(&self, name: &str) -> Option<&Vec<u8>>`
* `fn set_key(&mut self, name: &str, key: Vec<u8>)`

### SecretDecryptor

* `fn decrypt(&self, encrypted_value: &Vec<u8>, key: &Vec<u8>) -> Result<Vec<u8>, SecretDescryptorError>`
* `encrypted_value` is the **base64 text as bytes**, not decoded ciphertext; a decryptor decodes it itself.
* `SecretDescryptorError` variants: `EncryptionFailed`, `DecryptionFailed`, `DecodeFailed`, `BadKeyPubPriv`.

### Base64SecretDecryptor

Base64-decodes the value and ignores the key entirely. For tests and fixtures; it is not encryption.

* `struct Base64SecretDecryptor {}`

### EciesX25519SecretDecryptor

ECIES over X25519.

* `fn new(ecies25519: EciesX25519) -> Self`

### SystemdCredential

Maps one `systemd` credential to a key name. Available whenever `secrets` is on; acted upon only when `secrets_systemd` is on.

* `credential_name: String`: must match the unit file's `LoadCredential=` name
* `ref_key_name: String`: must match the key name in the `.c5encval` array
* `format: KeyFormat` (`#[serde(default)]`, so it defaults to `Raw`)

With the feature on and `CREDENTIALS_DIRECTORY` unset, loading logs a warning and continues. With the feature on and the variable set, a credential that cannot be read or parsed **fails** `create_c5store`, unlike every other secrets failure.

### KeyFormat

* `Raw` (default): the credential bytes are the key
* `PemX25519`: the credential is a PEM-encoded X25519 private key; the raw 32 bytes are extracted from it

Serializes lowercase (`"raw"`, `"pemx25519"`).

### load_secret_key_files

Loads every file in a directory as a key. Called by `create_c5store` when `secret_keys_path` is set; public for direct use.

* `fn load_secret_key_files(secret_keys_path: Option<&PathBuf>, secret_key_store: &mut SecretKeyStore) -> Result<(), ConfigError>`

The key name is the filename with only its **last** extension removed, so `app.c5.key.pem` files a key under `app.c5.key`. Files ending `.pem` are parsed as OpenSSL X25519 private keys and stored as raw bytes; every other file is stored verbatim. Subdirectories, unreadable files, extensionless files and PEM files that fail to parse are logged and skipped. A path that does not exist logs a warning and returns `Ok`. A path that is not a directory returns `ConfigError::Message`.

## 7. Serialization

### Deserializer functions

Registered on `C5FileValueProvider::default` under the format names `json` and `yaml`.

* `fn deserialize_json(raw_value: C5RawValue) -> Result<C5DataValue, SerializationError>`
* `fn deserialize_yaml(raw_value: C5RawValue) -> Result<C5DataValue, SerializationError>`

### Conversion functions

* `fn serde_yaml_val_to_c5_value(raw_value: serde_yaml::Value) -> C5DataValue`: YAML tagged values become `Null`
* `fn serde_json_val_to_c5_value(raw_value: serde_json::Value) -> C5DataValue`
* `fn toml_value_to_c5_value(toml_value: toml::Value) -> C5DataValue` (`toml` feature): datetimes become `String`

Non-string map keys are stringified. Integers land in `Integer` or `UInteger` by sign.

### SerializationError

* Variants: `Json(serde_json::Error)`, `Yaml(serde_yaml::Error)`, both `#[from]`
* Returned only by the deserializer functions, never by `C5Store` methods.

## 8. Telemetry

### Logger

* `fn debug(&self, message: &str)`
* `fn info(&self, message: &str)`
* `fn warn(&self, message: &str)`
* `fn error(&self, message: &str, backtrace: Option<&dyn Error>)`

### ConsoleLogger

The default. Forwards each level to the `log` crate and discards the `error` backtrace argument.

* `struct ConsoleLogger {}`

### StatsRecorder

* `fn record_counter_increment(&self, tags: HashMap<String, TagValue>, name: String)`
* `fn record_timer(&self, tags: HashMap<String, TagValue>, name: String, value: Duration)`
* `fn record_gauge(&self, tags: HashMap<String, TagValue>, name: String, value: GaugeValue)`

Counters emitted by the store, all tagged `group=c5store`: `get_attempts` on every read, `set_attempts` on every write and `set_secret_attempts` on each secret that reaches decryption. No timers or gauges are emitted.

### StatsRecorderStub

The default. Discards everything.

* `struct StatsRecorderStub {}`

### TagValue

* Variants: `String(String)`, `TypedBytes(String, Vec<u8>)`: the first field of `TypedBytes` names the data type

### GaugeValue

* Variants: `Int8`, `UInt8`, `Int16`, `UInt16`, `Int32`, `UInt32`, `Int64`, `UInt64`, `Int128`, `UInt128`, `Ratio32(Rational32)`

## 9. Utilities

### expand_vars

Shell-style variable expansion for use by dependent libraries.

* `fn expand_vars(template_str: &str, variables: &HashMap<String, String>) -> String`

Variable names are lowercased before lookup. **Panics** on a name that is not in the map.

## 10. Bootstrapping

Behind the `bootstrapper` feature, which pulls in `tokio`, `reqwest` and `url`. Fetches config files that are missing from disk, so that the store has something to read. Nothing here touches `C5Store`: it writes files, and `create_c5store` runs afterwards.

### ConfigBootstrapper

* `fn new(local_source_base_path: Option<PathBuf>, default_git_repo_web_url: Option<String>) -> ConfigBootstrapper`
* `fn add_item(self, item: BootstrapItem) -> Self`
* `fn add_items(self, items: Vec<BootstrapItem>) -> Self`
* `async fn run(&self) -> Result<(), BootstrapError>`

Both `add_` methods take and return `self`, so they chain off `new`.

`local_source_base_path` is joined onto every `ConfigSource::Local` path. When it is `None` those paths are used as given, so a relative one resolves against the process working directory. `default_git_repo_web_url` serves any `ConfigSource::Git` whose own `repo_web_url` is `None`.

`run` processes items in the order they were added, and for each one:

* returns `TargetIsDir` if `target_path` is a directory
* skips the item if `target_path` already exists, leaving that file untouched
* creates the parent directory of `target_path` if it is missing
* fetches the source and writes it to `target_path`

The first error aborts the run, so later items are not processed and files already written are not rolled back. Progress goes to stdout through `println!` rather than through `log` or the crate's `Logger`, so no logging setup suppresses or redirects it.

### BootstrapItem

One file to fetch, and where to put it.

* `source: ConfigSource`, `target_path: PathBuf`
* `fn new_local(source_relative_path: impl AsRef<Path>, target_path: PathBuf) -> BootstrapItem`
* `fn new_http(url: String, target_path: PathBuf) -> BootstrapItem`
* `fn new_git(repo_web_url: Option<String>, host_type: GitHost, reference: String, file_path_in_repo: impl AsRef<Path>, target_path: PathBuf) -> BootstrapItem`

### ConfigSource (bootstrapper)

Distinct from the [`ConfigSource`](#configsource) of section 3, which reports where a loaded value came from.

* `Local(PathBuf)`: a file on disk, copied. A source file that does not exist is `LocalSourceNotFound`, not a skip.
* `Http(String)`: an absolute URL, fetched with a bare `reqwest::get`. No authentication, headers, timeout or redirect policy is exposed.
* `Git(GitSourceDetails)`: a file in a repository, resolved to a raw URL and then fetched exactly like `Http`.

### GitSourceDetails

* `repo_web_url: Option<String>`: the repository's web URL, such as `https://github.com/normano/c5store`. `None` falls back to the bootstrapper's default, and to `GitUrlMissing` when there is no default either.
* `host_type: GitHost`
* `reference: String`: branch, tag or commit
* `file_path_in_repo: PathBuf`: must be relative

Owner and repo are the first two path segments of `repo_web_url`, with a trailing `.git` stripped from the repo. The URL's own host is discarded: raw URLs are always built against `raw.githubusercontent.com` or `gitlab.com`, so a self-hosted GitLab, Gitea or GitHub Enterprise URL fetches from the public host instead and either fails or returns an unrelated repository's file. Point those at `ConfigSource::Http` with an explicit raw URL.

Since a Git item ends in an HTTP fetch, a failed fetch surfaces as `Http`, `HttpStatus` or `HttpBody`. The `GitUrl*` and `GitFilePath*` variants only cover building the URL.

### GitHost

* Variants: `GitHub`, `GitLab`
* Raw URL forms: `https://raw.githubusercontent.com/{owner}/{repo}/{reference}/{path}` and `https://gitlab.com/{owner}/{repo}/-/raw/{reference}/{path}`

## 11. Error Handling

### ConfigError

Returned by every fallible call. Implements `std::error::Error` through `thiserror`, and `serde::de::Error`.

Variants that the crate returns:

* `KeyNotFound(String)`: nothing at or under the key path
* `TypeMismatch { key, expected_type, found_type }`: wrong `C5DataValue` variant
* `ConversionError { key, message }`: right variant, unconvertible value
* `DeserializationError { key, source }`: serde rejected the reconstructed section; `source` is a `serde_json::Error`
* `IoError { path, source }`: a config file or key directory could not be read
* `YamlParseError { path, source }`
* `TomlParseError { path, source }` (`toml` feature)
* `DotEnvLoadError { path, source }` (`dotenv` feature)
* `Message(String)`: config-level problems with no better variant

Variants declared but never constructed anywhere in the crate: `EnvVarError`, `SecretKeyNotFound`, `SecretAlgorithmNotFound`, `DecryptionError`, `InvalidSecretConfig`, `Internal`. A match arm for any of them is unreachable today.

`key` is `"_conversion_"` on errors raised inside a `TryInto` conversion, and `""` on errors raised inside deserialization, because neither has the key path in scope. `get_into_struct` rewrites `TypeMismatch` and `DeserializationError` to carry the real key path before returning them; `get_into` does not.

### BootstrapError

Returned by `ConfigBootstrapper::run`. `bootstrapper` feature. The module also exports `type Result<T, E = BootstrapError>`, shadowing `std::result::Result` for callers that glob-import it.

* `Io { path, source }`: a parent directory could not be created, a local file could not be copied, or a target could not be written. `path` is the source path for a copy and the target path for a write.
* `TargetIsDir(PathBuf)`: `target_path` is a directory
* `LocalSourceNotFound(PathBuf)`: carries the path after `local_source_base_path` was joined
* `Http { url, source }`: the request itself failed
* `HttpStatus { url, status, body }`: a non-success status; `body` is the response text, or `"Could not read error body"` when that could not be read either
* `HttpBody { url, source }`: the response body could not be read
* `GitUrlMissing`: a Git item with no `repo_web_url` and no default on the bootstrapper
* `GitUrlInvalid { url, source }`: `repo_web_url` did not parse as a URL
* `GitUrlNoPath(String)`: the URL has no path segments
* `GitUrlParseError { host, url }`: fewer than two path segments, so there is no owner/repo pair
* `GitFilePathInvalid(PathBuf)`: `file_path_in_repo` is not valid UTF-8
* `GitFilePathNotRelative(PathBuf)`: `file_path_in_repo` is absolute
