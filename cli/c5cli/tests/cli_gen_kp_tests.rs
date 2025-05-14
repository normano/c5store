use assert_cmd::prelude::*; // Add methods on commands
use predicates::prelude::*; // Used for writing assertions
use std::fs;
use std::path::Path;
use std::process::Command; // Used to run the CLI
use tempfile::tempdir; // For creating temporary directories for test outputs

// Helper function to get the path to the compiled c5cli binary
fn c5cli_cmd() -> Command {
  Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap()
  // CARGO_PKG_NAME will be "c5cli" if running tests from within the c5cli crate
}

#[test]
fn test_gen_kp_default_no_args() -> Result<(), Box<dyn std::error::Error>> {
  let temp_dir = tempdir()?; // Create a new temporary directory
  let output_dir = temp_dir.path();

  let mut cmd = c5cli_cmd();
  cmd.current_dir(output_dir); // Run the command from within the temp_dir
  cmd.arg("gen").arg("kp");

  cmd
    .assert()
    .success()
    .stdout(predicate::str::contains(
      "Generating c5store key pair with prefix 'c5key'",
    ))
    .stdout(predicate::str::contains("Public key saved to:").and(predicate::str::contains("c5key.c5.pub.pem")))
    .stdout(predicate::str::contains("Private key saved to:").and(predicate::str::contains("c5key.c5.key.pem")))
    .stdout(predicate::str::contains("c5store key pair generated successfully."));

  // Check if files were created
  assert!(output_dir.join("c5key.c5.pub.pem").exists());
  assert!(output_dir.join("c5key.c5.key.pem").exists());

  // Optional: Clean up (temp_dir will do this automatically on drop)
  // temp_dir.close()?;
  Ok(())
}

#[test]
fn test_gen_kp_with_prefix_and_output_dir() -> Result<(), Box<dyn std::error::Error>> {
  let base_temp_dir = tempdir()?; // A base temp dir
  let specific_output_dir = base_temp_dir.path().join("custom_keys"); // Subdirectory for output

  let mut cmd = c5cli_cmd();
  // We don't need to cmd.current_dir() if --output-dir is absolute or relative to where cmd is run
  cmd
    .arg("gen")
    .arg("kp")
    .arg("my_service_key") // Positional prefix
    .arg("--output-dir")
    .arg(specific_output_dir.as_os_str());

  cmd
    .assert()
    .success()
    .stdout(predicate::str::contains(
      "Generating c5store key pair with prefix 'my_service_key'",
    ))
    .stdout(predicate::str::contains("my_service_key.c5.pub.pem"))
    .stdout(predicate::str::contains("my_service_key.c5.key.pem"));

  assert!(specific_output_dir.join("my_service_key.c5.pub.pem").exists());
  assert!(specific_output_dir.join("my_service_key.c5.key.pem").exists());

  Ok(())
}

#[test]
fn test_gen_kp_with_algo() -> Result<(), Box<dyn std::error::Error>> {
  let temp_dir = tempdir()?;
  let output_dir = temp_dir.path();

  let mut cmd = c5cli_cmd();
  cmd.current_dir(output_dir);
  cmd
    .arg("gen")
    .arg("kp")
    .arg("algo_test_key")
    .arg("--algo")
    .arg("ecies_x25519"); // Currently only one supported by c5_core for this

  cmd.assert().success();
  assert!(output_dir.join("algo_test_key.c5.pub.pem").exists());
  assert!(output_dir.join("algo_test_key.c5.key.pem").exists());
  Ok(())
}

#[test]
fn test_gen_kp_force_overwrite() -> Result<(), Box<dyn std::error::Error>> {
  let temp_dir = tempdir()?;
  let output_dir = temp_dir.path();

  // Create dummy files first
  fs::write(output_dir.join("overwrite_key.c5.pub.pem"), "old pub data")?;
  fs::write(output_dir.join("overwrite_key.c5.key.pem"), "old priv data")?;

  let mut cmd = c5cli_cmd();
  cmd.current_dir(output_dir);
  cmd.arg("gen").arg("kp").arg("overwrite_key").arg("-y"); // --force flag

  cmd.assert().success();

  // Check content is new (hard to check exact PEM content without parsing,
  // but we can check it's not the old dummy data)
  let pub_content = fs::read_to_string(output_dir.join("overwrite_key.c5.pub.pem"))?;
  assert_ne!(pub_content, "old pub data");
  assert!(pub_content.contains("-----BEGIN PUBLIC KEY-----"));

  let priv_content = fs::read_to_string(output_dir.join("overwrite_key.c5.key.pem"))?;
  assert_ne!(priv_content, "old priv data");
  assert!(priv_content.contains("-----BEGIN PRIVATE KEY-----"));

  Ok(())
}

#[test]
fn test_gen_kp_no_overwrite_error() -> Result<(), Box<dyn std::error::Error>> {
  let temp_dir = tempdir()?;
  let output_dir = temp_dir.path();

  // Create a dummy file first
  let pub_key_path = output_dir.join("no_overwrite.c5.pub.pem");
  fs::write(&pub_key_path, "old pub data")?;

  let mut cmd = c5cli_cmd();
  cmd.current_dir(output_dir);
  cmd.arg("gen").arg("kp").arg("no_overwrite"); // No --force flag

  cmd
    .assert()
    .failure() // Expect failure
    .stderr(predicate::str::contains("File already exists").and(predicate::str::contains("no_overwrite.c5.pub.pem")));
  // Or stderr(predicate::str::contains(format!("File already exists at path: {:?}", pub_key_path))); // More precise
  // This depends on the exact error message from C5CoreError::FileExists

  // Ensure the old file was not modified
  let pub_content = fs::read_to_string(&pub_key_path)?;
  assert_eq!(pub_content, "old pub data");

  Ok(())
}

#[test]
fn test_gen_kp_invalid_output_dir() -> Result<(), Box<dyn std::error::Error>> {
  let mut cmd = c5cli_cmd();
  // Using a path that likely cannot be created (e.g., inside /dev/null or a read-only part of fs)
  // This is hard to make reliably cross-platform without more setup.
  // A simpler test is a non-existent deep path that std::fs::create_dir_all would fail on
  // if it lacked permissions, but create_dir_all in the handler *should* create it.
  // Let's test trying to write into a file as if it were a directory.
  let temp_file = tempfile::NamedTempFile::new()?;
  let file_as_dir_path = temp_file.path().to_path_buf(); // Path to an existing file

  cmd
    .arg("gen")
    .arg("kp")
    .arg("key_in_file_dir")
    .arg("--output-dir")
    .arg(file_as_dir_path.join("sub")); // Try to create "sub" inside a file

  cmd.assert().failure().stderr(predicate::str::contains("I/O error")); // Or more specific error from create_dir_all

  Ok(())
}
