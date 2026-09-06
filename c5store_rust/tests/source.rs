mod common;

use std::env;
use std::fs;

use c5store::value::C5DataValue;
use c5store::{C5Store, create_c5store};
use common::SequenceProvider;
use serial_test::serial;
use tempfile::TempDir;

#[test]
#[serial]
fn a_value_from_a_file_reports_that_file() {
  let dir = TempDir::new().unwrap();
  let path = dir.path().join("config.yaml");
  fs::write(&path, "database:\n  host: from-file\n").unwrap();

  let (store, _mgr) = create_c5store(vec![path.clone()], None).unwrap();

  // ConfigSource is not exported, so the variant is only reachable through Display.
  assert_eq!(
    store.get_source("database.host").map(|s| s.to_string()),
    Some(format!("File({:?})", path))
  );
}

#[test]
#[serial]
fn a_value_overridden_by_the_environment_reports_the_variable() {
  let dir = TempDir::new().unwrap();
  let path = dir.path().join("config.yaml");
  fs::write(&path, "database:\n  host: from-file\n").unwrap();

  unsafe {
    env::set_var("C5_DATABASE__HOST", "from-env");
  }
  let (store, _mgr) = create_c5store(vec![path], None).unwrap();
  unsafe {
    env::remove_var("C5_DATABASE__HOST");
  }

  assert_eq!(store.get("database.host"), Some(C5DataValue::String("from-env".into())));
  assert_eq!(
    store.get_source("database.host").map(|s| s.to_string()),
    Some("EnvVar(C5_DATABASE__HOST)".to_string())
  );
}

#[test]
#[serial]
fn a_value_filled_by_a_provider_reports_the_provider_name() {
  let dir = TempDir::new().unwrap();
  let path = dir.path().join("config.yaml");
  fs::write(&path, "market:\n  regions:\n    .provider: seq\n").unwrap();

  let (store, mut mgr) = create_c5store(vec![path], None).unwrap();
  mgr.set_value_provider(
    "seq",
    SequenceProvider::new(vec![C5DataValue::String("filled".into())]),
    0,
  );

  assert_eq!(store.get("market.regions"), Some(C5DataValue::String("filled".into())));
  assert_eq!(
    store.get_source("market.regions").map(|s| s.to_string()),
    Some("Provider(seq)".to_string())
  );
}

#[test]
#[serial]
fn an_absent_key_has_no_source() {
  let dir = TempDir::new().unwrap();
  let path = dir.path().join("config.yaml");
  fs::write(&path, "present: yes\n").unwrap();

  let (store, _mgr) = create_c5store(vec![path], None).unwrap();

  assert!(store.get_source("absent").is_none());
}
