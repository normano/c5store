use assert_cmd::prelude::*;
use predicates::prelude::*;
use serial_test::serial;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::{fs, path::PathBuf};
use tempfile::{tempdir, NamedTempFile};

/// The value at a c5cli path, read back through the same reader the tool uses.
fn at(text: &str, path: &str) -> Option<c5_core::Value> {
  let format = c5_core::Format::Yaml;
  c5_core::Document::parse(text, format).unwrap().get(&c5_core::parse_path(path).unwrap()).unwrap()
}

/// The key name a secret array records, for asserting which key encrypted it.
fn key_name(value: &c5_core::Value) -> &str {
  value.as_array().expect("a secret is an array")[1].as_str().expect("the key name is a string")
}


fn c5cli_cmd() -> Command {
  Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap()
}

fn setup_test_c5_keys(dir: &Path, prefix: &str) -> Result<(PathBuf, PathBuf), Box<dyn std::error::Error>> {
  let mut cmd = c5cli_cmd();
  cmd.current_dir(dir);
  cmd.arg("gen").arg("kp").arg(prefix).arg("--output-dir").arg("."); // Output to current_dir (which is 'dir')

  cmd.assert().success();

  let pub_key_path = dir.join(format!("{}.c5.pub.pem", prefix));
  let priv_key_path = dir.join(format!("{}.c5.key.pem", prefix));
  assert!(pub_key_path.exists());
  assert!(priv_key_path.exists());
  Ok((pub_key_path, priv_key_path))
}

#[test]
#[serial]
fn test_encrypt_value_dry_run() -> Result<(), Box<dyn std::error::Error>> {
  let test_dir = tempdir()?;
  let config_root = test_dir.path().join("config");
  let pub_key_root = test_dir.path().join("keys");
  fs::create_dir_all(&config_root)?;
  fs::create_dir_all(&pub_key_root)?;

  // Create a dummy public key file (content doesn't strictly matter for dry run path check)
  let (pub_key_path, _) = setup_test_c5_keys(test_dir.path(), "testkey_encrypt_dry_run")?;
  let pub_key_name = pub_key_path.file_name().unwrap().to_str().unwrap();

  let config_file_path = config_root.join("app.yaml");
  fs::write(&config_file_path, "")?;

  let mut cmd = c5cli_cmd();
  cmd
    .arg("encrypt")
    .arg(config_file_path.file_name().unwrap()) // Pass only name
    .arg(pub_key_name) // Pass only name
    .arg("app.secret.password")
    .arg("-v")
    .arg("supersecret123")
    .arg("--config-root-dir")
    .arg(&config_root)
    .arg("--public-key-dir")
    .arg(test_dir.path());

  cmd
    .assert()
    .success()
    .stdout(predicate::str::contains("----- DRY RUN - Encrypt -----"))
    .stdout(predicate::str::contains(
      "The secret key '.c5encval' under YAML path 'app.secret.password' would be updated/created.",
    ))
    .stdout(predicate::str::contains("Full resulting YAML content:"))
    .stdout(predicate::str::contains("app:"))
    .stdout(predicate::str::contains("  secret:"))
    .stdout(predicate::str::contains("    password:"))
    .stdout(predicate::str::contains("      \".c5encval\":"))
    .stdout(predicate::str::contains("- ecies_x25519"))
    .stdout(predicate::str::contains("- testkey_encrypt_dry_run.c5"));

  // Ensure original config file is unchanged
  let content = fs::read_to_string(&config_file_path)?;
  assert_eq!(content, "");
  Ok(())
}

#[test]
#[serial]
fn test_encrypt_value_commit() -> Result<(), Box<dyn std::error::Error>> {
  let test_dir = tempdir()?;
  let config_root = test_dir.path().join("config");
  let pub_key_root = test_dir.path().join("keys"); // Not used if pubkey in test_dir
  fs::create_dir_all(&config_root)?;

  let (pub_key_path, _) = setup_test_c5_keys(test_dir.path(), "testkey_encrypt_commit")?;
  let pub_key_name = pub_key_path.file_name().unwrap().to_str().unwrap();

  let config_file_path = config_root.join("secrets.yaml");
  fs::write(&config_file_path, "existing_key: some_value\n")?;

  let mut cmd = c5cli_cmd();
  cmd
    .arg("encrypt")
    .arg(config_file_path.file_name().unwrap())
    .arg(pub_key_name)
    .arg("database.user.token")
    .arg("-v")
    .arg("mySecureToken!@#")
    .arg("--config-root-dir")
    .arg(&config_root)
    .arg("--public-key-dir")
    .arg(test_dir.path()) // Pubkey is in test_dir
    .arg("--commit");

  cmd
    .assert()
    .success()
    .stdout(predicate::str::contains("Encrypted secret successfully committed."));

  let content = fs::read_to_string(&config_file_path)?;
  assert!(content.contains("existing_key: some_value"));
  assert!(content.contains("database:"));
  assert!(content.contains("  user:"));
  assert!(content.contains("    token:"));
  assert!(content.contains("      \".c5encval\":"));
  assert!(content.contains("- ecies_x25519"));
  assert!(content.contains("- testkey_encrypt_commit.c5")); // Derived key name
  assert!(content.contains("- testkey_encrypt_commit.c5\n        - ")); // Part of base64
  Ok(())
}

