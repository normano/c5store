# API Reference: `bootstrapper` Module

This module provides the `ConfigBootstrapper` utility to ensure necessary configuration files exist at specified target paths. If a target file is missing, the bootstrapper can source it from a local path, a direct HTTP(S) URL, or a Git repository. It is enabled using the `"bootstrapper"` feature.

This is particularly useful for applications that need to set up default configurations on first run or in new environments.

## Quick Start / Main Usage

The primary way to use this module is through the `ConfigBootstrapper` struct.

```rust
use my_crate::bootstrapper::{ConfigBootstrapper, BootstrapItem, ConfigSource, GitHost, GitSourceDetails, BootstrapError}; // Adjust `my_crate`
use std::path::PathBuf;
use std::env;

#[tokio::main]
async fn main() -> Result<(), BootstrapError> { // Or use anyhow::Result in an application
    let target_config_dir = env::current_dir().unwrap().join("my_app_configs");

    // Define a default Git repository (web URL) for some config files
    let default_repo = "https://github.com/your-org/app-default-configs";

    let bootstrapper = ConfigBootstrapper::new(
        Some(PathBuf::from(".")), // Base for ConfigSource::Local during development
        Some(default_repo.to_string()),
    )
    .add_item(BootstrapItem::new_git(
        None, // Use the default_repo defined above
        GitHost::GitHub,
        "main".to_string(), // branch
        "common_settings.yaml", // path in repo
        target_config_dir.join("common.yaml"), // target on disk
    ))
    .add_item(BootstrapItem::new_http(
        "https://example.com/configs/special_api_keys.template.json".to_string(),
        target_config_dir.join("api_keys.json"),
    ))
    .add_item(BootstrapItem::new_local(
        "dev_overrides/local_debug.toml", // Relative to local_source_base_path
        target_config_dir.join("debug.toml"),
    ));

    match bootstrapper.run().await {
        Ok(()) => println!("Configuration bootstrapping successful!"),
        Err(e) => {
            eprintln!("Bootstrapping failed: {}", e);
            // Handle specific errors if needed
            match e {
                BootstrapError::LocalSourceNotFound(path) => {
                    eprintln!("A required local file was missing: {:?}", path);
                }
                _ => {}
            }
        }
    }
    Ok(())
}
```

## Core Components

### 1. `ConfigBootstrapper` (Struct)

The main struct responsible for managing and executing a list of `BootstrapItem`s.

**Constructor:**

*   `pub fn new(local_source_base_path: Option<PathBuf>, default_git_repo_web_url: Option<String>) -> Self`
    *   `local_source_base_path`: Optional base path for resolving `ConfigSource::Local` items. If `None`, local paths are treated as absolute or relative to CWD.
    *   `default_git_repo_web_url`: Optional default Git repository web URL (e.g., `https://github.com/your-org/default-configs`) used for `ConfigSource::Git` items when their specific `repo_web_url` is `None`.

**Methods:**

*   `pub fn add_item(mut self, item: BootstrapItem) -> Self`
    Adds a single `BootstrapItem` to be processed. Returns `self` for chaining.
*   `pub fn add_items(mut self, items: Vec<BootstrapItem>) -> Self`
    Adds multiple `BootstrapItem`s. Returns `self` for chaining.
*   `pub async fn run(&self) -> Result<(), BootstrapError>`
    Asynchronously processes all configured `BootstrapItem`s.
    *   Checks if `item.target_path` exists; if so, skips.
    *   Ensures parent directories of `item.target_path` exist.
    *   Sources the file content based on `item.source` (Local, HTTP, or Git) and writes it to `item.target_path`.
    *   Returns `Ok(())` on success (all items processed or skipped), or the first critical `BootstrapError` encountered.

---

### 2. `BootstrapItem` (Struct)

Represents a single file to be bootstrapped, pairing a source with a target destination.

**Fields:**

*   `source: ConfigSource`: Defines where to get the file from (see [`ConfigSource`](#configsource-enum)).
*   `target_path: PathBuf`: The absolute filesystem path where the file should be placed if it doesn't already exist.

**Constructors:**

*   `pub fn new_local(source_relative_path: impl AsRef<Path>, target_path: PathBuf) -> Self`
*   `pub fn new_http(url: String, target_path: PathBuf) -> Self`
*   `pub fn new_git(repo_web_url: Option<String>, host_type: GitHost, reference: String, file_path_in_repo: impl AsRef<Path>, target_path: PathBuf) -> Self`

---

### 3. `ConfigSource` (Enum)

Defines the possible sources from which a configuration file can be obtained.

**Variants:**

*   `Local(PathBuf)`: Source from a local filesystem path (typically relative to `ConfigBootstrapper::local_source_base_path`).
*   `Http(String)`: Source directly from an absolute HTTP(S) URL.
*   `Git(GitSourceDetails)`: Source from a Git repository (see [`GitSourceDetails`](#gitsourcedetails-struct)).

---

### 4. `GitSourceDetails` (Struct)

Contains information to fetch a file from a Git repository.

**Fields:**

*   `repo_web_url: Option<String>`: Optional web URL of the Git repository. If `None`, uses `ConfigBootstrapper::default_git_repo_web_url`.
*   `host_type: GitHost`: The type of Git hosting platform (see [`GitHost`](#githost-enum)).
*   `reference: String`: Git reference (branch, tag, commit).
*   `file_path_in_repo: PathBuf`: Relative path to the file within the repository.

---

### 5. `GitHost` (Enum)

Specifies the Git hosting platform, aiding in URL construction for raw file access.

**Variants:**

*   `GitHub`
*   `GitLab`

---

## Error Handling

### `BootstrapError` (Enum)

Defines the possible errors that can occur during the bootstrapping process.

```rust
#[derive(Error, Debug)]
pub enum BootstrapError {
    Io { path: PathBuf, source: io::Error },
    TargetIsDir(PathBuf),
    LocalSourceNotFound(PathBuf),
    Http { url: String, source: reqwest::Error },
    HttpStatus { url: String, status: reqwest::StatusCode, body: String },
    HttpBody { url: String, source: reqwest::Error },
    GitUrlMissing,
    GitUrlInvalid { url: String, source: url::ParseError },
    GitUrlNoPath(String),
    GitUrlParseError { host: String, url: String },
    GitFilePathInvalid(PathBuf),
    GitFilePathNotRelative(PathBuf),
    GitUnsupportedHostForAutomaticUrl { host: String },
}
```
*(See previous detailed breakdown for variant descriptions if needed, or link to a separate detailed error section).*

---

### `Result<T, E = BootstrapError>` (Type Alias)

A convenience type alias: `std::result::Result<T, BootstrapError>`.