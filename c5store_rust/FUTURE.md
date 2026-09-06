# Future plans

Work left after 1.0.0. The public API is stable from that release on, so anything here that changes existing behaviour waits for 2.0.

## Documentation

Rustdoc is nearly absent: there is no crate-level `//!` block, and `///` comments cover only a handful of items in `options.rs`, `util.rs`, `internal.rs` and `data.rs`. Every public item needs one, covering parameters, `Result` variants and feature gating.

README.md, README.USAGE.md and API_REFERENCE.md are current and need no further work.

## Tests

54 tests: 22 unit tests inline in `lib.rs` and `providers.rs`, and 32 integration tests in `tests/`, which share the `SequenceProvider` mock in `tests/common/mod.rs`.

| File | Covers |
| --- | --- |
| `tests/config_paths.rs` | `default_config_paths` |
| `tests/loading.rs` | merge order, array replacement, directory expansion and its lexical sort, extension filtering, empty and missing paths, TOML |
| `tests/source.rs` | `get_source` for file, environment and provider values |
| `tests/notify.rs` | `subscribe`, `subscribe_detailed`, ancestor fan-out, unchanged writes, debounce coalescing |
| `tests/dotenv.rs` | `.env` loading, process environment precedence, missing file, type inference |
| `tests/secrets_env.rs` | keys from `C5_SECRETKEY_*`, name lowercasing, missing key, invalid base64 |
| `tests/secrets_systemd.rs` | credential loading, unset `CREDENTIALS_DIRECTORY`, missing credential, invalid PEM |

Every feature combination compiles and its tests run: `--no-default-features`, `default`, `full`, `secrets_systemd` and each feature alone. That belongs in CI, which does not exist yet.

Notification tests cost about 3s of the suite because `refresh_period_sec` is whole seconds and a real `old_value` needs a second hydrate.

Still untested:

* the `bootstrapper` module, which has no tests at all
* `C5FileValueProvider`'s panic and abandon paths, listed under Defects below
* `get_into` returning `TypeMismatch`
* `branch`, `key_paths_with_prefix` and `current_key_path`
* `Case` selection for environment keys beyond the one existing unit test

## Examples

`examples/` holds only `bootstrap.rs`. Wanted: basic setup and typed reads, mixed sources, `get_source`, `subscribe`, secrets, `.env` and the file provider.

## Value providers

Consul and Vault providers as their own crates, following `c5store_cove`: blocking `reqwest`, a cheap change check before refetching and poll-based refresh through `set_value_provider`. Both need interior mutability to hold state between refreshes, which is where `c5store_cove` fails today: its `_last_commit_id` can never be assigned because `hydrate` takes `&self`, so its short circuit never fires.

`hydrate` receives `&SetDataFn` rather than an owned handle, so a provider cannot keep it for a background watcher. Consul blocking queries and Vault lease renewal both need that, and granting it is a breaking trait change.

## Defects

`C5FileValueProvider` panics on a relative `path` that does not resolve and on a file it cannot read, and an absolute `path` that does not exist sets the key to `Null` and then abandons every remaining section registered to that provider. API_REFERENCE.md documents all three as current behaviour.

The bootstrapper discards the host of a Git repository URL and always builds raw URLs against `raw.githubusercontent.com` or `gitlab.com`, so a self-hosted GitLab, Gitea or GitHub Enterprise URL fetches from the public host and either fails or returns an unrelated repository's file. Fixing it means a `GitHost` variant carrying its own base URL.

`providers.rs` still swallows a failed `C5ValueProviderSchema::from_map` without logging it.

`ConfigSource` is declared in a private module and never re-exported, so `get_source` returns a type callers cannot name or match on, only `Display`. `C5DataStore::set_data` has no callers and tags its writes `Provider("UnknownProvider")`.

## Error enums

Neither `ConfigError` nor `BootstrapError` is `#[non_exhaustive]`, so every future variant is a breaking addition. Marking them is itself breaking, which means deciding now or waiting for 2.0.

`ConfigError` also declares six variants the crate never constructs: `EnvVarError`, `SecretKeyNotFound`, `SecretAlgorithmNotFound`, `DecryptionError`, `InvalidSecretConfig` and `Internal`.