#[test]
#[serial]
fn test_encrypt_file_commit_output_file() -> Result<(), Box<dyn std::error::Error>> {
  let test_dir = tempdir()?;
  let config_root = test_dir.path().join("source_config");
  let pub_key_root = test_dir.path().join("source_keys");
  let output_dir = test_dir.path().join("output_config");
  fs::create_dir_all(&config_root)?;
  fs::create_dir_all(&pub_key_root)?;
  fs::create_dir_all(&output_dir)?;

  let (pub_key_path, _) = setup_test_c5_keys(&pub_key_root, "key_for_file_encrypt")?;
  let pub_key_name = pub_key_path.file_name().unwrap().to_str().unwrap();

  let original_config_path = config_root.join("original.yaml");
  fs::write(&original_config_path, "version: 1.0\n")?;

  let file_to_encrypt_path = test_dir.path().join("my_cert.pem");
  fs::write(
    &file_to_encrypt_path,
    "-----BEGIN CERTIFICATE-----\nCERTDATA\n-----END CERTIFICATE-----",
  )?;

  let output_config_path = output_dir.join("encrypted_config.yaml");

  let mut cmd = c5cli_cmd();
  cmd
    .arg("encrypt")
    .arg(original_config_path.file_name().unwrap()) // original.yaml
    .arg(pub_key_name) // key_for_file_encrypt.c5.pub.pem
    .arg("certs.service_x.content")
    .arg("-f")
    .arg(&file_to_encrypt_path)
    .arg("--config-root-dir")
    .arg(&config_root)
    .arg("--public-key-dir")
    .arg(&pub_key_root)
    .arg("--commit")
    .arg("--output-file")
    .arg(&output_config_path);

  cmd.assert().success();

  // Original file should be unchanged
  let original_content = fs::read_to_string(&original_config_path)?;
  assert_eq!(original_content.trim(), "version: 1.0");

  // Output file should exist and have the encrypted data
  assert!(output_config_path.exists());
  let output_content = fs::read_to_string(&output_config_path)?;
  assert!(output_content.contains("version: 1.0")); // Original content merged
  assert!(output_content.contains("certs:"));
  assert!(output_content.contains("  service_x:"));
  assert!(output_content.contains("    content:"));
  assert!(output_content.contains("      \".c5encval\":"));
  assert!(output_content.contains("- key_for_file_encrypt.c5"));
  Ok(())
}

#[test]
#[serial]
fn test_encrypt_reencrypt() -> Result<(), Box<dyn std::error::Error>> {
  let test_dir = tempdir()?;
  let config_root = test_dir.path().join("config");
  let keys_dir = test_dir.path().join("keys");
  fs::create_dir_all(&config_root)?;
  fs::create_dir_all(&keys_dir)?;

  // 1. Create initial keypair (old_key) and encrypt a secret
  let (old_pub_path, old_priv_path) = setup_test_c5_keys(&keys_dir, "old_key")?;
  let old_pub_name = old_pub_path.file_name().unwrap().to_str().unwrap();

  let config_file_path = config_root.join("app_for_reencrypt.yaml");

  let mut cmd_initial_encrypt = c5cli_cmd();
  cmd_initial_encrypt
    .arg("encrypt")
    .arg(config_file_path.file_name().unwrap())
    .arg(old_pub_name)
    .arg("my_app.api_key")
    .arg("-v")
    .arg("initial_secret_value")
    .arg("--config-root-dir")
    .arg(&config_root)
    .arg("--public-key-dir")
    .arg(&keys_dir)
    .arg("--commit");
  cmd_initial_encrypt.assert().success();

  // 2. Create a new keypair (new_key)
  let (new_pub_path, _) = setup_test_c5_keys(&keys_dir, "new_key")?;
  let new_pub_name = new_pub_path.file_name().unwrap().to_str().unwrap();

  // 3. Re-encrypt using new_key's public key and old_key's private key
  let mut cmd_reencrypt = c5cli_cmd();
  cmd_reencrypt
    .arg("encrypt")
    .arg(config_file_path.file_name().unwrap())
    .arg(new_pub_name) // New public key
    .arg("my_app.api_key")
    .arg("--reencrypt")
    .arg("--old-private-key-file")
    .arg(&old_priv_path) // Old private key
    .arg("--config-root-dir")
    .arg(&config_root)
    .arg("--public-key-dir")
    .arg(&keys_dir)
    .arg("--commit");

  cmd_reencrypt
    .assert()
    .success()
    .stdout(predicate::str::contains("Successfully decrypted existing value"))
    .stdout(predicate::str::contains("Encrypted secret successfully committed"));

  // 4. Verify the config file: key_name should now be "new_key.c5"
  let content = fs::read_to_string(&config_file_path)?;
  assert!(content.contains("- new_key.c5")); // Check for new key name
  assert!(!content.contains("- old_key.c5")); // Old key name should be gone

  Ok(())
}

