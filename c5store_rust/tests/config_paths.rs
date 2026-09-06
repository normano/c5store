use std::path::PathBuf;

use c5store::default_config_paths;

#[test]
fn builds_the_five_conventional_paths() {
  let paths = default_config_paths("config", "production", "prod", "us-east");

  assert_eq!(
    paths,
    vec![
      PathBuf::from("config/common.yaml"),
      PathBuf::from("config/production.yaml"),
      PathBuf::from("config/prod.yaml"),
      PathBuf::from("config/us-east.yaml"),
      PathBuf::from("config/prod-us-east.yaml"),
    ]
  );
}

#[test]
fn does_not_normalize_the_config_dir() {
  let paths = default_config_paths("config/", "production", "prod", "us-east");

  assert_eq!(paths[0], PathBuf::from("config//common.yaml"));
}
