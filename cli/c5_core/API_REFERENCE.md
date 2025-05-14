# `c5_core` API Reference

The `c5_core` crate provides foundational utilities for cryptographic operations, key management, YAML manipulation, and file I/O, primarily designed to support the `c5cli` tool and `c5store` secret management.

## Error Type

All fallible functions in this crate return `Result<T, C5CoreError>`.

*   **`C5CoreError`**: An enum representing various errors that can occur.
    *   `Io(std::io::Error)`: General I/O error.
    *   `IoWithPath { path: PathBuf, source: std::io::Error }`: I/O error with path context.
    *   `PemParse(String)`: Error parsing PEM data.
    *   `KeyLoad(String)`: General error loading a key.
    *   `EciesOperation(ecies_25519::Error)`: Error during ECIES encryption/decryption.
    *   `EciesKeyParse(ecies_25519::KeyParsingError)`: Error parsing an ECIES key.
    *   `Base64Decode(base64::DecodeError)`: Error decoding Base64 data.
    *   `YamlDeserialize(String)`: Error deserializing YAML string (e.g., `YamlLoader::load_from_str`).
    *   `YamlSerialize(String)`: Error serializing YAML to string (e.g., `YamlEmitter::dump`).
    *   `YamlNavigation(String)`: Error navigating or manipulating YAML structure.
    *   `YamlRust2Parse(yaml_rust2::ScanError)`: Lower-level YAML parsing error from `yaml-rust2`.
    *   `UnsupportedAlgorithm(String)`: Algorithm not supported.
    *   `FileExists(PathBuf)`: Attempted to write to a file that already exists without force.
    *   `Encoding(String)`: Text encoding/decoding error.
    *   `InvalidInput(String)`: Invalid input provided to a function.

## Cryptographic Operations (`crypto_ops.rs`)

*   **Types:**
    *   `CryptoAlgorithm`: Enum for c5store crypto algorithms.
        *   `EciesX25519`
    *   `EciesPublicKey` (re-export of `ecies_25519::PublicKey`)
    *   `EciesStaticSecret` (re-export of `ecies_25519::StaticSecret`)

*   **Functions:**
    *   `encrypt_data(plaintext: &[u8], public_key: &EciesPublicKey, algo: CryptoAlgorithm, rng: &mut (impl RngCore + CryptoRng)) -> Result<Vec<u8>, C5CoreError>`
        *   Encrypts `plaintext` using the given ECIES `public_key` and `algo`.
        *   Requires a cryptographically secure random number generator `rng`.
    *   `decrypt_data(ciphertext: &[u8], private_key: &EciesStaticSecret, algo: CryptoAlgorithm) -> Result<Vec<u8>, C5CoreError>`
        *   Decrypts `ciphertext` using the given ECIES `private_key` and `algo`.

## Key Management (`keys.rs`)

*   **Types:**
    *   `PemEncodedKey(String)`: Wrapper for a PEM-encoded key string.
    *   `KeyPair { public: PemEncodedKey, private: PemEncodedKey }`: Holds a PEM-encoded public/private key pair.
    *   `SshKeyAlgorithm`: Enum for SSH key algorithms.
        *   `Ed25519`
    *   `SshKeyPair { private_key_pem: PemEncodedKey, public_key_openssh_format: String }`: Holds an SSH key pair.

*   **Functions:**
    *   `generate_c5_keypair(algo: CryptoAlgorithm, rng: &mut (impl RngCore + CryptoRng)) -> Result<KeyPair, C5CoreError>`
        *   Generates an ECIES key pair for c5store usage. Returns PEM-encoded keys.
    *   `generate_ssh_keypair(algo: SshKeyAlgorithm, comment_opt: Option<&str>) -> Result<SshKeyPair, C5CoreError>`
        *   Generates an SSH key pair (currently Ed25519).
        *   Private key is PEM-encoded PKCS#8. Public key is in OpenSSH format.
    *   `load_ecies_public_key(key_path: &Path) -> Result<EciesPublicKey, C5CoreError>`
        *   Loads an ECIES public key from a PEM file at `key_path`.
    *   `load_ecies_private_key(key_path: &Path) -> Result<EciesStaticSecret, C5CoreError>`
        *   Loads an ECIES private key from a PEM file at `key_path`.

