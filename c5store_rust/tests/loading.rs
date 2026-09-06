use std::fs;
use std::path::PathBuf;

use c5store::value::C5DataValue;
use c5store::{C5Store, create_c5store};
use tempfile::TempDir;

fn write(dir: &TempDir, name: &str, content: &str) -> PathBuf {
  let path = dir.path().join(name);
  fs::write(&path, content).unwrap();
  path
}

#[test]
fn later_file_wins_and_maps_merge_recursively() {
  let dir = TempDir::new().unwrap();
  let first = write(&dir, "first.yaml", "database:\n  host: prod\n  pool: 50\n");
  let second = write(&dir, "second.yaml", "database:\n  host: local\n");

  let (store, _mgr) = create_c5store(vec![first, second], None).unwrap();

  assert_eq!(store.get("database.host"), Some(C5DataValue::String("local".into())));
  assert_eq!(store.get("database.pool"), Some(C5DataValue::UInteger(50)));
}

#[test]
fn arrays_are_replaced_whole_rather_than_appended() {
  let dir = TempDir::new().unwrap();
  let first = write(&dir, "first.yaml", "hosts:\n  - a\n  - b\n");
  let second = write(&dir, "second.yaml", "hosts:\n  - c\n");

  let (store, _mgr) = create_c5store(vec![first, second], None).unwrap();

  assert_eq!(
    store.get("hosts"),
    Some(C5DataValue::Array(vec![C5DataValue::String("c".into())]))
  );
}

#[test]
fn a_directory_contributes_its_files_in_lexical_order() {
  let dir = TempDir::new().unwrap();
  write(&dir, "file2.yaml", "winner: two\n");
  write(&dir, "file10.yaml", "winner: ten\n");

  let (store, _mgr) = create_c5store(vec![dir.path().to_path_buf()], None).unwrap();

  // Plain lexical sort, so file10 is read before file2 and file2 wins.
  assert_eq!(store.get("winner"), Some(C5DataValue::String("two".into())));
}

#[test]
fn a_directory_skips_unsupported_extensions_and_does_not_recurse() {
  let dir = TempDir::new().unwrap();
  write(&dir, "good.yaml", "kept: yes\n");
  write(&dir, "notes.txt", "dropped: yes\n");
  write(&dir, "noext", "dropped: yes\n");
  fs::create_dir(dir.path().join("nested")).unwrap();
  fs::write(dir.path().join("nested").join("deep.yaml"), "dropped: yes\n").unwrap();

  let (store, _mgr) = create_c5store(vec![dir.path().to_path_buf()], None).unwrap();

  assert_eq!(store.get("kept"), Some(C5DataValue::String("yes".into())));
  assert_eq!(store.get("dropped"), None);
}

#[test]
fn an_empty_directory_and_a_missing_path_are_both_accepted() {
  let dir = TempDir::new().unwrap();
  let empty = dir.path().join("empty");
  fs::create_dir(&empty).unwrap();
  let missing = dir.path().join("nope.yaml");
  let real = write(&dir, "real.yaml", "loaded: yes\n");

  let (store, _mgr) = create_c5store(vec![empty, missing, real], None).unwrap();

  assert_eq!(store.get("loaded"), Some(C5DataValue::String("yes".into())));
}

#[test]
fn yml_extension_is_read_like_yaml() {
  let dir = TempDir::new().unwrap();
  let path = write(&dir, "config.yml", "short: extension\n");

  let (store, _mgr) = create_c5store(vec![path], None).unwrap();

  assert_eq!(store.get("short"), Some(C5DataValue::String("extension".into())));
}

#[cfg(feature = "toml")]
#[test]
fn toml_files_are_parsed_and_merge_with_yaml() {
  let dir = TempDir::new().unwrap();
  let yaml = write(&dir, "a.yaml", "service:\n  name: app\n  port: 8080\n");
  let toml = write(&dir, "b.toml", "[service]\nport = 9090\n");

  let (store, _mgr) = create_c5store(vec![yaml, toml], None).unwrap();

  assert_eq!(store.get("service.name"), Some(C5DataValue::String("app".into())));
  // TOML integers arrive as i64, so they land in Integer where YAML would give UInteger.
  assert_eq!(store.get("service.port"), Some(C5DataValue::Integer(9090)));
}

#[cfg(feature = "toml")]
#[test]
fn a_malformed_toml_file_fails_the_load() {
  let dir = TempDir::new().unwrap();
  let path = write(&dir, "bad.toml", "this is not = = toml\n");

  assert!(create_c5store(vec![path], None).is_err());
}
