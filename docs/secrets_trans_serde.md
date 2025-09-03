## Feature: Transparent Secret Deserialization

`c5store` provides a powerful and seamless experience for working with encrypted configuration values ("secrets"). The **Transparent Secret Deserialization** feature ensures that your application code can remain completely unaware of whether a configuration value was stored as plaintext or as an encrypted secret.

Your application's configuration structs define the data types you need, and `c5store` handles the complexity of providing them, whether they come from a simple development file or a secure production environment.

### The Core Concept

When you call `c5store.get_into_struct::<MyConfig>("...")`, `c5store` performs the following steps automatically:

1.  **Loads Configuration:** It reads all configuration sources, including files and environment variables.
2.  **Identifies Secrets:** It finds any values stored in the standard secret format (e.g., a map containing a `.c5encval` key).
3.  **Decrypts Secrets:** It uses the configured `SecretKeyStore` to decrypt these secrets. The result of any decryption is always a raw sequence of bytes (`Vec<u8>`).
4.  **Populates Internal Store:** The decrypted bytes are stored internally as a `C5DataValue::Bytes` variant, replacing the original encrypted value.
5.  **Deserializes with Intelligence:** When `get_into_struct` is called, a "smart" deserializer intelligently converts the stored values—whether they were originally plaintext or are now decrypted bytes—into the data types required by your struct's fields.

The result is that your application's `MyConfig` struct is populated correctly, regardless of the underlying storage format of the values.

### Supported Conversions

The smart deserializer supports the following transparent conversions, making it easy to switch between plaintext for development and secrets for production without changing your code.

#### From `C5DataValue::Bytes` (Decrypted Secrets)

When a field in your struct is being populated from a decrypted secret, the following conversions are supported:

| Struct Field Type | How Decrypted Bytes are Interpreted |
| :--- | :--- |
| `String` | Bytes must be a valid UTF-8 sequence. |
| `i8`, `i16`, `i32`, `i64` | Bytes must have the exact size of the integer type (1, 2, 4, or 8 bytes respectively) and are interpreted as a **big-endian** number. |
| `u8`, `u16`, `u32`, `u64` | Bytes must have the exact size of the integer type and are interpreted as a **big-endian** number. |
| `f32`, `f64` | Bytes must be 4 or 8 bytes respectively and are interpreted as a **big-endian** IEEE 754 floating-point number. |
| `Vec<u8>` | The raw decrypted bytes are used directly. |

#### From `C5DataValue::String` (Plaintext Values)

When a field is populated from a plaintext string, the deserializer is lenient and attempts helpful conversions:

| Struct Field Type | How Plaintext String is Interpreted |
| :--- | :--- |
| `String` | The string is used directly. |
| `i8`, ..., `u64`, `f64` | The string is parsed as a number (e.g., `"123"`, `"99.5"`). |
| `bool` | The string is parsed leniently. `"true"`, `"yes"`, `"on"`, and `"1"` are `true`. `"false"`, `"no"`, `"off"`, and `"0"` are `false` (case-insensitive). |
| `Vec<u8>` | The string is converted to its UTF-8 byte representation. |

### Example in Practice

This feature allows for a powerful and secure workflow.

**1. Your Application Code (Never Changes)**

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct AppConfig {
    api_key: String,
    retries: u32,
}
```

**2. Your Development Configuration (`config/local.yaml`)**

```yaml
# Simple, readable plaintext for local development.
api_key: "dev-api-key-12345"
retries: 3
```
Your application runs perfectly with this file.

**3. Your Production Configuration (`config/production.yaml`)**

```yaml
# Secure, encrypted values for production.
api_key:
  .c5encval:
  - "ecies_x25519"
  - "prod_api_key"
  - "A1B2C3D4..." # Encrypted ciphertext

# You can still mix plaintext for non-sensitive values.
retries: 5
```
Your application, **with no code changes**, runs perfectly with this file. `c5store` handles the decryption of `api_key` and provides the resulting string to your `AppConfig` struct, just as it did with the plaintext value in development.