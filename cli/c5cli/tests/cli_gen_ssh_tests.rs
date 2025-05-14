use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

fn c5cli_cmd() -> Command {
  Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap()
}

#[test]
fn test_gen_ssh_default_no_args() -> Result<(), Box<dyn std::error::Error>> {
  let temp_dir = tempdir()?;
  let output_dir = temp_dir.path();

  let mut cmd = c5cli_cmd();
  cmd.current_dir(output_dir); // Output defaults to current dir
  cmd.arg("gen").arg("ssh");

  cmd
    .assert()
    .success()
    .stdout(predicate::str::contains(
      "Generating SSH key pair with prefix 'id_ed25519'",
    ))
    .stdout(predicate::str::contains("SSH Private key saved to:").and(predicate::str::contains("id_ed25519")))
    .stdout(predicate::str::contains("SSH Public key saved to:").and(predicate::str::contains("id_ed25519.pub")))
    .stdout(predicate::str::contains("SSH key pair generated successfully."));

  assert!(output_dir.join("id_ed25519").exists());
  assert!(output_dir.join("id_ed25519.pub").exists());

  // Verify .pub file content
  let pub_content = fs::read_to_string(output_dir.join("id_ed25519.pub"))?;
  assert!(pub_content.starts_with("ssh-ed25519 AAAA"));
  Ok(())
}

#[test]
fn test_gen_ssh_with_prefix_output_dir_and_comment() -> Result<(), Box<dyn std::error::Error>> {
  let base_temp_dir = tempdir()?;
  let specific_output_dir = base_temp_dir.path().join("my_ssh_keys");

  let mut cmd = c5cli_cmd();
  cmd
    .arg("gen")
    .arg("ssh")
    .arg("custom_id_rsa") // Positional prefix
    .arg("--algo") // Assuming algo is a flag for SSH keys too, might be ed25519 only for now
    .arg("ed25519") // Be explicit for test
    .arg("--output-dir")
    .arg(specific_output_dir.as_os_str())
    .arg("-C")
    .arg("user@host_for_custom_id");

  cmd.assert().success();

  assert!(specific_output_dir.join("custom_id_rsa").exists());
  assert!(specific_output_dir.join("custom_id_rsa.pub").exists());

  let pub_content = fs::read_to_string(specific_output_dir.join("custom_id_rsa.pub"))?;
  assert!(pub_content.starts_with("ssh-ed25519 AAAA"));
  assert!(pub_content.ends_with("user@host_for_custom_id"));
  Ok(())
}

#[test]
fn test_gen_ssh_no_save_private_key() -> Result<(), Box<dyn std::error::Error>> {
  let temp_dir = tempdir()?; // Still need a dir for cmd to run in, even if not saving
  let output_dir = temp_dir.path();

  let mut cmd = c5cli_cmd();
  cmd.current_dir(output_dir);
  cmd
    .arg("gen")
    .arg("ssh")
    .arg("temp_ssh_key_no_save")
    .arg("--no-save-private-key")
    .arg("-C")
    .arg("key_for_stdout");

  cmd
    .assert()
    .success()
    .stdout(predicate::str::contains("SSH Public Key (OpenSSH format):"))
    .stdout(predicate::str::contains("ssh-ed25519 AAAA").and(predicate::str::contains("key_for_stdout")));

  // Assert files are NOT created
  assert!(!output_dir.join("temp_ssh_key_no_save").exists());
  assert!(!output_dir.join("temp_ssh_key_no_save.pub").exists());
  Ok(())
}

#[test]
fn test_gen_ssh_force_overwrite() -> Result<(), Box<dyn std::error::Error>> {
  let temp_dir = tempdir()?;
  let output_dir = temp_dir.path();

  fs::write(output_dir.join("id_overwrite"), "old private key")?;
  fs::write(output_dir.join("id_overwrite.pub"), "old public key")?;

  let mut cmd = c5cli_cmd();
  cmd.current_dir(output_dir);
  cmd.arg("gen").arg("ssh").arg("id_overwrite").arg("-y"); // --force

  cmd.assert().success();

  let priv_content = fs::read_to_string(output_dir.join("id_overwrite"))?;
  assert_ne!(priv_content, "old private key"); // Should be new PEM or OpenSSH format

  let pub_content = fs::read_to_string(output_dir.join("id_overwrite.pub"))?;
  assert_ne!(pub_content, "old public key");
  assert!(pub_content.starts_with("ssh-ed25519 AAAA"));
  Ok(())
}

#[test]
fn test_gen_ssh_no_overwrite_error() -> Result<(), Box<dyn std::error::Error>> {
  let temp_dir = tempdir()?;
  let output_dir = temp_dir.path();

  fs::write(output_dir.join("id_no_overwrite"), "existing private key")?;

  let mut cmd = c5cli_cmd();
  cmd.current_dir(output_dir);
  cmd.arg("gen").arg("ssh").arg("id_no_overwrite"); // No --force

  cmd
    .assert()
    .failure()
    .stderr(predicate::str::contains("File already exists").and(predicate::str::contains("id_no_overwrite")));
  Ok(())
}
