#![cfg(feature = "secrets_systemd")]

use std::env;
use std::fs;

use c5store::secrets::systemd::{KeyFormat, SystemdCredential};
use c5store::secrets::{Base64SecretDecryptor, SecretKeyStore};
use c5store::value::C5DataValue;
use c5store::{C5Store, C5StoreOptions, SecretOptions, create_c5store};
use serial_test::serial;
use tempfile::TempDir;

const SECRET_CONFIG: &str = "a_secret:\n  .c5encval: [\"base64\", \"my_key\", \"YWJjZA==\"]\n";

fn options_with_credential(credential_name: &str, format: KeyFormat) -> C5StoreOptions {
  let mut options = C5StoreOptions::default();
  options.secret_opts = SecretOptions {
    secret_key_store_configure_fn: Some(Box::new(|store: &mut SecretKeyStore| {
      store.set_decryptor("base64", Box::from(Base64SecretDecryptor {}));
    })),
    load_credentials_from_systemd: vec![SystemdCredential {
      credential_name: credential_name.to_string(),
      ref_key_name: "my_key".to_string(),
      format,
    }],
    ..Default::default()
  };
  options
}

#[test]
#[serial]
fn a_raw_credential_is_loaded_as_a_key() {
  let dir = TempDir::new().unwrap();
  let config = dir.path().join("config.yaml");
  fs::write(&config, SECRET_CONFIG).unwrap();

  let creds = dir.path().join("creds");
  fs::create_dir(&creds).unwrap();
  fs::write(creds.join("myapp.private.key"), b"dummy").unwrap();

  unsafe {
    env::set_var("CREDENTIALS_DIRECTORY", &creds);
  }
  let store = create_c5store(
    vec![config],
    Some(options_with_credential("myapp.private.key", KeyFormat::Raw)),
  );
  unsafe {
    env::remove_var("CREDENTIALS_DIRECTORY");
  }

  let (store, _mgr) = store.unwrap();
  assert_eq!(
    store.get("a_secret"),
    Some(C5DataValue::Bytes("abcd".as_bytes().to_vec()))
  );
}

#[test]
#[serial]
fn an_unset_credentials_directory_is_a_warning_not_a_failure() {
  let dir = TempDir::new().unwrap();
  let config = dir.path().join("config.yaml");
  fs::write(&config, SECRET_CONFIG).unwrap();

  unsafe {
    env::remove_var("CREDENTIALS_DIRECTORY");
  }
  let result = create_c5store(
    vec![config],
    Some(options_with_credential("myapp.private.key", KeyFormat::Raw)),
  );

  assert!(result.is_ok());
}

#[test]
#[serial]
fn a_named_credential_that_is_missing_fails_the_load() {
  let dir = TempDir::new().unwrap();
  let config = dir.path().join("config.yaml");
  fs::write(&config, SECRET_CONFIG).unwrap();

  let creds = dir.path().join("creds");
  fs::create_dir(&creds).unwrap();

  unsafe {
    env::set_var("CREDENTIALS_DIRECTORY", &creds);
  }
  let result = create_c5store(
    vec![config],
    Some(options_with_credential("absent.key", KeyFormat::Raw)),
  );
  unsafe {
    env::remove_var("CREDENTIALS_DIRECTORY");
  }

  assert!(result.is_err());
}

#[test]
#[serial]
fn a_credential_that_is_not_valid_pem_fails_the_load() {
  let dir = TempDir::new().unwrap();
  let config = dir.path().join("config.yaml");
  fs::write(&config, SECRET_CONFIG).unwrap();

  let creds = dir.path().join("creds");
  fs::create_dir(&creds).unwrap();
  fs::write(creds.join("myapp.private.key"), b"not a pem file").unwrap();

  unsafe {
    env::set_var("CREDENTIALS_DIRECTORY", &creds);
  }
  let result = create_c5store(
    vec![config],
    Some(options_with_credential("myapp.private.key", KeyFormat::PemX25519)),
  );
  unsafe {
    env::remove_var("CREDENTIALS_DIRECTORY");
  }

  assert!(result.is_err());
}
