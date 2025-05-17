# c5cli - Command Line Interface for c5store Secret Management

`c5cli` is a command-line tool for managing secrets within YAML configuration files, designed to work with the [c5store](https://github.com/normano/c5store) configuration library's secret format. It allows you to encrypt, decrypt, and generate cryptographic keys for securing sensitive data.

This tool leverages the `c5_core` library for its underlying cryptographic and YAML manipulation capabilities.

## Features

*   **Encrypt:**
    *   Encrypt string values or the content of files.
    *   Store encrypted secrets in a structured YAML format (`.c5encval` arrays).
    *   Re-encrypt existing secrets with new keys.
    *   Dry-run mode to preview changes.
*   **Decrypt:**
    *   Decrypt secrets stored in the c5store format from YAML files.
    *   Output decrypted content to a file or stdout.
*   **Generate Keys:**
    *   Generate ECIES X25519 key pairs (PEM format) for use with c5store secrets.
    *   Generate Ed25519 SSH key pairs (private key in PKCS#8 PEM, public key in OpenSSH format).

## Installation

### Prerequisites

*   Rust programming language and Cargo (Rust's package manager). Install from [rustup.rs](https://rustup.rs/).

### Building from Source

1.  Clone this repository (or the parent repository containing `c5cli`):
    ```bash
    git clone <repository_url>
    cd <repository_name>/cli
    ```
2.  Build the `c5cli` tool:
    ```bash
    cargo build --release -p c5cli
    ```
3.  The executable will be located at `target/release/c5cli`. You can copy this to a directory in your `PATH`, e.g., `/usr/local/bin`.

    ```bash
    # Example:
    # cp target/release/c5cli /usr/local/bin/
    ```

## Usage

`c5cli` provides several subcommands: `encrypt`, `decrypt`, and `gen`.

```
c5cli <COMMAND> --help
```

### General Options

*   `--config-root-dir <PATH>`: Root directory for configuration files (default: `config`).
*   `--public-key-dir <PATH>`: Directory for public keys (for `encrypt`, default: `config/public_keys`).
*   `--private-key-dir <PATH>`: Directory for private keys (for `decrypt`, default: `config/private_keys`).
*   `--secret-segment <SEGMENT>`: The YAML key used to store the secret array (default: `.c5encval`).

### 1. Generating Key Pairs (`gen`)

#### Generate c5store ECIES Key Pair

Used for encrypting and decrypting c5store secrets.

```bash
c5cli gen kp [OPTIONS] [OUTPUT_NAME_PREFIX]
```

*   `[OUTPUT_NAME_PREFIX]`: Prefix for the output key files (default: `c5key`).
    *   Public key: `<prefix>.c5.pub.pem`
    *   Private key: `<prefix>.c5.key.pem`
*   `--algo <ALGORITHM>`: Cryptographic algorithm (default: `ecies_x25519`).
*   `-d, --output-dir <PATH>`: Directory to save the keys (default: current directory).
*   `-y, --force`: Overwrite existing key files.

**Example:**

```bash
# Generate keys in the current directory with default prefix "c5key"
c5cli gen kp

# Generate keys for "my_service" in the "keys/" directory
mkdir -p keys
c5cli gen kp my_service --output-dir keys
```

#### Generate SSH Ed25519 Key Pair

```bash
c5cli gen ssh [OPTIONS] [OUTPUT_NAME_PREFIX]
```

*   `[OUTPUT_NAME_PREFIX]`: Prefix for the output key files (default: `id_ed25519`).
    *   Private key: `<prefix>`
    *   Public key: `<prefix>.pub`
*   `--algo <ALGORITHM>`: SSH key algorithm (default: `ed25519`).
*   `-d, --output-dir <PATH>`: Directory to save the keys (default: current directory).
*   `-C, --comment <COMMENT>`: Add a comment to the public key.
*   `--no-save-private-key`: Print public key to stdout and do not save the private key.
*   `-y, --force`: Overwrite existing key files.

**Example:**

```bash
# Generate SSH keys in "./ssh_keys" with a comment
mkdir -p ssh_keys
c5cli gen ssh my_ssh_key --output-dir ssh_keys -C "user@example.com"
```

### 2. Encrypting Secrets (`encrypt`)

Encrypts a value or file content and prepares it for inclusion in a YAML configuration file. By default, it performs a dry run. Use `--commit` to write changes.

```bash
c5cli encrypt [OPTIONS] <CONFIG_FILE_NAME> <PUBLIC_KEY_FILE_NAME> <KEY_PATH>
```

*   `<CONFIG_FILE_NAME>`: Name of the YAML config file (e.g., `app.yaml`). Searched in `--config-root-dir`.
*   `<PUBLIC_KEY_FILE_NAME>`: Name of the public key PEM file (e.g., `service_a.c5.pub.pem`). Searched in `--public-key-dir`.
*   `<KEY_PATH>`: Dot-separated path within the YAML where the secret should be stored (e.g., `database.password`).

**Input Options (choose one):**

*   `-v, --value <PLAINTEXT_VALUE>`: The string value to encrypt.
*   `-f, --file <INPUT_FILE_PATH>`: Path to a file whose content will be encrypted.
    *   `--encoding <ENCODING>`: Encoding of the input file if it's text (default: `utf8`). For binary files, encoding is usually ignored.
*   `--reencrypt`: Re-encrypt an existing secret at `<KEY_PATH>`. Requires `--old-private-key-file`.
    *   `--old-private-key-file <OLD_PRIVATE_KEY_FILE>`: Path to the old private key needed to decrypt the existing secret.

**Output Options:**

*   `--commit`: Actually write the changes to the `<CONFIG_FILE_NAME>` or `--output-file`.
*   `--output-file <OUTPUT_FILE_PATH>`: (Requires `--commit`) Write the modified YAML to a different file instead of in-place.

**Example: Encrypting a password (dry run)**

```bash
# Assuming config/app.yaml and config/public_keys/my_service.c5.pub.pem exist
c5cli encrypt app.yaml my_service.c5.pub.pem api.credentials.token -v "s3cr3tV@lu3"
```

**Example: Encrypting a file and committing to a new output file**

```bash
echo "sensitive file content" > /tmp/secret.txt
c5cli encrypt base_config.yaml service_key.c5.pub.pem certs.private_content \
    -f /tmp/secret.txt \
    --commit \
    --output-file config/production.yaml
```

### 3. Decrypting Secrets (`decrypt`)

Decrypts a secret from a YAML configuration file.

```bash
c5cli decrypt [OPTIONS] <CONFIG_FILE_NAME> <KEY_PATH> <PRIVATE_KEY_FILE_NAME> [OUTPUT_FILE_PATH]
```

*   `<CONFIG_FILE_NAME>`: Name of the YAML config file.
*   `<KEY_PATH>`: Dot-separated path within the YAML where the secret is stored.
*   `<PRIVATE_KEY_FILE_NAME>`: Name of the private key PEM file.
*   `[OUTPUT_FILE_PATH]`: Path to save the decrypted content. Required unless `--to-stdout` is used.

**Output Options:**

*   `--to-stdout`: Print decrypted content to standard output instead of a file.
*   `-y, --force`: (Requires `OUTPUT_FILE_PATH`) Overwrite the output file if it exists.
*   `--output-encoding <ENCODING>`: When using `--to-stdout`, attempt to interpret decrypted bytes using this encoding (default: `utf8`). If not valid UTF-8, raw bytes may be hinted.

**Example: Decrypting a token to stdout**

```bash
# Assuming config/app.yaml and config/private_keys/my_service.c5.key.pem exist
c5cli decrypt app.yaml api.credentials.token my_service.c5.key.pem --to-stdout
```

**Example: Decrypting a secret to a file**

```bash
c5cli decrypt app.yaml certs.private_content service_key.c5.key.pem /tmp/decrypted_cert.txt -y
```

## Configuration Secret Format

`c5cli` (and `c5store`) expect secrets to be stored in YAML as an array under a specific key (default `.c5encval`):

```yaml
some_service:
  api_key:
    ".c5encval": # This is the secret_segment
      - "ecies_x25519"                     # Algorithm
      - "my_key_name"                      # Key name (derived from public key filename)
      - "Base64EncodedCiphertextGoesHere=" # Ciphertext
```

## Development

(Instructions for developers contributing to `c5cli`)

1.  Ensure Rust is installed.
2.  Navigate to the `cli` directory within the project.
3.  Run tests:
    ```bash
    cargo test -p c5cli
    cargo test -p c5_core
    ```
4.  Build debug version:
    ```bash
    cargo build -p c5cli
    ```
    Executable: `target/debug/c5cli`

## License

This project is licensed under the Mozilla Public License Version 2.0 (MPL-2.0).