# C5Store for Rust

[![License: MPL-2.0](https://img.shields.io/badge/License-MPL%202.0-brightgreen.svg)](https://opensource.org/licenses/MPL-2.0)
[![Crates.io](https://img.shields.io/crates/v/c5store.svg)](https://crates.io/crates/c5store)

C5Store is a unified store for configuration and secrets. It merges YAML and TOML files, applies environment variable overrides, decrypts secrets inline, defers sections to value providers and hands the result back as typed Rust values or deserialized structs, all behind one dot-notation key path. For task-by-task instructions see the [usage guide](README.USAGE.md); for signatures and constraints see the [API reference](API_REFERENCE.md).

## Install

```toml
[dependencies]
c5store = "0"
serde = { version = "1", features = ["derive"] }
```

| Feature | Default | Enables |
|---|---|---|
| `secrets` | yes | Inline `.c5encval` decryption, `SecretOptions`, `SecretKeyStore`, the bundled `base64` and `ecies_x25519` decryptors |
| `secrets_systemd` | no | Reading decryption keys from the `systemd` credential store (Linux) |
| `toml` | no | Parsing `.toml` config files |
| `dotenv` | no | Loading a `.env` file before process environment variables are read |
| `bootstrapper` | no | `ConfigBootstrapper`, which fetches missing config files from a path, URL or Git repository |
| `full` | no | `dotenv`, `toml`, `secrets` and `bootstrapper` together |

`secrets_systemd` is not part of `full` and is not on by default on any target, so enable it explicitly:

```toml
c5store = { version = "0", features = ["secrets_systemd"] }
```

## What to reach for

| What you want to do | What to reach for |
|---|---|
| Read one value as a Rust type | `get_into::<T>(path)` |
| Fill a struct from a config section | `get_into_struct::<T>(path)` |
| Read a value and its origin without cloning | `get_ref(path)` |
| Override any value from the environment | `C5_SECTION__KEY=value` |
| Choose how env var names map to key case | `C5StoreOptions::env_case` |
| Decrypt a secret stored in the config file | `.c5encval` plus `SecretOptions` |
| Hand a decryption key in from `systemd` | `SecretOptions::load_credentials_from_systemd` |
| Load a section from an external file | `.provider` plus `C5FileValueProvider` |
| Layer that file per environment | `paths: [app.toml, "${release_env}.toml"]` plus `with_vars` |
| Refresh a provider's data on a timer | `C5StoreMgr::set_value_provider(.., secs)` |
| React when a value changes | `subscribe` or `subscribe_detailed` |
| Scope every read to one subtree | `branch(path)` |
| Find out where a value came from | `get_source(path)` |
| Keep a numeric-keyed section a map, not an array | the `#map` key suffix |
| Enumerate what is loaded | `key_paths_with_prefix(prefix)` |
| Fetch config files that are not on disk yet | the `bootstrapper` feature |

## Status

Version 1.0.0, Rust edition 2024. The public API is stable and in use; it follows semantic versioning from this release on. See [CHANGELOG.md](CHANGELOG.md) for the release history.

Licensed under the Mozilla Public License 2.0. Issues and pull requests are welcome on the project repository.
