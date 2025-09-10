#![cfg(feature = "secrets")]

use crate::{SecretOptions, error::ConfigError, secrets::SecretKeyStore};

use serde::{Deserialize, Serialize};
#[cfg(feature = "secrets_systemd")]
use std::{env, fs, path::PathBuf};

#[cfg(feature = "secrets_systemd")]
use curve25519_parser::parse_openssl_25519_privkey;

/// Defines the expected format of a key provided by a Systemd credential.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum KeyFormat {
  /// The credential is raw binary data and should be used as-is. This is the default.
  Raw,
  /// The credential is a PEM-encoded X25519 private key that requires parsing.
  PemX25519,
}

// ADD THIS impl block for the default value
impl Default for KeyFormat {
  fn default() -> Self {
    KeyFormat::Raw
  }
}

/// Holds the configuration for loading a single credential managed by systemd.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct SystemdCredential {
  /// The name of the credential file as configured in `LoadCredential=`.
  /// Example: "c5store.private.key"
  pub credential_name: String,

  /// The logical name this key will be known by within c5store.
  /// This is the name you will reference as the 'key_name' in your YAML files.
  /// Example: "my_master_key"
  pub ref_key_name: String,

  /// The format of the key material in the credential file. Defaults to `Raw`.
  #[serde(default)]
  pub format: KeyFormat,
}

#[cfg(feature = "secrets_systemd")]
pub(crate) fn load_systemd_credentials(
  options: &SecretOptions,
  secret_key_store: &mut SecretKeyStore,
) -> Result<(), ConfigError> {
  if options.load_credentials_from_systemd.is_empty() {
    return Ok(());
  }

  // Get the credentials directory from the environment. This is the official method.
  match env::var("CREDENTIALS_DIRECTORY") {
    Ok(cred_dir) => {
      let base_cred_path = PathBuf::from(cred_dir);
      for cred_config in &options.load_credentials_from_systemd {
        let credential_path = base_cred_path.join(&cred_config.credential_name);

        // Read the key file.
        match fs::read(&credential_path) {
          Ok(mut key_bytes) => { // Make key_bytes mutable
            
            // Process the key bytes based on the configured format
            match &cred_config.format {
              KeyFormat::Raw => {}
              KeyFormat::PemX25519 => {
                // Parse the PEM content to get the raw 32-byte key.
                match parse_openssl_25519_privkey(&key_bytes) {
                  Ok(parsed_key) => {
                    // Replace the PEM bytes with the raw parsed key bytes.
                    key_bytes = parsed_key.to_bytes().to_vec();
                  }
                  Err(e) => {
                    // If parsing fails, it's a fatal startup error.
                    return Err(ConfigError::Message(format!(
                      "Failed to parse PEM credential '{}' from systemd path {:?}: {}",
                      cred_config.credential_name, credential_path, e
                    )));
                  }
                }
              }
            }

            println!(
              "[Secrets] Loaded systemd credential '{}' as key '{}' (format: {:?})",
              cred_config.credential_name, cred_config.ref_key_name, cred_config.format
            );
            secret_key_store.set_key(&cred_config.ref_key_name, key_bytes);
          }
          Err(e) => {
            // If the file can't be read (e.g., not found, permissions error), it's a fatal startup error.
            return Err(ConfigError::IoError {
              path: credential_path,
              source: e,
            });
          }
        }
      }
    }
    Err(_) => {
      log::warn!(
        "Configuration requests systemd credentials, but CREDENTIALS_DIRECTORY is not set. Ensure the service unit uses the LoadCredential= directive. Skipping."
      );
    }
  }

  Ok(())
}

// This function does nothing but ensures the code in `lib.rs` always compiles.
#[cfg(not(feature = "secrets_systemd"))]
pub(crate) fn load_systemd_credentials(
  _options: &SecretOptions,
  _secret_key_store: &mut SecretKeyStore,
) -> Result<(), ConfigError> {
  // On non-Linux or when the feature is disabled, this is a no-op.
  // It silently does nothing, which is the desired behavior.
  Ok(())
}
