# C5Store

[![License: MPL-2.0](https://img.shields.io/badge/License-MPL%202.0-brightgreen.svg)](https://opensource.org/licenses/MPL-2.0)
<!-- Add other badges here if you have them -->

**C5Store** (ConfigStore) is a library providing a **unified, traceable, and dynamic store for configuration and secrets** across multiple programming languages, with implementations currently available for [Rust](./c5store_rust) and [JavaScript](./c5store_js).

# Background

Modern applications often need configuration from multiple sources (files, environment variables, remote systems). Managing this complexity, handling secrets securely, and reacting to configuration changes during runtime can lead to repetitive boilerplate and potential errors. Passing around raw configuration maps makes it hard to track value origins or manage context.

C5Store aims to solve these problems by providing a consistent interface and framework across different languages. It centralizes configuration access, integrates various sources, handles secrets transparently, and enables dynamic reconfiguration through a subscription model.

# Concept

C5Store provides a central store where applications can retrieve configuration values using a consistent API, regardless of the underlying source. Key concepts include:

1.  **Unified Interface:** Access configuration values via simple dot-notation keys (e.g., `database.host`). Create "branches" to work with subsections of the configuration using relative paths.
2.  **Multiple Sources & Formats:** Load initial configuration from various sources with defined precedence:
    *   **Environment Variables:** Override values using process environment variables (e.g., `C5_DATABASE__HOST=...`).
    *   **Configuration Files:** Load from YAML and TOML files.
    *   **Directories:** Load all supported files within specified directories.
    *   **(Optional Feature)** `.env` Files: Load environment variables from `.env` files.
    *   **(Extensible)** Custom sources via Value Providers.
3.  **Value Providers:** A pluggable system to load configuration dynamically from external sources (files, databases, network services). C5Store provides traits/interfaces for creating custom providers, allowing integration with virtually any data source. Includes a built-in File provider.
4.  **Secrets Management (Optional Feature):**
    *   Securely handle sensitive values (like API keys or passwords) embedded within configuration.
    *   Secrets are defined using a special key (default: `.c5encval`) containing `["<algorithm>", "<key_name>", "<base64_encrypted_data>"]`.
    *   Secrets are automatically decrypted when configuration is loaded or refreshed.
    *   Supports pluggable decryption algorithms (built-ins include `base64` decoding and `ecies_x25519`).
    *   Load decryption keys securely from files or environment variables.
5.  **Change Subscription:** Applications can subscribe to changes for specific configuration keys (or entire branches). Listeners are notified (with optional old/new value details) when values change, allowing for dynamic reconfiguration without restarts.
6.  **Source Tracking:** Identify the origin of any configuration value (which file, environment variable, or provider set it).

This approach allows developers to focus on using configuration rather than managing its complex lifecycle.

## Implementations

*   **Rust:** [c5store_rust](./c5store_rust) - The reference implementation, featuring strong typing, feature flags for optional components, and robust error handling.
*   **JavaScript:** [c5store_js](./c5store_js) - Provides a similar API and core concepts for Node.js environments.
    - Warning: API is not feature parity with rust currently
*   **Java:** [c5store_java](./c5store_java) - Provides a similar API and core concepts for Java environments.
    - Warning: API is not feature parity with rust currently

Refer to the specific implementation directories for detailed documentation and usage examples.

# Note

*   A potential future addition could be a command-line interface (CLI) tool to help with encrypting secrets and managing configuration files.
*   Additional built-in Value Providers for common sources (e.g., Consul, Vault, databases) may be added over time or contributed by the community.