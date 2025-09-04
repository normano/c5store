use c5_core::{generate_c5_keypair as core_gen_c5_kp, generate_ssh_keypair as core_gen_ssh_kp, io_utils, C5CoreError};
use clap::{Args, Subcommand};
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::path::PathBuf;

use crate::{CliCryptoAlgorithm, CliSshKeyAlgorithm};

#[derive(Args, Debug)]
pub struct GenArgs {
  #[clap(subcommand)]
  pub command: GenCommand,
}

#[derive(Subcommand, Debug)]
pub enum GenCommand {
  /// Generate a key pair (public/private) for c5store encryption.
  #[clap(name = "kp", alias = "keypair")]
  Keypair(GenerateKeypairArgs),

  /// Generate an Ed25519 key pair for SSH.
  #[clap(name = "ssh", alias = "ssh-keys")]
  Ssh(GenerateSshKeysArgs),
}

#[derive(Args, Debug)]
pub struct GenerateKeypairArgs {
  #[arg(value_name = "OUTPUT_NAME_PREFIX", default_value = "c5key")]
  pub output_name_prefix: String,
  #[arg(value_enum, long, default_value_t = CliCryptoAlgorithm::EciesX25519)]
  pub algo: CliCryptoAlgorithm,
  #[arg(long, short = 'd', value_name = "OUTPUT_DIR_PATH", default_value = ".")]
  pub output_dir: PathBuf,
  #[arg(long, short = 'y')]
  pub force: bool,
}

#[derive(Args, Debug)]
pub struct GenerateSshKeysArgs {
  /// The base name for the SSH key files (e.g., 'id_ed25519_deploy').
  #[arg(value_name = "OUTPUT_NAME_PREFIX", default_value = "id_ed25519")]
  pub output_name_prefix: String,

  /// The SSH key algorithm.
  #[arg(value_enum, long, default_value_t = CliSshKeyAlgorithm::Ed25519)]
  pub algo: CliSshKeyAlgorithm,

  /// The directory where the key files will be saved.
  #[arg(long, short = 'd', value_name = "OUTPUT_DIR_PATH", default_value = ".")]
  pub output_dir: PathBuf,

  /// Overwrite key files if they already exist.
  #[arg(long, short = 'y')]
  pub force: bool,

  /// A comment to append to the public key file (e.g., 'user@host').
  #[arg(long, short = 'C')]
  pub comment: Option<String>,

  /// Print the public key to stdout and do not save any files.
  #[arg(long)]
  pub no_save_private_key: bool,
}

pub fn handle_generate_keypair(args: GenerateKeypairArgs) -> Result<(), C5CoreError> {
  println!(
    "Generating c5store key pair with prefix '{}' using {:?}...",
    args.output_name_prefix, args.algo
  );

  let core_algo = args.algo.into(); // Convert CLI enum to c5_core enum
  let mut rng = StdRng::from_os_rng();

  let key_pair = core_gen_c5_kp(core_algo, &mut rng)?;

  // Ensure output directory exists
  if !args.output_dir.exists() {
    std::fs::create_dir_all(&args.output_dir)?; // Create if not exists, propagate IO error
  }

  // Define output file paths
  // Suggested naming: PREFIX.c5.pub.pem and PREFIX.c5.key.pem
  let pub_key_filename = format!("{}.c5.pub.pem", args.output_name_prefix);
  let priv_key_filename = format!("{}.c5.key.pem", args.output_name_prefix);

  let pub_key_path = args.output_dir.join(pub_key_filename);
  let priv_key_path = args.output_dir.join(priv_key_filename);

  // Write public key
  io_utils::write_string_to_file(&pub_key_path, &key_pair.public.0, args.force)?;
  println!("Public key saved to: {:?}", pub_key_path);

  // Write private key
  io_utils::write_string_to_file(&priv_key_path, &key_pair.private.0, args.force)?;
  println!("Private key saved to: {:?}", priv_key_path);
  // TODO: Set restrictive permissions on the private key file (e.g., 0600 on Unix)
  // This requires platform-specific code or a crate like `fs_set_permissions`.
  // For now, we'll skip this, but it's an important production consideration.
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(metadata) = std::fs::metadata(&priv_key_path) {
      let mut permissions = metadata.permissions();
      permissions.set_mode(0o600); // Read/write for owner only
      if let Err(e) = std::fs::set_permissions(&priv_key_path, permissions) {
        eprintln!(
          "Warning: Could not set restrictive permissions on private key file {:?}: {}",
          priv_key_path, e
        );
      }
    }
  }

  println!("c5store key pair generated successfully.");
  Ok(())
}

pub fn handle_generate_ssh_keys(args: GenerateSshKeysArgs) -> Result<(), C5CoreError> {
  println!(
    "Generating SSH key pair with prefix '{}' using {:?}...",
    args.output_name_prefix, args.algo
  );

  let core_ssh_algo = args.algo.into();
  let ssh_key_pair = core_gen_ssh_kp(core_ssh_algo, args.comment.as_deref())?;

  if args.no_save_private_key {
    println!("SSH Public Key (OpenSSH format):");
    println!("{}", ssh_key_pair.public_key_openssh_format);
  } else {
    // Ensure output directory exists
    if !args.output_dir.exists() {
      std::fs::create_dir_all(&args.output_dir)?;
    }

    // Define output file paths (standard SSH naming)
    let priv_key_path = args.output_dir.join(&args.output_name_prefix);
    let pub_key_path = args.output_dir.join(format!("{}.pub", args.output_name_prefix));

    // Write private key
    io_utils::write_string_to_file(&priv_key_path, &ssh_key_pair.private_key_pem.0, args.force)?;
    println!("SSH Private key saved to: {:?}", priv_key_path);
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      if let Ok(metadata) = std::fs::metadata(&priv_key_path) {
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o600);
        if let Err(e) = std::fs::set_permissions(&priv_key_path, permissions) {
          eprintln!(
            "Warning: Could not set restrictive permissions on SSH private key file {:?}: {}",
            priv_key_path, e
          );
        }
      }
    }

    // Write public key (OpenSSH format)
    io_utils::write_string_to_file(&pub_key_path, &ssh_key_pair.public_key_openssh_format, args.force)?;
    println!("SSH Public key saved to: {:?}", pub_key_path);

    println!("SSH key pair generated successfully.");
    if args.comment.is_none() && args.output_name_prefix == "id_ed25519" {
      // Common default
      println!("Hint: You might want to add a comment with -C (e.g., -C \"user@host\")");
    }
  }
  Ok(())
}
