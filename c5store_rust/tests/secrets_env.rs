#![cfg(feature = "secrets")]

use std::env;
use std::fs;

use c5store::secrets::{Base64SecretDecryptor, SecretKeyStore};
use c5store::value::C5DataValue;
use c5store::{C5Store, C5StoreOptions, SecretOptions, create_c5store};
use serial_test::serial;
use tempfile::TempDir;

const SECRET_CONFIG: &str = "a_secret:\n  .c5encval: [\"base64\", \"key1\", \"YWJjZA==\"]\n";

fn options_loading_keys_from_env() -> C5StoreOptions {
  let mut options = C5StoreOptions::default();
  options.secret_opts = SecretOptions {
    secret_key_store_configure_fn: Some(Box::new(|store: &mut SecretKeyStore| {
      store.set_decryptor("base64", Box::from(Base64SecretDecryptor {}));
    })),
    load_secret_keys_from_env: true,
    secret_key_env_prefix: Some("C5_TESTKEY_".to_string()),
    ..Default::default()
  };
  options
}

fn write_config(dir: &TempDir) -> std::path::PathBuf {
  let path = dir.path().join("config.yaml");
  fs::write(&path, SECRET_CONFIG).unwrap();
  path
}

#[test]
#[serial]
fn a_key_from_an_environment_variable_decrypts_a_value() {
  let dir = TempDir::new().unwrap();
  let path = write_config(&dir);

  unsafe {
    env::set_var("C5_TESTKEY_KEY1", "ZHVtbXk=");
  }
  let (store, _mgr) = create_c5store(vec![path], Some(options_loading_keys_from_env())).unwrap();
  unsafe {
    env::remove_var("C5_TESTKEY_KEY1");
  }

  assert_eq!(
    store.get("a_secret"),
    Some(C5DataValue::Bytes("abcd".as_bytes().to_vec()))
  );
}

#[test]
#[serial]
fn the_variable_name_is_lowercased_into_the_key_name() {
  let dir = TempDir::new().unwrap();
  let path = write_config(&dir);

  unsafe {
    env::set_var("C5_TESTKEY_Key1", "ZHVtbXk=");
  }
  let (store, _mgr) = create_c5store(vec![path], Some(options_loading_keys_from_env())).unwrap();
  unsafe {
    env::remove_var("C5_TESTKEY_Key1");
  }

  assert_eq!(
    store.get("a_secret"),
    Some(C5DataValue::Bytes("abcd".as_bytes().to_vec()))
  );
}

#[test]
#[serial]
fn a_value_whose_key_was_never_loaded_is_left_alone() {
  let dir = TempDir::new().unwrap();
  let path = write_config(&dir);

  let (store, _mgr) = create_c5store(vec![path], Some(options_loading_keys_from_env())).unwrap();

  assert_ne!(
    store.get("a_secret"),
    Some(C5DataValue::Bytes("abcd".as_bytes().to_vec()))
  );
}

#[test]
#[serial]
fn a_value_that_is_not_base64_is_skipped_without_failing_the_load() {
  let dir = TempDir::new().unwrap();
  let path = write_config(&dir);

  unsafe {
    env::set_var("C5_TESTKEY_KEY1", "not base64 !!!");
  }
  let result = create_c5store(vec![path], Some(options_loading_keys_from_env()));
  unsafe {
    env::remove_var("C5_TESTKEY_KEY1");
  }

  assert!(result.is_ok());
}
