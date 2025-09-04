use assert_cmd::prelude::*;
use predicates::prelude::*;
use serial_test::serial;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::{fs, path::PathBuf};
use tempfile::{tempdir, NamedTempFile};

fn c5cli_cmd() -> Command {
  Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap()
}

fn setup_test_c5_keys_for_decrypt(dir: &Path, prefix: &str) -> Result<(PathBuf, PathBuf), Box<dyn std::error::Error>> {
  let mut cmd = c5cli_cmd();
  cmd.current_dir(dir);
  cmd.arg("gen").arg("kp").arg(prefix).arg("--output-dir").arg(".");
  cmd.assert().success();
  Ok((
    dir.join(format!("{}.c5.pub.pem", prefix)),
    dir.join(format!("{}.c5.key.pem", prefix)),
  ))
}

fn setup_encrypted_config(
  config_dir: &Path,
  config_name: &str,
  keys_dir: &Path,
  key_prefix: &str,
  secret_path: &str,
  secret_value: &str,
) -> Result<(PathBuf, PathBuf, PathBuf), Box<dyn std::error::Error>> {
  let (pub_key_path, priv_key_path) = setup_test_c5_keys_for_decrypt(keys_dir, key_prefix)?;
  let pub_key_name = pub_key_path.file_name().unwrap().to_str().unwrap();

  let config_file_path = config_dir.join(config_name);

  let mut cmd_encrypt = c5cli_cmd();
  cmd_encrypt
    .arg("encrypt")
    .arg(config_name)
    .arg(pub_key_name)
    .arg(secret_path)
    .arg("-v")
    .arg(secret_value)
    .arg("--config-root-dir")
    .arg(config_dir)
    .arg("--public-key-dir")
    .arg(keys_dir)
    .arg("--commit");
  cmd_encrypt.assert().success();
  Ok((config_file_path, pub_key_path, priv_key_path))
}

#[test]
#[serial]
fn test_decrypt_to_stdout() -> Result<(), Box<dyn std::error::Error>> {
  let test_dir = tempdir()?;
  let config_root = test_dir.path().join("config");
  let keys_root = test_dir.path().join("keys");
  fs::create_dir_all(&config_root)?;
  fs::create_dir_all(&keys_root)?;

  let secret_value = "decrypt_me_please_123";
  let (config_file_path, _, priv_key_path) = setup_encrypted_config(
    &config_root,
    "app_decrypt.yaml",
    &keys_root,
    "key_for_decrypt",
    "service.token",
    secret_value,
  )?;
  let priv_key_name = priv_key_path.file_name().unwrap().to_str().unwrap();

  let mut cmd = c5cli_cmd();
  cmd
    .arg("decrypt")
    .arg(config_file_path.file_name().unwrap())
    .arg("service.token")
    .arg(priv_key_name) // This is now a positional argument (output file) - need to adjust
    // If output file is positional, we need to provide one or use --to-stdout
    .arg("--config-root-dir")
    .arg(&config_root)
    .arg("--private-key-dir")
    .arg(&keys_root)
    .arg("--to-stdout");

  cmd
    .assert()
    .success()
    .stdout(predicate::str::contains(secret_value))
    .stderr(predicate::str::contains(
      "[Warning] Outputting decrypted content to stdout",
    )); // Check for warning
  Ok(())
}

#[test]
#[serial]
fn test_decrypt_to_file() -> Result<(), Box<dyn std::error::Error>> {
  let test_dir = tempdir()?;
  let config_root = test_dir.path().join("config");
  let keys_root = test_dir.path().join("keys");
  let output_files_dir = test_dir.path().join("decrypted_output");
  fs::create_dir_all(&config_root)?;
  fs::create_dir_all(&keys_root)?;
  fs::create_dir_all(&output_files_dir)?;

  let secret_value = "secret_for_file_output";
  let (config_file_path, _, priv_key_path) = setup_encrypted_config(
    &config_root,
    "app_decrypt_file.yaml",
    &keys_root,
    "key_dec_file",
    "another.secret",
    secret_value,
  )?;
  let priv_key_name = priv_key_path.file_name().unwrap().to_str().unwrap();

  let output_file = output_files_dir.join("decrypted_secret.txt");

  let mut cmd = c5cli_cmd();
  cmd
    .arg("decrypt")
    .arg(config_file_path.file_name().unwrap())
    .arg("another.secret")
    .arg(priv_key_name)
    .arg(&output_file) // Positional output file path
    .arg("--config-root-dir")
    .arg(&config_root)
    .arg("--private-key-dir")
    .arg(&keys_root);

  cmd.assert().success().stdout(
    predicate::str::contains("Decrypted content written to").and(predicate::str::contains("decrypted_secret.txt")),
  );

  assert!(output_file.exists());
  let decrypted_content = fs::read_to_string(&output_file)?;
  assert_eq!(decrypted_content, secret_value);
  Ok(())
}