## I/O Utilities (`io_utils.rs`)

*   **Functions:**
    *   `bytes_to_base64_string(data: &[u8]) -> String`
        *   Encodes byte slice to a Base64 string.
    *   `base64_string_to_bytes(s: &str) -> Result<Vec<u8>, C5CoreError>`
        *   Decodes a Base64 string to bytes.
    *   `read_file_to_bytes(file_path: &Path) -> Result<Vec<u8>, C5CoreError>`
        *   Reads entire file content into a byte vector.
    *   `read_file_to_string(file_path: &Path, encoding_name: &str) -> Result<String, C5CoreError>`
        *   Reads entire file content into a String. Currently, `encoding_name` must be "utf-8" (case-insensitive).
    *   `write_bytes_to_file(file_path: &Path, data: &[u8], force_overwrite: bool) -> Result<(), C5CoreError>`
        *   Writes byte slice to a file. If `force_overwrite` is false and file exists, returns `C5CoreError::FileExists`.
    *   `write_string_to_file(file_path: &Path, content: &str, force_overwrite: bool) -> Result<(), C5CoreError>`
        *   Writes string content (assumed UTF-8) to a file. If `force_overwrite` is false and file exists, returns `C5CoreError::FileExists`.

## c5store Secret Formatting (`secrets_format.rs`)

This module deals with the standard array format for c5store secrets within YAML.
`[<algorithm_string>, <key_name_string>, <base64_ciphertext_string>]`

*   **Types:**
    *   `C5SecretValueParts { algo_str: String, key_name: String, b64_ciphertext: String }`: Struct representing the parts of a c5store secret array.

*   **Functions:**
    *   `format_c5_secret_array(algo: CryptoAlgorithm, public_key_file_name: &str, b64_ciphertext: String) -> Result<yaml_rust2::Yaml, C5CoreError>`
        *   Formats the components into a `yaml_rust2::Yaml::Array`.
        *   Derives `key_name` from `public_key_file_name` (e.g., "my.key.pub.pem" -> "my.key").
    *   `parse_c5_secret_array(secret_yaml_value: &yaml_rust2::Yaml) -> Result<C5SecretValueParts, C5CoreError>`
        *   Parses a `yaml_rust2::Yaml::Array` into `C5SecretValueParts`.

## YAML Utilities (`yaml_utils.rs`)

Uses `yaml_rust2` for YAML processing.

*   **Types:**
    *   `yaml_rust2::Yaml`: The primary enum representing YAML values.

*   **Functions:**
    *   `load_yaml_from_string(yaml_str: &str) -> Result<Yaml, C5CoreError>`
        *   Loads the first YAML document from a string. Returns an empty `Yaml::Hash` for empty string.
    *   `dump_yaml_to_string(yaml_doc: &Yaml) -> Result<String, C5CoreError>`
        *   Serializes a `Yaml` document to a string.
    *   `get_yaml_value_at_path<'a>(root: &'a Yaml, path_str: &str) -> Option<&'a Yaml>`
        *   Retrieves a reference to a YAML value at a dot-separated `path_str` (e.g., "level1.level2.key").
        *   Returns `Some(root)` if `path_str` is empty.
    *   `set_yaml_value_at_path(root: &mut Yaml, path_str: &str, value_to_set: Yaml) -> Result<(), C5CoreError>`
        *   Sets a YAML value at a dot-separated `path_str`.
        *   Creates intermediate `Yaml::Hash` (map) nodes if they don't exist (or are `Yaml::Null`).
        *   If `path_str` is empty, replaces the `root` with `value_to_set`.
        *   Returns an error if an intermediate path segment is a scalar or array that cannot be converted to a map.