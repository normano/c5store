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

fn store(content: &str) -> (impl C5Store, TempDir) {
  let dir = TempDir::new().unwrap();
  let path = write(&dir, "config.yaml", content);
  let (store, _mgr) = create_c5store(vec![path], None).unwrap();
  (store, dir)
}

const CONFIG: &str = "\
fsr:
  cache:
    capacity: 5000
  server:
    listen: \"127.0.0.1:11110\"
http:
  server:
    worker_threads: 4
";

#[test]
fn a_branch_lists_only_its_own_keys_and_lists_them_relative() {
  let (root, _dir) = store(CONFIG);
  let branch = root.branch("fsr");

  let mut keys = branch.key_paths_with_prefix(None);
  keys.sort();

  assert_eq!(keys, vec!["cache.capacity".to_owned(), "server.listen".to_owned()]);
}

#[test]
fn a_prefix_under_a_branch_is_relative_to_the_branch() {
  let (root, _dir) = store(CONFIG);
  let branch = root.branch("fsr");

  assert_eq!(branch.key_paths_with_prefix(Some("cache")), vec!["cache.capacity".to_owned()]);
}

#[test]
fn a_nested_branch_lists_relative_to_itself() {
  let (root, _dir) = store(CONFIG);
  let branch = root.branch("fsr").branch("server");

  assert_eq!(branch.key_paths_with_prefix(None), vec!["listen".to_owned()]);
  assert_eq!(branch.current_key_path(), "fsr.server");
}

#[test]
fn a_branch_over_a_key_that_does_not_exist_lists_nothing() {
  let (root, _dir) = store(CONFIG);

  assert!(root.branch("absent").key_paths_with_prefix(None).is_empty());
}

#[test]
fn the_root_still_lists_every_key_absolute() {
  let (root, _dir) = store(CONFIG);

  let mut keys = root.key_paths_with_prefix(None);
  keys.sort();

  assert_eq!(
    keys,
    vec![
      "fsr.cache.capacity".to_owned(),
      "fsr.server.listen".to_owned(),
      "http.server.worker_threads".to_owned(),
    ]
  );
}

#[test]
fn reads_through_a_branch_are_unaffected() {
  let (root, _dir) = store(CONFIG);
  let branch = root.branch("fsr");

  assert_eq!(branch.get("cache.capacity"), Some(C5DataValue::UInteger(5000)));
  assert!(branch.path_exists("server"));
  assert!(!branch.path_exists("http"));
}
