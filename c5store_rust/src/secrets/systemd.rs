#![cfg(feature = "secrets")]

use crate::{error::ConfigError, secrets::SecretKeyStore, SecretOptions};

use serde::{Deserialize, Serialize};
#[cfg(feature = "secrets_systemd")]
use std::{env, fs, path::PathBuf};

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

        // Read the key file. This is a hard requirement.
        match fs::read(&credential_path) {
          Ok(key_bytes) => {
            println!(
              "[Secrets] Loaded systemd credential '{}' as key '{}'",
              cred_config.credential_name, cred_config.ref_key_name
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