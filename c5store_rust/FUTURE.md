# Future plans

1.  **Comprehensive Testing:** This is the biggest remaining *code-level* task. We need to ensure all the new features work correctly individually and together, especially with different feature flag combinations.
    *   **Loading Logic:**
        *   Test merging order (files, dirs, env vars).
        *   Test TOML file parsing (`toml` feature).
        *   Test directory loading (empty dir, dir with mixed files, sorting).
        *   Test `.env` loading (`dotenv` feature) and interaction with process env vars.
    *   **Core API:**
        *   Test `get_into` with various types, ensuring `ConfigError` variants are correct (KeyNotFound, TypeMismatch, ConversionError).
        *   Test `get_into_struct` with valid and invalid data/structs, testing `DeserializationError`.
        *   Test `get_source` returns accurate `ConfigSource` for values originating from files and env vars.
    *   **Secrets (`secrets` feature):**
        *   Test loading keys from files *and* environment variables.
        *   Test decryption with different algorithms/keys.
        *   Test error conditions (key not found, bad key, decryption fail).
    *   **Change Notifications:**
        *   Test `subscribe` still works.
        *   Test `subscribe_detailed` receives correct `old_value` (including `None` for initial set).
        *   Test debouncing behavior.
    *   **Feature Combinations:** Test building and running basic functionality with different feature sets enabled/disabled (`default`, `full`, `default-features=false`, `features=["toml"]`, etc.).

2.  **Documentation Finalization:**
    *   **Rustdoc Comments:** Add detailed `///` documentation comments to *all* public items (`create_c5store`, `C5Store` trait methods, `C5StoreOptions`, `SecretOptions`, `ConfigError`, `ConfigSource`, `C5DataValue`, public provider types, etc.). Explain parameters, return values (`Result` variants), feature gating, usage examples, and **especially the breaking changes**.
    *   **README / API Ref Review:** Proofread the updated README.md and Quick API Reference for clarity, accuracy, and completeness based on the final code. Ensure examples are correct.
    *   **`lib.rs` Crate Docs:** Add or update the main crate-level documentation (`//!`) explaining the library's purpose, features, and basic usage.

3.  **Examples (`examples/` directory):** Create small, runnable example programs showcasing:
    *   Basic setup and value retrieval (`get`, `get_into`, `get_into_struct`).
    *   Loading from mixed sources (YAML, TOML, Dirs, Env).
    *   Using `get_source`.
    *   Using `subscribe` and `subscribe_detailed`.
    *   Secrets configuration and usage (`secrets` feature).
    *   `.env` file loading (`dotenv` feature).
    *   Basic file provider usage.

4.  **Refinement & Review:**
    *   **API Ergonomics:** Does the API feel intuitive now? Are the error types helpful enough? Is `get_source` useful in its current form?
    *   **Source Tracking:** Is the current level (tracking file/env var origin) sufficient, or is more granular tracking needed later? (Defer major changes unless blockers found).
    *   **Code Cleanup:** Remove commented-out code, ensure consistent formatting, address any remaining TODOs or warnings.
    *   **Helper Function Placement:** Ensure internal helpers (`_merge`, etc.) are appropriately scoped (`pub(crate)` or private).