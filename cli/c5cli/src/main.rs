use c5_core::{CryptoAlgorithm as CoreCryptoAlgo, SshKeyAlgorithm as CoreSshAlgo};
use clap::{Parser, Subcommand, ValueEnum};
use std::process::ExitCode;

mod commands; // Declare the commands module

// --- CLI Specific Enums (deriving ValueEnum) ---
// These are now defined at the crate root level of c5cli (via main.rs)
#[derive(ValueEnum, Clone, Debug, Copy)]
pub enum CliCryptoAlgorithm {
  // Add pub to make it accessible to other modules
  #[clap(name = "ecies_x25519")]
  EciesX25519,
}

impl From<CliCryptoAlgorithm> for CoreCryptoAlgo {
  fn from(cli_algo: CliCryptoAlgorithm) -> Self {
    match cli_algo {
      CliCryptoAlgorithm::EciesX25519 => CoreCryptoAlgo::EciesX25519,
    }
  }
}

#[derive(ValueEnum, Clone, Debug, Copy)]
pub enum CliSshKeyAlgorithm {
  // Add pub
  #[clap(name = "ed25519")]
  Ed25519,
}

impl From<CliSshKeyAlgorithm> for CoreSshAlgo {
  fn from(cli_algo: CliSshKeyAlgorithm) -> Self {
    match cli_algo {
      CliSshKeyAlgorithm::Ed25519 => CoreSshAlgo::Ed25519,
    }
  }
}

#[derive(Parser, Debug)]
#[clap(name = "c5cli", author, version, about = "CLI tool for c5store secret management", long_about = None)]
struct C5Cli {
  #[clap(subcommand)]
  command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
  /// Encrypt a value or file, or re-encrypt an existing c5store secret. Dry-run by default.
  Encrypt(commands::encrypt::EncryptArgs),
  /// Decrypt a c5store secret. Writes to OUTPUT_FILE_PATH by default.
  Decrypt(commands::decrypt::DecryptArgs),
  /// Generate cryptographic keys.
  #[clap(name = "gen", alias = "generate")]
  Generate(commands::generate::GenArgs),
}

// Using a custom error type for CLI operations can be helpful
#[derive(Debug)]
enum CliError {
  Core(c5_core::C5CoreError),
  // Add other CLI specific errors, e.g., argument validation
  Io(std::io::Error),
  Config(String),
}

impl From<c5_core::C5CoreError> for CliError {
  fn from(core_err: c5_core::C5CoreError) -> Self {
    CliError::Core(core_err)
  }
}

impl std::fmt::Display for CliError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      // When displaying a Core error, just delegate to its Display impl for the base message
      CliError::Core(core_err) => write!(f, "{}", core_err),
      CliError::Io(io_err) => write!(f, "IO Error: {}", io_err),
      CliError::Config(msg) => write!(f, "Configuration Error: {}", msg),
    }
  }
}

impl std::error::Error for CliError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      CliError::Core(e) => Some(e),
      CliError::Io(e) => Some(e),
      CliError::Config(_) => None,
    }
  }
}

fn main() -> ExitCode {
  let cli = C5Cli::parse();
  let result = run_command(cli);

  match result {
    Ok(_) => ExitCode::SUCCESS,
    Err(e) => {
      // Print the primary error message
      eprint!("Error: {}", e); // Using eprint! to control newline

      // Add specific hints for certain types of core errors
      if let CliError::Core(core_err) = &e {
        // Match on reference
        if let c5_core::C5CoreError::FileExists(path) = core_err {
          eprint!(" (Hint: Use -y/--force to overwrite existing file {:?})", path);
        }
        // Add more specific hints for other C5CoreError variants if desired
      }
      eprintln!(); // Ensure a final newline after all parts of the message
      ExitCode::FAILURE
    }
  }
}

fn run_command(cli: C5Cli) -> Result<(), CliError> {
  // Return your CliError
  match cli.command {
    Command::Encrypt(args) => {
      if !args.reencrypt && args.value_to_encrypt.is_none() && args.file_to_encrypt.is_none() {
        return Err(CliError::Config(
          "For new encryption, must provide input via -v/--value OR -f/--file.".into(),
        ));
      }
      commands::encrypt::handle_encrypt(args)?;
    }
    Command::Decrypt(args) => {
      // Validation
      if !args.to_stdout && args.output_file_path.is_none() {
        return Err(CliError::Config(
          "For decrypt, must specify an output file with positional OUTPUT_FILE_PATH or use --to-stdout.".into(),
        ));
      }
      if args.to_stdout && args.output_file_path.is_some() {
        return Err(CliError::Config(
          "Cannot use --to-stdout and specify an output file path simultaneously for decrypt.".into(),
        ));
      }
      commands::decrypt::handle_decrypt(args)?;
    }
    Command::Generate(gen_args) => match gen_args.command {
      commands::generate::GenCommand::Keypair(args) => {
        commands::generate::handle_generate_keypair(args)?;
      }
      commands::generate::GenCommand::Ssh(args) => {
        commands::generate::handle_generate_ssh_keys(args)?;
      }
    },
  }
  Ok(())
}
