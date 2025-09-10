# Using `systemd` Credentials for Secure Key Injection with `c5store`

This guide explains how to use standard Linux tools and `systemd` to securely provide a private key to a `c5store`-enabled application. This is the recommended method for production environments as it leverages hardware-backed encryption (if a TPM is available) and ensures the private key is never stored unencrypted on disk.

## Core Concept

The process separates responsibilities:
*   **The Administrator** uses `c5cli` and `systemd-creds` to prepare secrets and keys.
*   **`systemd`** securely stores the private key, decrypts it using the machine's unique key (TPM), and injects the plaintext into the application at runtime.
*   **`c5store`** reads the injected key and uses it to decrypt application-level secrets from the configuration file.

---

## **Step 1: Prepare Keys and Secrets (Offline)**

These actions are performed on a secure machine, which could be an administrator's workstation or a dedicated bastion host.

### 1.1. Generate the ECIES Key Pair

Use `c5cli` to create the private key that will be protected by `systemd` and the public key for encrypting secrets.

```bash
# This command generates two files: my_app.c5.key.pem and my_app.c5.pub.pem
c5cli gen kp my_app
```

*   **`my_app.c5.key.pem`**: This is the private key. You will need its contents for Step 2.
*   **`my_app.c5.pub.pem`**: This is the public key.

### 1.2. Encrypt a Secret

Use `c5cli` and the public key to encrypt a sensitive value. The tool will update your YAML configuration file.

```bash
# This encrypts the string and updates config.yaml under the key 'database.password'
c5cli encrypt config.yaml my_app.c5.pub.pem database.password --value "postgres_password_123" --commit
```

After running this, your `config.yaml` will contain an entry like this:

```yaml
database:
  password.c5encval:
    - "ecies_x25519"
    - "my_app"  # This name is derived from the public key filename.
    - "VGhlRW5jcnlwdGVkU2VjcmV0..." # This is the encrypted secret.
```

Your `config.yaml` is now ready to be deployed. It contains no plaintext secrets.

---

## **Step 2: Provision the Private Key on the Server**

These actions are performed on each server where your application will run.

### 2.1. Securely Transfer the Private Key

Copy the contents of `my_app.c5.key.pem` to the server. For example, use `scp` and then delete the file, or copy-paste it into a terminal session.

### 2.2. Store the Key with `systemd-creds`

On the server, use the `systemd-creds` command to encrypt and store the private key. The name given here (e.g., `myapp.private.key`) is the **credential name**.

```bash
# Paste the contents of my_app.c5.key.pem when prompted, then press Ctrl+D.
# The credential name "myapp.private.key" will be used in the systemd service file.
sudo systemd-creds encrypt - --name=myapp.private.key /etc/credstore.encrypted/myapp.private.key

# Securely remove the original PEM file if it was transferred
shred -u my_app.c5.key.pem
```
This command reads the key from standard input, encrypts it using a key unique to this machine (often TPM-backed), and saves it to the protected `credstore` directory. The plaintext private key no longer needs to be on the server.

---

## **Step 3: Configure the Application Service**

These files are deployed to the server, typically via an RPM package or other deployment script.

### 3.1. Configure the `systemd` Service File

Create or edit your application's `.service` file to tell `systemd` to load the credential at startup.

**File:** `/etc/systemd/system/myapp.service`
```ini
[Unit]
Description=My Application Service

[Service]
# Recommended for security
DynamicUser=yes

# This directive tells systemd to decrypt and provide the key.
# The name MUST match the credential name from step 2.2.
LoadCredentialEncrypted=myapp.private.key:/etc/credstore.encrypted/myapp.private.key

ExecStart=/usr/bin/myapp-server

[Install]
WantedBy=multi-user.target
```
After creating or modifying this file, run `sudo systemctl daemon-reload`.

### 3.2. Configure `c5store` in the Application Code

Modify your application's startup code to opt-in to the `systemd` credential feature and specify the key format.

**File:** (e.g., `src/main.rs` of your application)

```rust
use c5store::{create_c5store, C5StoreOptions};
use c5store::secrets::systemd::{KeyFormat, SystemdCredential};

// In your application's setup function:
let mut options = C5StoreOptions::default();

options.secret_opts.load_credentials_from_systemd = vec![
  SystemdCredential {
    // This MUST match the name in the LoadCredentialEncrypted directive.
    credential_name: "myapp.private.key".to_string(),
    
    // This MUST match the key name in your YAML's .c5encval array.
    ref_key_name: "my_app".to_string(),
    
    // This tells c5store to parse the decrypted PEM content.
    format: KeyFormat::PemX25519,
  }
];

// Proceed to create the c5store with these options.
let (c5store, _mgr) = create_c5store(config_paths, Some(options))?;
```

## Runtime Process Summary

1.  An admin runs `sudo systemctl start myapp.service`.
2.  `systemd` finds `LoadCredentialEncrypted=myapp.private.key:...`.
3.  It decrypts the file at `/etc/credstore.encrypted/myapp.private.key` using the machine's secret key.
4.  It writes the **plaintext PEM content** to `/run/credentials/myapp.service/myapp.private.key`.
5.  It starts your application.
6.  Your app initializes `c5store` with the `SystemdCredential` option.
7.  `c5store` reads the plaintext PEM from `/run/credentials/...`.
8.  Because `format` is `KeyFormat::PemX25519`, it **parses the PEM** to get the raw 32-byte key.
9.  It loads this raw key into its key store under the logical name `"my_app"`.
10. `c5store` parses `config.yaml`, finds the encrypted secret, and uses the loaded key and the ECIES decryptor to get the plaintext value.