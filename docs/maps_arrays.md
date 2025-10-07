# Configuration Structure Inference and Customization

This library offers powerful mechanisms for inferring nested configuration structures from flattened data, primarily from environment variables and configuration files. It aims to be flexible, supporting common data structures like arrays and maps, and providing escape hatches for ambiguous cases.

## 1. Flattened Configuration and Key Paths

Configuration values are accessed using hierarchical `key paths`, where segments are separated by dots (`.`). For example, `database.connection.port`.

## 2. Inferring Arrays vs. Maps

When reconstructing nested structures from flattened key-value pairs, the library uses heuristics to determine whether a segment of the configuration path represents an array (list) or a map (object).

*   **Array Inference:**
    *   A path segment is inferred to be an array if all its constituent keys are **sequential, non-negative integers starting from 0**.
    *   **Example:** If the flattened keys are `config.items.0`, `config.items.1`, `config.items.2`, the library will reconstruct `config.items` as an array.
    *   **YAML Source:** `items: ["value0", "value1", "value2"]`
    *   **Env Var Source:** `C5_CONFIG__ITEMS__0=value0`, `C5_CONFIG__ITEMS__1=value1`

*   **Map Inference:**
    *   A path segment is inferred to be a map if its keys are **not sequential, non-negative integers starting from 0**. This includes:
        *   String keys (e.g., `users.johnDoe`).
        *   Numeric keys that are non-sequential, negative, or do not start at `0` (e.g., `tiers.5`, `config.ports.-10`).
    *   **YAML Source:** `settings: { enableLogging: true, useTls: false }`
    *   **Env Var Source:** `C5_CONFIG__SETTINGS__ENABLE_LOGGING=true`

## 3. The `#map` Suffix: Forcing Map Interpretation

Ambiguity arises when a path segment has sequential, non-negative integer keys, but the desired structure is a map, not an array. For instance, you might have configuration for event handlers where `0` means "startup handler" and `1` means "shutdown handler," which are distinct roles, not array indices.

To resolve this ambiguity and explicitly tell the library to treat a segment as a map *even if its keys look like array indices*, you can append the suffix `#map` to the key **in the flattened representation of your configuration data**.

*   **How it Works:** When processing flattened keys, if a key segment (like `eventHandlers`) is followed by a `#map` suffix (e.g., `eventHandlers#map`), any subsequent integer keys found under that segment (like `0`, `1`, `2`) will be treated as map keys, not array indices.

*   **YAML Source:**
    ```yaml
    eventHandlers#map:
      "0": "on_start"
      "1": "on_message"
      "2": "on_shutdown"
    ```
    Here, `eventHandlers#map` as the key tells the system to build a map. The quoted numeric keys are then parsed as strings and used as map keys.

*   **Environment Variable Source:**
    *   To achieve this with environment variables, you need to include the `#map` suffix in the variable name:
        `C5_RECON__EVENT_HANDLERS#map__0=on_start`
        `C5_RECON__EVENT_HANDLERS#map__1=on_message`
        `C5_RECON__EVENT_HANDLERS#map__2=on_shutdown`
    *   The library's environment variable processing logic recognizes the `#map` suffix and applies the correct directive during structure reconstruction.

## 4. Environment Variable Naming and Case Conversion

Environment variables are a common source of configuration. To map them seamlessly to your structured data (especially with `camelCase` fields in Rust structs), the library provides flexible naming conventions:

*   **Prefix:** Environment variables are typically prefixed with `C5_` (configurable via `C5StoreOptions.secret_key_env_prefix`).
*   **Path Separator:** Double underscores (`__`) are used to denote nested structure levels, mirroring the dot (`.`) in configuration files and key paths.
*   **Case Conversion:** By default, environment variable parts are converted to `camelCase` to match `#[serde(rename_all = "camelCase")]` conventions. This behavior is configurable via `C5StoreOptions.env_case`.
    *   **`Case::Camel` (Default):** `C5_MY_VAR__API_CLIENT__USER_NAME` becomes `myVar.apiClient.userName`.
    *   **`Case::Snake`:** `C5_MY_VAR__API_CLIENT__USER_NAME` becomes `my_var.api_client.user_name`.
    *   **`Case::Kebab`:** `C5_MY_VAR__API_CLIENT__USER_NAME` becomes `my-var.api-client-user-name`.
    *   **`Case::Lower`:** `C5_MY_VAR__API_CLIENT__USER_NAME` becomes `myvarapiclientusername`.
*   **Special Suffixes:** The `#map` suffix for forcing map interpretation is also preserved when processing environment variables. For example, `C5_MY_CONFIG__EVENT_HANDLERS#map__0=handler_a` correctly signals a map.

## Summary of Key Rules

| Feature                       | YAML Syntax                               | Environment Variable                  |
| :---------------------------- | :---------------------------------------- | :------------------------------------ |
| **Array Inference**           | Sequential numeric keys from `0`          | Sequential numeric env vars from `0`  |
| **Map Inference**             | Non-numeric or non-sequential numeric keys | Non-numeric or non-sequential env vars |
| **Force Map Interpretation**  | Key ends with `#map`                      | Env Var Key ends with `#map`          |
| **Nested Structure**          | Dot notation (`a.b.c`)                    | Double underscore (`A__B__C`)         |
| **Case Convention (Default)** | N/A (depends on struct)                   | `camelCase`                           |

This layered approach allows for robust and intuitive configuration management across various sources and data structures.