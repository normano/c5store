# `c5_core` API Reference

The `c5_core` crate provides foundational utilities for cryptographic operations, key management, configuration documents and file I/O, primarily designed to support the `c5cli` tool and `c5store` secret management.

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
    *   `YamlDeserialize(String)`: Error parsing a configuration document, whichever format it is written in.
    *   `YamlSerialize(String)`: Error rendering a value into a document.
    *   `YamlNavigation(String)`: Error navigating or manipulating a document's structure.
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

This module deals with the standard array format for a c5store secret, whichever format the document holding it is written in.
`[<algorithm_string>, <key_name_string>, <base64_ciphertext_string>]`

*   **Types:**
    *   `C5SecretValueParts { algo_str: String, key_name: String, b64_ciphertext: String }`: Struct representing the parts of a c5store secret array.

*   **Functions:**
    *   `format_c5_secret_array(algo: CryptoAlgorithm, public_key_file_name: &str, b64_ciphertext: String) -> Result<Value, C5CoreError>`
        *   Formats the components into a `Value::Array`.
        *   Derives `key_name` from `public_key_file_name` (e.g., "my.key.pub.pem" -> "my.key").
    *   `parse_c5_secret_array(secret_value: &Value) -> Result<C5SecretValueParts, C5CoreError>`
        *   Parses a `Value::Array` into `C5SecretValueParts`.

## Values (`value.rs`)

The value a configuration document holds, independent of the format it is written in.

*   **Types:**
    *   `Value`: `Null`, `Bool(bool)`, `Int(i64)`, `Float(f64)`, `String(String)`, `Array(Vec<Value>)`, `Map(Vec<(String, Value)>)`. The map is ordered, since a document keeps the order it was written in.

*   **Methods:**
    *   `as_str(&self) -> Option<&str>`, `as_array(&self) -> Option<&[Value]>`, `as_map(&self) -> Option<&[(String, Value)]>`
    *   `get(&self, key: &str) -> Option<&Value>`: The entry of a map, or `None` for anything else.
    *   `kind(&self) -> &'static str`: What the value is, for an error that has to say what it found.

## Paths (`path.rs`)

*   **Types:**
    *   `PathSegment<'a>`: `Key(&'a str)`, `Index(usize)`, `Query { key: &'a str, value: &'a str }`

*   **Functions:**
    *   `parse_path(path_str: &str) -> Result<Vec<PathSegment>, C5CoreError>`
        *   Parses `auth.bootstrap`, `users[0].name` and `credentials[name="default"].value` into segments. An empty path is an empty `Vec`.

## Documents (`document.rs`)

A configuration document, read and **edited in place**. Writing replaces the bytes of one value and touches nothing else, so comments, blank lines, key order and the file's own indentation all survive an edit.

*   **Types:**
    *   `Format`: `Yaml`, `Toml`, `Json`.
        *   `Format::of(path: &Path) -> Format`: chosen by extension; anything else is `Yaml`.
        *   `name(&self) -> &'static str`: `"YAML"`, `"TOML"` or `"JSON"`.
    *   `Document`: a document's text and its format.

*   **Methods:**
    *   `Document::load(path: &Path) -> Result<Document, C5CoreError>`
        *   Reads and checks the file. A file that does not exist is an empty document of its extension's format.
    *   `Document::parse(text: impl Into<String>, format: Format) -> Result<Document, C5CoreError>`
    *   `Document::empty(format: Format) -> Document`
    *   `format(&self) -> Format`, `text(&self) -> &str`
    *   `get(&self, segments: &[PathSegment]) -> Result<Option<Value>, C5CoreError>`
        *   The value a path names, or `None` when it names nothing.
    *   `depth_of(&self, segments: &[PathSegment]) -> usize`
        *   How many leading segments the document holds, for an error that has to say where a path stopped being true.
    *   `set(&mut self, segments: &[PathSegment], value: &Value) -> Result<(), C5CoreError>`
        *   Puts `value` at the path, creating the maps along the way that do not exist yet.

### What each format keeps

*   **TOML** is edited through `toml_edit`, so comments, spacing, alignment, key order and quoting are all preserved.
*   **JSON** is edited by replacing the bytes of one value, so the document's own layout, including tab indentation, is preserved.
*   **YAML** is navigated with `yaml-rust2` and edited by replacing the lines an entry occupies. Its emitter is never used, so nothing is reflowed.

### What a write will refuse

*   A YAML value written in flow style, `key: {a: 1}`, has no lines of its own to replace. It is refused by name rather than reflowed; rewrite it as a block first.
*   A query matching more than one object, since a write has to know which object it is changing.
*   An index or a query naming something the document does not already hold, since neither can be created.

### Indentation

A line already in the document is never re-indented. A line being created takes the file's own indent: the character and width are read from the first place the document nests, and a document with nothing to learn from gets two spaces. Note that YAML forbids a tab as indentation, so a tab-indented YAML file is refused by the parser.