#[test]
#[serial]
fn test_encrypt_into_array_by_index() -> Result<(), Box<dyn std::error::Error>> {
  let test_dir = tempdir()?;
  let config_root = test_dir.path();
  let keys_dir = test_dir.path().join("keys");
  fs::create_dir(&keys_dir)?;

  let (pub_key_path, _) = setup_test_c5_keys(&keys_dir, "idx_key")?;
  let pub_key_name = pub_key_path.file_name().unwrap().to_str().unwrap();

  let config_file_path = config_root.join("array_config.yaml");
  fs::write(&config_file_path, "users:\n  - name: alice\n  - name: bob\n")?;

  let mut cmd = c5cli_cmd();
  cmd
    .arg("encrypt")
    .arg(config_file_path.file_name().unwrap())
    .arg(pub_key_name)
    .arg("users[0].token") // Use array index syntax
    .arg("-v")
    .arg("alices_secret_token")
    .arg("--config-root-dir")
    .arg(&config_root)
    .arg("--public-key-dir")
    .arg(&keys_dir)
    .arg("--commit");

  cmd.assert().success();

  let content = fs::read_to_string(&config_file_path)?;

  let alice = at(&content, "users[0].token").expect("alice has a token");
  assert_eq!(key_name(alice.get(".c5encval").expect("alice's token is a secret")), "idx_key.c5");

  assert_eq!(at(&content, "users[1].token"), None, "bob was not the one encrypted");

  Ok(())
}

#[test]
#[serial]
fn test_encrypt_into_array_by_query() -> Result<(), Box<dyn std::error::Error>> {
  let test_dir = tempdir()?;
  let config_root = test_dir.path();
  let keys_dir = test_dir.path().join("keys");
  fs::create_dir(&keys_dir)?;

  let (pub_key_path, _) = setup_test_c5_keys(&keys_dir, "query_key")?;
  let pub_key_name = pub_key_path.file_name().unwrap().to_str().unwrap();

  let config_file_path = config_root.join("query_config.yaml");
  fs::write(
    &config_file_path,
    "users:\n  - name: alice\n    role: user\n  - name: bob\n    role: admin\n",
  )?;

  let mut cmd = c5cli_cmd();
  cmd
    .arg("encrypt")
    .arg(config_file_path.file_name().unwrap())
    .arg(pub_key_name)
    .arg("users[name=\"bob\"].token") // Use query syntax
    .arg("-v")
    .arg("bobs_admin_token")
    .arg("--config-root-dir")
    .arg(&config_root)
    .arg("--public-key-dir")
    .arg(&keys_dir)
    .arg("--commit");

  cmd.assert().success();

  let content = fs::read_to_string(&config_file_path)?;

  let bob = at(&content, "users[1].token").expect("bob has a token");
  assert_eq!(key_name(bob.get(".c5encval").expect("bob's token is a secret")), "query_key.c5");

  assert_eq!(at(&content, "users[0].token"), None, "alice was not the one encrypted");

  Ok(())
}