#[test]
#[serial]
fn test_decrypt_to_file_force_overwrite() -> Result<(), Box<dyn std::error::Error>> {
  let test_dir = tempdir()?; // ... setup dirs as above ...
  let config_root = test_dir.path().join("config");
  let keys_root = test_dir.path().join("keys");
  let output_files_dir = test_dir.path().join("decrypted_output");
  fs::create_dir_all(&config_root)?;
  fs::create_dir_all(&keys_root)?;
  fs::create_dir_all(&output_files_dir)?;

  let (config_file_path, _, priv_key_path) = setup_encrypted_config(
    &config_root,
    "app_dec_force.yaml",
    &keys_root,
    "key_dec_force",
    "data.item",
    "initial_data",
  )?;
  let priv_key_name = priv_key_path.file_name().unwrap().to_str().unwrap();
  let output_file = output_files_dir.join("output.txt");

  fs::write(&output_file, "pre_existing_content")?; // Create existing file

  let mut cmd = c5cli_cmd();
  cmd
    .arg("decrypt")
    .arg(config_file_path.file_name().unwrap())
    .arg("data.item")
    .arg(priv_key_name)
    .arg(&output_file)
    .arg("--config-root-dir")
    .arg(&config_root)
    .arg("--private-key-dir")
    .arg(&keys_root)
    .arg("-y"); // --force

  cmd.assert().success();
  let content = fs::read_to_string(&output_file)?;
  assert_eq!(content, "initial_data");
  Ok(())
}

#[test]
#[serial]
fn test_decrypt_to_file_no_force_error() -> Result<(), Box<dyn std::error::Error>> {
  let test_dir = tempdir()?; // ... setup dirs ...
  let config_root = test_dir.path().join("config");
  let keys_root = test_dir.path().join("keys");
  let output_files_dir = test_dir.path().join("decrypted_output");
  fs::create_dir_all(&config_root)?;
  fs::create_dir_all(&keys_root)?;
  fs::create_dir_all(&output_files_dir)?;

  let (config_file_path, _, priv_key_path) = setup_encrypted_config(
    &config_root,
    "app_dec_noforce.yaml",
    &keys_root,
    "key_dec_noforce",
    "config.value",
    "secret_val",
  )?;
  let priv_key_name = priv_key_path.file_name().unwrap().to_str().unwrap();
  let output_file = output_files_dir.join("exists.txt");

  fs::write(&output_file, "don't overwrite me")?;

  let mut cmd = c5cli_cmd();
  cmd
    .arg("decrypt")
    .arg(config_file_path.file_name().unwrap())
    .arg("config.value")
    .arg(priv_key_name)
    .arg(&output_file)
    .arg("--config-root-dir")
    .arg(&config_root)
    .arg("--private-key-dir")
    .arg(&keys_root);

  let output_file_name_str = output_file.file_name().unwrap().to_str().unwrap();
  cmd.assert().failure().stderr(
    predicate::str::contains("Error: File already exists at path:") // Check for the prefix
      .and(predicate::str::contains(output_file_name_str)) // Check that the filename is in there
      .and(predicate::str::contains("Hint: Use -y/--force to overwrite")), // Check for the hint
  );

  let content = fs::read_to_string(&output_file)?;
  assert_eq!(content, "don't overwrite me"); // Should be unchanged
  Ok(())
}

#[test]
#[serial]
fn test_decrypt_missing_secret_in_config() -> Result<(), Box<dyn std::error::Error>> {
  let test_dir = tempdir()?;
  let config_root = test_dir.path().join("config");
  let keys_root = test_dir.path().join("keys");
  fs::create_dir_all(&config_root)?;
  fs::create_dir_all(&keys_root)?;

  // Create an empty config and a key
  let config_file_path = config_root.join("empty.yaml");
  fs::write(&config_file_path, "")?; // Write an empty string to create an empty file
  let (_, priv_key_path) = setup_test_c5_keys_for_decrypt(&keys_root, "dummy_key")?;
  let priv_key_name = priv_key_path.file_name().unwrap().to_str().unwrap();

  let mut cmd = c5cli_cmd();
  cmd
    .arg("decrypt")
    .arg(config_file_path.file_name().unwrap())
    .arg("non.existent.secret") // This secret path won't be in empty.yaml
    .arg(priv_key_name)
    .arg("--config-root-dir")
    .arg(&config_root)
    .arg("--private-key-dir")
    .arg(&keys_root)
    .arg("--to-stdout"); // to avoid needing an output file path for this error test

  cmd.assert().failure().stderr(
    predicate::str::contains("Error: YAML navigation/manipulation error:")
      .and(predicate::str::contains("Key 'non' not found")),
  );
  Ok(())
}
