# Usage Guide: c5store

How to load configuration from files and the environment, read it back as typed Rust values, decrypt secrets, defer sections to providers and react to changes.

## Table of Contents

* [Core Concepts](#core-concepts)
* [Quick Start](#quick-start)
  * [Reading typed values](#reading-typed-values)
  * [Deserializing a section into a struct](#deserializing-a-section-into-a-struct)
* [Loading Configuration Files](#loading-configuration-files)
* [Fetching Missing Config Files](#fetching-missing-config-files)
* [Overriding From the Environment](#overriding-from-the-environment)
* [Choosing the Environment Key Case](#choosing-the-environment-key-case)
* [Reading Values](#reading-values)
* [Deserializing Into Structs](#deserializing-into-structs)
* [Controlling Map and Array Inference](#controlling-map-and-array-inference)
* [Scoping Reads With Branches](#scoping-reads-with-branches)
* [Tracing Where a Value Came From](#tracing-where-a-value-came-from)
* [Decrypting Secrets](#decrypting-secrets)
  * [Loading keys from a directory](#loading-keys-from-a-directory)
  * [Loading keys from environment variables](#loading-keys-from-environment-variables)
  * [Loading keys from systemd credentials](#loading-keys-from-systemd-credentials)
* [Loading Sections Through Value Providers](#loading-sections-through-value-providers)
* [Subscribing to Changes](#subscribing-to-changes)
* [Loading a .env File](#loading-a-env-file)
* [Plugging In Logging and Stats](#plugging-in-logging-and-stats)
* [Error Handling](#error-handling)

## Core Concepts

* **Key path** is a dot-separated address for one value, such as `database.connection.pool_size`. Every read takes one.
* **Store** is the read side, the `C5Store` trait, implemented by `C5StoreRoot` and `C5StoreBranch`.
* **Store manager** is `C5StoreMgr`, the write and lifecycle side. It owns value providers and their refresh timers; dropping it stops them.
* **Branch** is a view of the store rooted at a key path, so reads inside it take paths relative to that root.
* **Flattening** is how the store holds data internally: every leaf lives under its full dotted path, whatever shape the source had.
* **Reconstruction** is the reverse, rebuilding nested maps and arrays out of flattened keys when you deserialize a section.
* **Array inference** turns a set of sibling keys into an array when they are sequential integers counting from `0`, and into a map otherwise.
* **`#map` suffix** appended to a key overrides that inference and forces the children to stay a map.
* **Value provider** is a named source that fills in a config section at load time, and optionally refreshes it on a timer.
* **Provider directive** is the `.provider` key inside a config section that names which provider fills it.
* **Secret** is a value written in the config as a `.c5encval` array of algorithm, key name and base64 ciphertext, decrypted while the store loads.
* **Secret key store** is `SecretKeyStore`, the registry mapping decryptor names to implementations and key names to key bytes.
* **`ref_key_name`** is the logical name a decryption key is filed under, and the second element of every `.c5encval` array.
* **Config source** is the origin recorded for each value: a file, an environment variable, a provider or a programmatic set.
* **Change listener** is a callback fired after a debounce window when a value at or under a subscribed path changes.

## Quick Start

### Reading typed values

```rust
use c5store::{create_c5store, C5Store};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let paths = vec![
    PathBuf::from("config/common.yaml"),
    PathBuf::from("config/local.yaml"),
  ];

  let (store, _store_mgr) = create_c5store(paths, None)?;

  let host: String = store.get_into("database.host")?;
  let pool_size: u64 = store.get_into("database.pool_size")?;

  println!("{host} with {pool_size} connections");
  Ok(())
}
```

`_store_mgr` must stay alive for provider refreshes to keep running. Bind it, do not discard it with `_`, when you register providers.

### Deserializing a section into a struct

```rust
use c5store::{create_c5store, C5Store};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize, Debug)]
struct ServiceConfig {
  name: String,
  port: u16,
  #[serde(default)]
  threads: u32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let (store, _store_mgr) = create_c5store(vec![PathBuf::from("config/")], None)?;

  let service: ServiceConfig = store.get_into_struct("service")?;
  println!("{service:?}");
  Ok(())
}
```

## Loading Configuration Files

Pass files, directories or both. A directory contributes every file in it with a supported extension, in alphabetical order.

```rust
let paths = vec![
  PathBuf::from("config/common.yaml"),
  PathBuf::from("config/defaults.toml"),
  PathBuf::from("config/environments/"),
  PathBuf::from("config/local.yaml"),
];
let (store, _mgr) = create_c5store(paths, None)?;
```

`.yaml` and `.yml` are always understood. `.toml` needs the `toml` feature. Files with any other extension are skipped silently, and a path that does not exist is not an error.

Later sources win. Maps merge recursively; every other type is replaced whole, so an array in a later file replaces the earlier array rather than appending to it.

```yaml
# config/common.yaml
service:
  name: MyAwesomeApp
  port: 8080
database:
  host: prod-db.example.com
  pool_size: 50
```

```toml
# config/local.toml, read after common.yaml
service.port = 9090

[database]
host = "localhost"
user = "dev_user"
```

The result has `service.name` and `database.pool_size` from the first file, `service.port` and `database.host` overridden by the second and `database.user` added by it.

For the conventional five-file layout there is a helper that builds the path list:

```rust
use c5store::default_config_paths;

// config/common.yaml, config/production.yaml, config/prod.yaml,
// config/us-east.yaml, config/prod-us-east.yaml
let paths = default_config_paths("config", "production", "prod", "us-east");
```

## Fetching Missing Config Files

Needs the `bootstrapper` feature, which brings in `tokio`, `reqwest` and `url`. `ConfigBootstrapper` writes files that are not on disk yet, taking them from a local path, an HTTP URL or a Git repository, so that `create_c5store` has something to read on a first run or in a fresh environment.

```rust
use c5store::bootstrapper::{BootstrapItem, ConfigBootstrapper, GitHost};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let config_dir = PathBuf::from("config");

  ConfigBootstrapper::new(
    Some(PathBuf::from("templates")),
    Some("https://github.com/your-org/app-config".to_string()),
  )
  .add_item(BootstrapItem::new_git(
    None,
    GitHost::GitHub,
    "main".to_string(),
    "common.yaml",
    config_dir.join("common.yaml"),
  ))
  .add_item(BootstrapItem::new_http(
    "https://example.com/configs/regions.json".to_string(),
    config_dir.join("regions.json"),
  ))
  .add_item(BootstrapItem::new_local(
    "local.example.yaml",
    config_dir.join("local.yaml"),
  ))
  .run()
  .await?;

  Ok(())
}
```

Create the store from those paths afterwards, in the same process or a later one.

An item whose target already exists is skipped, so this is safe to run on every start and a file edited locally is never overwritten. The first failure aborts the run, leaving the files written before it in place.

The two arguments to `new` are a base directory for local sources and a default repository. The base is joined onto every `new_local` path. The default repository serves any `new_git` item that passes `None` for its own repo URL, so a set of files from one repository names it once.

Git items are resolved to a raw URL and fetched over plain unauthenticated HTTP, which reaches public repositories only, and a download that fails reports an HTTP error rather than a Git one. The host in the URL is discarded, so `https://gitlab.example.com/team/repo` still fetches from `gitlab.com`: point self-hosted GitLab, Gitea or GitHub Enterprise at `new_http` with the raw URL written out in full.

Progress lines go straight to stdout rather than through `log`, so they appear whether or not a logger is installed.

The shipped example runs with `cargo run --example bootstrap --features bootstrapper`.

## Overriding From the Environment

Environment variables are applied after every file has been read and merged, so they win over all of them.

```sh
export C5_DATABASE__HOST=env-host.example.com
export C5_DATABASE__POOL_SIZE=200
export C5_SERVICE__DEBUG=true
```

`C5_` marks a variable as ours and is stripped. `__` separates path segments. The value is parsed into the narrowest type that fits, trying boolean, then `i64`, then `u64`, then `f64`, and falling back to a string.

```rust
let pool_size: u64 = store.get_into("database.pool_size")?; // 200, not "200"
let debug: bool = store.get_into("service.debug")?;         // true, not "true"
```

A variable whose name yields an empty path segment, such as `C5_DATABASE____HOST`, is skipped with a warning rather than failing the load.

## Choosing the Environment Key Case

Each `__`-separated segment is case-converted before it becomes a key path segment. The default is `Case::Camel`, which matches `#[serde(rename_all = "camelCase")]` structs.

```rust
use c5store::{create_c5store, C5StoreOptions, Case};

let mut options = C5StoreOptions::default();
options.env_case = Case::Snake;

let (store, _mgr) = create_c5store(paths, Some(options))?;
```

Given `C5_MY_VAR__API_CLIENT__USER_NAME`, the four cases produce:

| `env_case` | Resulting key path |
|---|---|
| `Case::Camel` (default) | `myVar.apiClient.userName` |
| `Case::Snake` | `my_var.api_client.user_name` |
| `Case::Kebab` | `my-var.api-client.user-name` |
| `Case::Lower` | `myvar.apiclient.username` |

This affects environment variables only. Keys read from files are used exactly as written.

## Reading Values

`get` returns the raw value and clones it; `get_ref` borrows it along with its source and clones nothing; `get_into` converts.

```rust
use c5store::value::C5DataValue;

// Raw, cloned.
if let Some(C5DataValue::String(host)) = store.get("database.host") {
  println!("{host}");
}

// Borrowed, plus the source, no clone.
if let Some(value_ref) = store.get_ref("database.host") {
  println!("{:?} from {:?}", value_ref.value(), value_ref.source());
}

// Converted.
let host: String = store.get_into("database.host")?;
```

Reach for `get_into` by default, `get_ref` when the value is large enough that the clone matters and `get` when you want to branch on the `C5DataValue` variant yourself.

One sharp edge is worth knowing before you pick a type. A non-negative integer read from a file is stored as `UInteger`, but the same value arriving from an environment variable is stored as `Integer`, because env parsing tries `i64` first. The narrow conversions check the variant exactly, so a call that works against the file fails once someone overrides the key from the environment:

```rust
// service.port is 8080 in config.yaml
let port: u16 = store.get_into("service.port")?;  // Ok

// C5_SERVICE__PORT=8080 now overrides it
let port: u16 = store.get_into("service.port")?;  // Err(TypeMismatch { expected: UInteger, found: Integer })
let port: u64 = store.get_into("service.port")?;  // Ok, u64 and i64 accept either variant
```

Use `u64` or `i64` with `get_into`, or use `get_into_struct`, for anything an environment variable may override. Both accept either variant. The narrow types are safe only for values that always come from a file.

Existence comes in two flavours, and the difference matters:

```rust
store.exists("database.host");      // true only if that exact key holds a value
store.path_exists("database");      // true if that key exists or anything sits under it
```

To see what is actually loaded, list the keys:

```rust
for key in store.key_paths_with_prefix(Some("database")) {
  println!("{key}");
}

let everything = store.key_paths_with_prefix(None);
```

## Deserializing Into Structs

`get_into_struct` rebuilds a nested value from whatever sits under the key path and hands it to serde, so the same struct loads from a nested YAML file and from flat environment variables without changing.

```yaml
# From a file
web:
  loadbalancer: lb.site.com
  servers:
    - ip: 1.1.1.1
      port: 80
    - ip: 2.2.2.2
      port: 8080
```

```sh
# Or from the environment, for the same struct
export C5_WEB__LOADBALANCER=lb.site.com
export C5_WEB__SERVERS__0__IP=1.1.1.1
export C5_WEB__SERVERS__0__PORT=80
export C5_WEB__SERVERS__1__IP=2.2.2.2
export C5_WEB__SERVERS__1__PORT=8080
```

```rust
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

let web: WebConfig = store.get_into_struct("web")?;
```

Pass `""` as the key path to deserialize the whole store into one struct.

Map keys are parsed rather than kept as strings, so a YAML map with numeric keys lands in a `HashMap<u32, _>`:

```yaml
milestoneContractsByTier:
  2: ["reach_100k_net_worth"]
  5: ["first_derivative", "unlock_foreign_exchange"]
  10: ["become_a_millionaire"]
```

```rust
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MilestoneConfig {
  milestone_contracts_by_tier: HashMap<u32, Vec<String>>,
}
```

Boolean fields accept more than `true` and `false`. `yes`, `on` and `1` all read as true; `no`, `off` and `0` read as false, in any case. A decrypted secret held as bytes deserializes into a `String` field when the bytes are valid UTF-8, and fails with a `ConversionError` when they are not.

## Controlling Map and Array Inference

Sibling keys become an array when they are sequential integers starting at `0`, and a map otherwise.

```sh
export C5_APP__SERVERS__0=alpha.example.com   # array, keys 0 and 1
export C5_APP__SERVERS__1=beta.example.com

export C5_APP__TIERS__5=Standard              # map, keys do not start at 0
export C5_APP__TIERS__10=Premium
```

When keys genuinely are `0`, `1`, `2` but mean roles rather than positions, that inference is wrong. Append `#map` to the parent key to force a map:

```yaml
"eventHandlers#map":
  0: "on_start"
  1: "on_message"
  2: "on_shutdown"
```

```sh
export "C5_APP__EVENT_HANDLERS#map__0=on_start"
export "C5_APP__EVENT_HANDLERS#map__1=on_message"
```

The suffix is consumed during reconstruction, so the field is still named `eventHandlers` and deserializes into a `HashMap<u8, String>`.

## Scoping Reads With Branches

A branch is a view rooted at a key path. Reads on it take paths relative to that root.

```rust
let db = store.branch("database");

let host: String = db.get_into("host")?;        // reads database.host
let user: String = db.get_into("user")?;        // reads database.user

println!("{}", db.current_key_path());          // "database"
```

Branches nest, and every `C5Store` method is available on them, subscriptions included. `current_key_path` returns `""` on the root store.

## Tracing Where a Value Came From

Every value carries its origin, which is how you answer "why is this not what the file says".

```rust
use c5store::ConfigSource;

match store.get_source("database.host") {
  Some(ConfigSource::EnvironmentVariable(name)) => println!("overridden by {name}"),
  Some(ConfigSource::File(path)) => println!("from {}", path.display()),
  Some(ConfigSource::Provider(name)) => println!("supplied by provider {name}"),
  Some(other) => println!("{other}"),
  None => println!("not set"),
}
```

`ConfigSource` implements `Display`, so `println!("{}", source)` works for logging. For values that came from a file the path recorded is the file that contributed the top-level key.

## Decrypting Secrets

*Requires the `secrets` feature, which is on by default.*

A secret is a three-element array under a `.c5encval` key: the decryptor name, the key name to decrypt it with and the base64 ciphertext.

```yaml
database:
  host: db.prod.example.com
  password:
    .c5encval: ["ecies_x25519", "my_app", "gAAAAA...base64..."]
```

The store decrypts while it loads and replaces the whole `.c5encval` map with the plaintext bytes, so you read the key that held it and the `.c5encval` key no longer exists:

```rust
let password: String = store.get_into("database.password")?;
assert!(!store.exists("database.password.c5encval"));
```

Register the decryptors you use, then choose where the keys come from:

```rust
use c5store::{create_c5store, C5StoreOptions, SecretOptions};
use c5store::secrets::{Base64SecretDecryptor, EciesX25519SecretDecryptor, SecretKeyStore};
use ecies_25519::EciesX25519;

let mut options = C5StoreOptions::default();
options.secret_opts = SecretOptions {
  secret_key_store_configure_fn: Some(Box::new(|store: &mut SecretKeyStore| {
    store.set_decryptor("base64", Box::new(Base64SecretDecryptor {}));
    store.set_decryptor(
      "ecies_x25519",
      Box::new(EciesX25519SecretDecryptor::new(EciesX25519::new())),
    );
  })),
  ..Default::default()
};
```

A secret whose decryptor or key name is not registered decrypts to `C5DataValue::Null` with an error logged, rather than failing the load. Read it back as `Null` if you need to detect that.

`base64` is not encryption. It is there for tests and fixtures; use `ecies_x25519` for anything real.

To change the marker key from `.c5encval`, set `secret_key_path_segment`.

### Loading keys from a directory

The filename minus its last extension becomes the key name, so `my_app.pem` files a key under `my_app`. Files ending in `.pem` are parsed as OpenSSL X25519 private keys; everything else is taken as raw key bytes.

```rust
options.secret_opts.secret_keys_path = Some(PathBuf::from("/etc/myapp/keys"));
```

A directory that does not exist logs a warning and loads nothing. A path that exists but is not a directory is an error. Note that the extension stripped is only the last one, so `my_app.c5.key.pem` files its key under `my_app.c5.key`, not `my_app`.

### Loading keys from environment variables

The variable name minus the prefix is lowercased to form the key name, and the value must be base64-encoded key bytes.

```rust
options.secret_opts.load_secret_keys_from_env = true;
options.secret_opts.secret_key_env_prefix = Some("C5_SECRETKEY_".to_string()); // the default
```

```sh
export C5_SECRETKEY_MY_APP="$(base64 < my_app.key)"   # files a key under "my_app"
```

A value that is not valid base64 is logged as an error and skipped.

### Loading keys from systemd credentials

*Requires the `secrets_systemd` feature, which is off by default on every target.*

This is the production path on Linux: the private key never sits on the filesystem in plaintext.

Encrypt the key on the target host, where `myapp.private.key` is the credential name:

```sh
cat my_app.c5.key.pem | systemd-creds encrypt - /etc/credstore.encrypted/myapp.private.key
```

Load it in the unit file under the same name, then `systemctl daemon-reload`:

```ini
# /etc/systemd/system/myapp.service
[Service]
DynamicUser=yes
LoadCredential=myapp.private.key
ExecStart=/usr/bin/myapp-server
```

Map the credential name to the `ref_key_name` your config refers to:

```rust
use c5store::secrets::systemd::{KeyFormat, SystemdCredential};

options.secret_opts.load_credentials_from_systemd = vec![SystemdCredential {
  credential_name: "myapp.private.key".to_string(),
  ref_key_name: "my_app".to_string(),
  format: KeyFormat::PemX25519,
}];
```

`format` says what the credential holds. `KeyFormat::Raw`, the default, uses the bytes as they arrive. `KeyFormat::PemX25519` parses the PEM text and extracts the raw 32-byte key, which is what you want when you encrypted a `.pem` file as above.

Two failure modes are worth knowing. Without the `secrets_systemd` feature this list is accepted and then ignored, with no key loaded and no error; check the feature first when secrets decrypt to `Null` in production. And when the feature is on but `CREDENTIALS_DIRECTORY` is unset, meaning the unit has no `LoadCredential=`, the store logs a warning and carries on. A credential that is named but unreadable or unparseable does fail the load.

## Loading Sections Through Value Providers

A provider fills a config section from somewhere outside the config files. Mark the section with `.provider` naming the provider, then register an implementation under that name.

```yaml
market:
  regions:
    .provider: resource
    path: regions.yaml
    format: yaml

templates:
  welcome:
    .provider: resource
    path: welcome.txt
```

```rust
use c5store::providers::C5FileValueProvider;

let (store, mut store_mgr) = create_c5store(paths, None)?;

store_mgr.set_value_provider(
  "resource",
  C5FileValueProvider::default("data_files/"),
  300,
);

let regions: Vec<RegionData> = store.get_into_struct("market.regions")?;
```

The third argument is the refresh interval in seconds; `0` registers the provider without a timer. Refreshes run on a background thread owned by `C5StoreMgr`, so dropping the manager stops them. A refreshed value that differs from the current one fires change notifications.

`C5FileValueProvider::default` arrives with `json` and `yaml` deserializers registered, selected by the `format` key. `C5FileValueProvider::new` registers none, so `format: yaml` will not resolve there. Without a `format` the file content is stored as raw bytes. A `format` naming a deserializer that is not registered logs a warning and skips that entry.

`path` is resolved against the base path given to the constructor unless it is already absolute. Two file-shaped failures behave badly enough to be worth designing around: a relative `path` that does not exist **panics** while resolving, and an absolute `path` that does not exist stores `Null` and then abandons the rest of that provider's entries, so a later section the same provider was going to fill is silently left empty. Check that provider files exist before registering the provider.

To provide values from somewhere else, implement `C5ValueProvider`:

```rust
use c5store::providers::C5ValueProvider;
use c5store::{HydrateContext, SetDataFn};
use c5store::value::C5DataValue;

struct EnvProvider {
  keys: Vec<String>,
}

impl C5ValueProvider for EnvProvider {
  fn register(&mut self, data: &C5DataValue) {
    // `data` is the config map holding .provider, .key and .keyPath
  }

  fn unregister(&mut self, key: &str) {
    self.keys.retain(|k| k != key);
  }

  fn hydrate(&self, set_data_fn: &SetDataFn, _force: bool, context: &HydrateContext) {
    for key in &self.keys {
      match std::env::var(key) {
        Ok(value) => set_data_fn(key, C5DataValue::String(value)),
        Err(e) => context.logger.error(&format!("{key}: {e}"), None),
      }
    }
  }
}
```

`hydrate` is called once at registration and again on every refresh. Call `set_data_fn` once per leaf, or use `HydrateContext::push_value_to_data_store` to push a nested map and let it flatten.

## Subscribing to Changes

Subscribe at a key or at any ancestor of it. An ancestor subscription fires once per changed descendant, and the callback is told both which path it subscribed to and which key actually changed.

```rust
store.subscribe(
  "database",
  Box::new(|notify_key, changed_key, new_value| {
    println!("{notify_key} saw {changed_key} become {new_value:?}");
  }),
);

store.subscribe_detailed(
  "database.pool_size",
  Box::new(|notify_key, changed_key, new_value, old_value| {
    println!("{changed_key}: {old_value:?} -> {new_value:?}");
  }),
);
```

Use `subscribe_detailed` when you need the previous value; `subscribe` otherwise.

Notifications are debounced. Changes are collected and delivered after a quiet period, 500 ms by default, and multiple changes to the same key inside that window collapse to one notification carrying the latest value. Tune it with `change_delay_period`, in milliseconds:

```rust
let mut options = C5StoreOptions::default();
options.change_delay_period = Some(2000);
```

A value set to something equal to what it already held fires nothing.

## Loading a .env File

*Requires the `dotenv` feature.*

```rust
let mut options = C5StoreOptions::default();
options.dotenv_path = Some(PathBuf::from(".env.local"));

let (store, _mgr) = create_c5store(paths, Some(options))?;
```

The file is read before process environment variables, so a variable set in the real environment beats the same variable in the file. A missing file is not an error; a malformed one is, as `ConfigError::DotEnvLoadError`. There is no default path: leave `dotenv_path` as `None` and nothing is loaded.

## Plugging In Logging and Stats

The store logs through the `log` crate by default and records no statistics. Supply your own by implementing the two telemetry traits.

```rust
use c5store::telemetry::{GaugeValue, Logger, StatsRecorder, TagValue};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

struct MetricsRecorder;

impl StatsRecorder for MetricsRecorder {
  fn record_counter_increment(&self, tags: HashMap<String, TagValue>, name: String) {
    // "get_attempts" and "set_attempts" arrive here, tagged group=c5store
  }
  fn record_timer(&self, tags: HashMap<String, TagValue>, name: String, value: Duration) {}
  fn record_gauge(&self, tags: HashMap<String, TagValue>, name: String, value: GaugeValue) {}
}

let mut options = C5StoreOptions::default();
options.stats = Some(Arc::new(MetricsRecorder));
```

Leave `logger` unset for `ConsoleLogger`, which forwards to `log`, and `stats` unset for `StatsRecorderStub`, which discards everything.

## Error Handling

Every fallible call returns `Result<_, ConfigError>`. `ConfigError` implements `std::error::Error` through `thiserror`, so `?` into a `Box<dyn Error>` or `anyhow::Error` works without conversion.

```rust
use c5store::error::ConfigError;

match store.get_into::<u64>("database.pool_size") {
  Ok(size) => println!("{size}"),
  Err(ConfigError::KeyNotFound(_)) => println!("using the default of 10"),
  Err(ConfigError::TypeMismatch { key, expected_type, found_type }) => {
    eprintln!("{key} should be {expected_type}, config has {found_type}");
  }
  Err(ConfigError::ConversionError { key, message }) => {
    eprintln!("{key}: {message}");
  }
  Err(e) => eprintln!("{e}"),
}
```

The variants worth matching on:

| Variant | Raised when |
|---|---|
| `KeyNotFound(String)` | Nothing exists at the key path, and nothing exists under it either |
| `TypeMismatch` | The value is there but is the wrong `C5DataValue` variant |
| `ConversionError` | The variant was right but the value would not convert, such as bytes that are not UTF-8 |
| `DeserializationError` | Serde rejected the reconstructed section |
| `IoError` | A config file or key directory could not be read |
| `YamlParseError` | A `.yaml` or `.yml` file is malformed |
| `TomlParseError` | A `.toml` file is malformed (`toml` feature) |
| `DotEnvLoadError` | The `.env` file is malformed (`dotenv` feature) |
| `Message(String)` | A config-level problem with no better variant, such as a key directory path that is not a directory |

Note the split in how missing things behave. A missing key path is an error at read time, but a secret that cannot be decrypted is not an error at all: whatever went wrong, a bad `.c5encval` shape, an unregistered decryptor, a missing key or a decryptor that failed, the store logs a warning, stores `C5DataValue::Null` and carries on. If a secret comes back `Null`, read the warnings from startup; the read itself will not tell you.

`ConfigError` also declares `SecretKeyNotFound`, `SecretAlgorithmNotFound`, `DecryptionError`, `InvalidSecretConfig`, `EnvVarError` and `Internal`. None of them is constructed anywhere in the crate today, so a match arm for one is unreachable. Do not write code that depends on receiving them.
