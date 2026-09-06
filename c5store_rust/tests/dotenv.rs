#![cfg(feature = "dotenv")]

use std::env;
use std::fs;

use c5store::value::C5DataValue;
use c5store::{C5Store, C5StoreOptions, create_c5store};
use serial_test::serial;
use tempfile::TempDir;

fn fixture(dotenv_body: &str) -> (TempDir, C5StoreOptions) {
  let dir = TempDir::new().unwrap();
  fs::write(dir.path().join("config.yaml"), "database:\n  host: from-file\n").unwrap();
  fs::write(dir.path().join(".env"), dotenv_body).unwrap();

  let mut options = C5StoreOptions::default();
  options.dotenv_path = Some(dir.path().join(".env"));

  (dir, options)
}

#[test]
#[serial]
fn a_dotenv_variable_overrides_a_file_value() {
  let (dir, options) = fixture("C5_DATABASE__HOST=from-dotenv\n");

  let (store, _mgr) = create_c5store(vec![dir.path().join("config.yaml")], Some(options)).unwrap();
  unsafe {
    env::remove_var("C5_DATABASE__HOST");
  }

  assert_eq!(
    store.get("database.host"),
    Some(C5DataValue::String("from-dotenv".into()))
  );
}

#[test]
#[serial]
fn a_process_variable_wins_over_the_dotenv_file() {
  let (dir, options) = fixture("C5_DATABASE__HOST=from-dotenv\n");

  unsafe {
    env::set_var("C5_DATABASE__HOST", "from-process");
  }
  let (store, _mgr) = create_c5store(vec![dir.path().join("config.yaml")], Some(options)).unwrap();
  unsafe {
    env::remove_var("C5_DATABASE__HOST");
  }

  assert_eq!(
    store.get("database.host"),
    Some(C5DataValue::String("from-process".into()))
  );
}

#[test]
#[serial]
fn a_missing_dotenv_file_is_not_an_error() {
  let dir = TempDir::new().unwrap();
  fs::write(dir.path().join("config.yaml"), "database:\n  host: from-file\n").unwrap();

  let mut options = C5StoreOptions::default();
  options.dotenv_path = Some(dir.path().join("absent.env"));

  let (store, _mgr) = create_c5store(vec![dir.path().join("config.yaml")], Some(options)).unwrap();

  assert_eq!(store.get("database.host"), Some(C5DataValue::String("from-file".into())));
}

#[test]
#[serial]
fn dotenv_values_are_typed_like_any_other_environment_variable() {
  let (dir, options) = fixture("C5_DATABASE__POOL=200\nC5_DATABASE__DEBUG=true\n");

  let (store, _mgr) = create_c5store(vec![dir.path().join("config.yaml")], Some(options)).unwrap();
  unsafe {
    env::remove_var("C5_DATABASE__POOL");
    env::remove_var("C5_DATABASE__DEBUG");
  }

  assert_eq!(store.get_into::<u64>("database.pool").unwrap(), 200);
  assert_eq!(store.get_into::<bool>("database.debug").unwrap(), true);
}