#[test]
#[serial]
fn test_encrypt_fails_on_non_unique_query() -> Result<(), Box<dyn std::error::Error>> {
  let test_dir = tempdir()?;
  let config_root = test_dir.path();
  let keys_dir = test_dir.path().join("keys");
  fs::create_dir(&keys_dir)?;

  let (pub_key_path, _) = setup_test_c5_keys(&keys_dir, "multi_query_key")?;
  let pub_key_name = pub_key_path.file_name().unwrap().to_str().unwrap();

  let config_file_path = config_root.join("multi_query_config.yaml");
  fs::write(
    &config_file_path,
    "users:\n  - role: admin\n  - role: user\n  - role: admin\n",
  )?;

  let mut cmd = c5cli_cmd();
  cmd
    .arg("encrypt")
    .arg(config_file_path.file_name().unwrap())
    .arg(pub_key_name)
    .arg("users[role=\"admin\"].key") // This query matches 2 objects
    .arg("-v")
    .arg("some_key_value")
    .arg("--config-root-dir")
    .arg(&config_root)
    .arg("--public-key-dir")
    .arg(&keys_dir);

  cmd.assert().failure().stderr(predicate::str::contains(
    "Query '[role=admin]' matched multiple objects",
  ));

  Ok(())
}

#[test]
#[serial]
fn test_encrypt_fails_on_zero_match_query() -> Result<(), Box<dyn std::error::Error>> {
  let test_dir = tempdir()?;
  let config_root = test_dir.path();
  let keys_dir = test_dir.path().join("keys");
  fs::create_dir(&keys_dir)?;

  let (pub_key_path, _) = setup_test_c5_keys(&keys_dir, "zero_query_key")?;
  let pub_key_name = pub_key_path.file_name().unwrap().to_str().unwrap();

  let config_file_path = config_root.join("zero_query_config.yaml");
  fs::write(&config_file_path, "users:\n  - name: alice\n")?;

  let mut cmd = c5cli_cmd();
  cmd
    .arg("encrypt")
    .arg(config_file_path.file_name().unwrap())
    .arg(pub_key_name)
    .arg("users[name=\"bob\"].token") // This query matches 0 objects
    .arg("-v")
    .arg("no_one_gets_this_token")
    .arg("--config-root-dir")
    .arg(&config_root)
    .arg("--public-key-dir")
    .arg(&keys_dir);

  cmd
    .assert()
    .failure()
    .stderr(predicate::str::contains("Query '[name=bob]' matched no objects"));

  Ok(())
}


#[test]
#[serial]
fn test_encrypt_replaces_existing_scalar_value() -> Result<(), Box<dyn std::error::Error>> {
  let test_dir = tempdir()?;
  let config_root = test_dir.path();
  let keys_dir = test_dir.path().join("keys");
  fs::create_dir(&keys_dir)?;

  // 1. Setup keys
  let (pub_key_path, priv_key_path) = setup_test_c5_keys(&keys_dir, "replace_key")?;
  let pub_key_name = pub_key_path.file_name().unwrap().to_str().unwrap();
  let priv_key_name = priv_key_path.file_name().unwrap().to_str().unwrap();

  // 2. Setup initial config with a plaintext scalar value
  let config_file_path = config_root.join("replace_config.yaml");
  let initial_yaml_content = r#"
users:
  - name: testuser
    credentials:
      - type: password
        value: "old_plaintext_password"
"#;
  fs::write(&config_file_path, initial_yaml_content)?;

  let new_secret_value = "SupaDupaS3cr3t!";

  // 3. Run encrypt command to replace the 'value' key
  let mut cmd_encrypt = c5cli_cmd();
  cmd_encrypt
    .arg("encrypt")
    .arg(config_file_path.file_name().unwrap())
    .arg(pub_key_name)
    .arg("users[0].credentials[0].value") // Target the existing scalar key
    .arg("-v")
    .arg(new_secret_value)
    .arg("--config-root-dir")
    .arg(&config_root)
    .arg("--public-key-dir")
    .arg(&keys_dir)
    .arg("--commit");

  cmd_encrypt.assert().success();

  // 4. Assert the structure has been correctly modified
  let final_content = fs::read_to_string(&config_file_path)?;

  let value_node = at(&final_content, "users[0].credentials[0].value").expect("the credential still has a value");
  let secret = value_node.get(".c5encval").expect("the value was replaced with a secret");
  assert_eq!(key_name(secret), "replace_key.c5");

  // 5. (Bonus) Decrypt to verify the *content* is correct
  let output_file = test_dir.path().join("decrypted.txt");
  let mut cmd_decrypt = c5cli_cmd();
  cmd_decrypt
    .arg("decrypt")
    .arg(config_file_path.file_name().unwrap())
    .arg("users[0].credentials[0].value")
    .arg(priv_key_name)
    .arg(&output_file)
    .arg("--config-root-dir")
    .arg(&config_root)
    .arg("--private-key-dir")
    .arg(&keys_dir);

  cmd_decrypt.assert().success();

  let decrypted_content = fs::read_to_string(&output_file)?;
  assert_eq!(decrypted_content, new_secret_value);

  Ok(())
}
