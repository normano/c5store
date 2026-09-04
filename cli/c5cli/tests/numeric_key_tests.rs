use assert_cmd::prelude::*;
use predicates::prelude::*;
use serial_test::serial;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

fn c5cli_cmd() -> Command {
  Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap()
}

fn setup_keys(dir: &Path, prefix: &str) -> Result<(PathBuf, PathBuf), Box<dyn std::error::Error>> {
  let mut cmd = c5cli_cmd();
  cmd.current_dir(dir);
  cmd.arg("gen").arg("kp").arg(prefix).arg("--output-dir").arg(".");
  cmd.assert().success();
  Ok((
    dir.join(format!("{}.c5.pub.pem", prefix)),
    dir.join(format!("{}.c5.key.pem", prefix)),
  ))
}

#[test]
#[serial]
fn test_encrypt_numeric_key_start() -> Result<(), Box<dyn std::error::Error>> {
  let test_dir = tempdir()?;
  let config_dir = test_dir.path().join("config");
  let keys_dir = test_dir.path().join("keys");
  fs::create_dir_all(&config_dir)?;
  fs::create_dir_all(&keys_dir)?;

  let (pub_key_path, priv_key_path) = setup_keys(&keys_dir, "num_test")?;
  let pub_key_name = pub_key_path.file_name().unwrap().to_str().unwrap();
  let priv_key_name = priv_key_path.file_name().unwrap().to_str().unwrap();

  let config_path = config_dir.join("numeric.yaml");

  // YAML parsers treat unquoted numbers as Integers.
  let yaml_content = "dataEncryptionKeys:\n  1: old_value\n";
  fs::write(&config_path, yaml_content)?;

  // 1. ENCRYPT
  // This used to fail because "1" wasn't a valid key token
  let mut cmd_enc = c5cli_cmd();
  cmd_enc
    .arg("encrypt")
    .arg(config_path.file_name().unwrap())
    .arg(pub_key_name)
    .arg("dataEncryptionKeys.1") // Numeric key path
    .arg("-v")
    .arg("secret_value")
    .arg("--config-root-dir")
    .arg(&config_dir)
    .arg("--public-key-dir")
    .arg(&keys_dir)
    .arg("--commit");

  cmd_enc.assert().success();

  let content = fs::read_to_string(&config_path)?;
  // We expect the key '1' to now be a map containing the secret
  assert!(content.contains("1:"));
  assert!(content.contains(".c5encval"));

  // 2. DECRYPT
  // This validates the read logic can traverse the integer key
  let output_file = test_dir.path().join("decrypted.txt");
  let mut cmd_dec = c5cli_cmd();
  cmd_dec
    .arg("decrypt")
    .arg(config_path.file_name().unwrap())
    .arg("dataEncryptionKeys.1")
    .arg(priv_key_name)
    .arg(&output_file)
    .arg("--config-root-dir")
    .arg(&config_dir)
    .arg("--private-key-dir")
    .arg(&keys_dir);

  cmd_dec.assert().success();

  let decrypted_content = fs::read_to_string(&output_file)?;
  assert_eq!(decrypted_content, "secret_value");

  Ok(())
}

#[test]
#[serial]
fn test_encrypt_deep_numeric_path() -> Result<(), Box<dyn std::error::Error>> {
  // Test path like: a.1.b.2
  let test_dir = tempdir()?;
  let config_dir = test_dir.path().join("config");
  let keys_dir = test_dir.path().join("keys");
  fs::create_dir_all(&config_dir)?;
  fs::create_dir_all(&keys_dir)?;

  let (pub_key_path, priv_key_path) = setup_keys(&keys_dir, "deep_num")?;
  let pub_key_name = pub_key_path.file_name().unwrap().to_str().unwrap();
  let priv_key_name = priv_key_path.file_name().unwrap().to_str().unwrap();

  let config_path = config_dir.join("deep.yaml");

  // Encrypt into a non-existent file (creating structure)
  let mut cmd_enc = c5cli_cmd();
  cmd_enc
    .arg("encrypt")
    .arg(config_path.file_name().unwrap())
    .arg(pub_key_name)
    .arg("layer.1.deep.2") // Multiple numeric keys
    .arg("-v")
    .arg("deep_secret")
    .arg("--config-root-dir")
    .arg(&config_dir)
    .arg("--public-key-dir")
    .arg(&keys_dir)
    .arg("--commit");

  cmd_enc.assert().success();

  // Decrypt to verify path traversal
  let output_file = test_dir.path().join("out.txt");
  let mut cmd_dec = c5cli_cmd();
  cmd_dec
    .arg("decrypt")
    .arg(config_path.file_name().unwrap())
    .arg("layer.1.deep.2")
    .arg(priv_key_name)
    .arg(&output_file)
    .arg("--config-root-dir")
    .arg(&config_dir)
    .arg("--private-key-dir")
    .arg(&keys_dir);

  cmd_dec.assert().success();

  assert_eq!(fs::read_to_string(&output_file)?, "deep_secret");

  Ok(())
}
