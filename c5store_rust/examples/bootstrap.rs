// examples/bootstrap_example.rs

use c5store::bootstrapper; // Assuming bootstrapper.rs is in the same crate's src/
use bootstrapper::{BootstrapError, BootstrapItem, ConfigBootstrapper, GitHost};

use std::env;
use std::fs;
use std::path::PathBuf;

// Helper to determine the target config directory for the example.
fn get_example_target_dir() -> Result<PathBuf, std::io::Error> {
  let current_dir = env::current_dir()?;
  let example_dir = current_dir.join("example_bootstrap_output");
  // Clean up previous run for fresh test
  if example_dir.exists() {
    fs::remove_dir_all(&example_dir)?;
  }
  fs::create_dir_all(&example_dir)?;
  Ok(example_dir)
}

#[tokio::main]
async fn main() {
  println!("--- Running Bootstrap Example ---");

  let example_target_dir = match get_example_target_dir() {
    Ok(dir) => dir,
    Err(e) => {
      eprintln!("Failed to set up example target directory: {}", e);
      return;
    }
  };
  println!("Output will be in: {:?}", example_target_dir);

  // --- Define the default Git repository for configs ---
  let default_config_repo_web_url = "https://github.com/normano/c5store";

  let default_git_ref = "main"; // jq uses master

  // --- Create a local file for testing ConfigSource::Local ---
  let local_source_base = env::current_dir().expect("Failed to get current dir for local source base");
  let local_file_name = "my_local_boot_file.txt";
  let local_file_path_for_source = local_source_base.join(local_file_name);
  if fs::write(
    &local_file_path_for_source,
    "This is a local test file for bootstrapping.",
  )
  .is_err()
  {
    eprintln!("Could not create local test file at {:?}", local_file_path_for_source);
  }

  // --- Create and configure the bootstrapper ---
  let bootstrapper = ConfigBootstrapper::new(
    Some(local_source_base.clone()), // Base for ConfigSource::Local
    Some(default_config_repo_web_url.to_string()),
  )
  .add_item(BootstrapItem::new_git(
    None, // Use default_config_repo_web_url from bootstrapper
    GitHost::GitHub,
    default_git_ref.to_string(),
    PathBuf::from("README.md"),
    example_target_dir.join("readme.md"),
  ))
  .add_item(BootstrapItem::new_git(
    None, // Use default
    GitHost::GitHub,
    default_git_ref.to_string(),
    PathBuf::from("c5store_rust/CHANGELOG.md"),
    example_target_dir.join("changelog.md"),
  ))
  .add_item(BootstrapItem::new_http(
    "https://www.rust-lang.org/static/images/rust-logo-blk.svg".to_string(),
    example_target_dir.join("rust_logo.svg"),
  ))
  .add_item(BootstrapItem::new_local(
    PathBuf::from(local_file_name), // Relative to local_source_base
    example_target_dir.join("copied_local_file.txt"),
  ))
  // Example of an item that might fail (e.g., non-existent file in repo)
  .add_item(BootstrapItem::new_git(
    None,
    GitHost::GitHub,
    default_git_ref.to_string(),
    PathBuf::from("NON_EXISTENT_FILE.txt"),
    example_target_dir.join("should_not_be_created.txt"),
  ));

  // --- Run the bootstrapper ---
  match bootstrapper.run().await {
    Ok(()) => {
      println!("\n--- Bootstrap Succeeded (or files already existed/skippable errors occurred) ---");
    }
    Err(e) => {
      eprintln!("\n--- Bootstrap Failed Critically ---");
      match e {
        BootstrapError::LocalSourceNotFound(path) => {
          eprintln!("Critical Error: Local source file not found: {:?}", path);
        }
        // Handle other specific BootstrapError variants if you want custom messages
        _ => eprintln!("Critical Error: {:?}", e),
      }
      // In a real app, you might exit here
    }
  }

  println!("\n--- Checking created files: ---");
  for entry in fs::read_dir(&example_target_dir).unwrap() {
    let entry = entry.unwrap();
    println!("Found: {:?}", entry.file_name());
  }

  // Clean up the local test file
  if fs::remove_file(&local_file_path_for_source).is_err() {
    // ignore
  };
}
