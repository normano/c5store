use atty;
use c5_core::{
  C5CoreError, CryptoAlgorithm as CoreCryptoAlgo, Document, base64_string_to_bytes, decrypt_data,
  io_utils::write_bytes_to_file, load_ecies_private_key, parse_c5_secret_array, parse_path,
};
use clap::Args;
use std::fs;
use std::io::{self, Write as IoWrite};
use std::path::PathBuf;

use crate::CliCryptoAlgorithm;

#[derive(Args, Debug)]
#[clap(after_help = "EXAMPLES:\n\
    # Decrypt a secret and print it to the console\n\
    c5cli decrypt prod.yaml app.api_key my_key.key.pem --to-stdout\n\n\
    # Decrypt a secret from an array and save it to a file, overwriting if it exists\n\
    c5cli decrypt config.yaml 'users[name=\"admin\"].token' admin.key.pem decrypted_token.txt -y")]
pub struct DecryptArgs {
  #[arg(value_name = "CONFIG_FILE_NAME")]
  pub config_file_name: String,
  #[arg(value_name = "KEY_PATH")]
  pub key_path: String,
  #[arg(value_name = "PRIVATE_KEY_FILE_NAME")]
  pub private_key_file_name: String,
  #[arg(value_name = "OUTPUT_FILE_PATH", required_unless_present("to_stdout"))]
  pub output_file_path: Option<PathBuf>,

  #[arg(long, value_name = "PATH", default_value = "config")]
  pub config_root_dir: PathBuf,
  #[arg(long, value_name = "PATH", default_value = "config/private_keys")]
  pub private_key_dir: PathBuf,

  #[arg(long, conflicts_with("output_file_path"))]
  pub to_stdout: bool,
  #[arg(short = 'y', long = "force", requires = "output_file_path")]
  pub force: bool,
  #[arg(long, value_name = "ENCODING", default_value = "utf8")]
  pub output_encoding: String,

  #[arg(value_enum, long)]
  pub algo: Option<CliCryptoAlgorithm>,
  #[arg(long, value_name = "SEGMENT", default_value = ".c5encval")]
  pub secret_segment: String,
}

pub fn handle_decrypt(args: DecryptArgs) -> Result<(), C5CoreError> {
  // Output mode validation is now primarily handled by clap attributes in main.rs
  let full_config_path = args.config_root_dir.join(&args.config_file_name);
  let full_privkey_path = args.private_key_dir.join(&args.private_key_file_name);

  println!(
    "Decrypting secret at key path '{}' from config file '{}'...",
    args.key_path,
    full_config_path.display()
  );
  println!("Using private key from: {}", full_privkey_path.display());

  // --- 1. Load Private Key ---
  let private_key = load_ecies_private_key(&full_privkey_path)?;

  // --- 2. Load the configuration, in whatever format it is written ---
  if !full_config_path.exists() {
    return Err(C5CoreError::IoWithPath {
      path: full_config_path.clone(),
      source: std::io::Error::new(std::io::ErrorKind::NotFound, "configuration file not found"),
    });
  }
  let document = Document::load(&full_config_path)?;
  let segments = parse_path(&args.key_path)?;

  let holder = match document.get(&segments)? {
    Some(holder) => holder,
    None => {
      let stopped = document.depth_of(&segments);
      let missing = segment_name(&segments[stopped]);
      return Err(C5CoreError::YamlNavigation(format!(
        "Key '{missing}' not found: path '{}' names nothing in {}.",
        args.key_path,
        full_config_path.display()
      )));
    }
  };
  let secret_value = holder.get(&args.secret_segment).ok_or_else(|| {
    C5CoreError::YamlNavigation(format!(
      "Secret segment '{}' not found under path '{}' in {}; the path holds {}.",
      args.secret_segment,
      args.key_path,
      full_config_path.display(),
      holder.kind()
    ))
  })?;

  let secret_parts = parse_c5_secret_array(secret_value)?;
  println!(
    "Found secret array: algo='{}', key_name='{}'",
    secret_parts.algo_str, secret_parts.key_name
  );

  // --- 3. Determine Algorithm and Decrypt ---
  let effective_core_algo = match args.algo {
    Some(cli_algo) => {
      let core_algo_from_cli: CoreCryptoAlgo = cli_algo.into();
      let algo_str_from_cli = format!("{:?}", core_algo_from_cli)
        .to_lowercase()
        .replace("corecryptoalgo::", ""); // hacky way to get string
      if algo_str_from_cli != secret_parts.algo_str.to_lowercase() {
        println!(
          "[Warning] CLI specified algorithm ({:?}) mismatches algorithm in secret ('{}'). Using CLI override.",
          core_algo_from_cli, secret_parts.algo_str
        );
      }
      core_algo_from_cli
    }
    None => match secret_parts.algo_str.as_str() {
      "ecies_x25519" => CoreCryptoAlgo::EciesX25519,
      _ => {
        return Err(C5CoreError::UnsupportedAlgorithm(format!(
          "Algorithm '{}' found in secret is not supported for decryption.",
          secret_parts.algo_str
        )));
      }
    },
  };

  let ciphertext_bytes = base64_string_to_bytes(&secret_parts.b64_ciphertext)?;
  let decrypted_bytes = decrypt_data(&ciphertext_bytes, &private_key, effective_core_algo)?;
  println!(
    "Decryption successful. Plaintext length: {} bytes.",
    decrypted_bytes.len()
  );

  // --- 4. Output Decrypted Content ---
  if args.to_stdout {
    eprintln!("[Warning] Outputting decrypted content to stdout. Ensure this is a secure terminal.");

    let output_encoding_lower = args.output_encoding.to_lowercase();
    if output_encoding_lower == "utf-8" || output_encoding_lower == "utf8" {
      match String::from_utf8(decrypted_bytes.clone()) {
        Ok(s) => {
          print!("{}", s);
        }
        Err(_) => {
          eprintln!("[Warning] Decrypted data is not valid UTF-8. Outputting raw bytes.");
          io::stdout().write_all(&decrypted_bytes)?;
        }
      }
    } else {
      eprintln!(
        "[Info] Output encoding is '{}'. Outputting raw bytes to stdout.",
        args.output_encoding
      );
      io::stdout().write_all(&decrypted_bytes)?;
    }

    // Add a newline only if stdout is a TTY, for better shell prompt integration after output.
    if atty::is(atty::Stream::Stdout) {
      println!();
    }
  } else {
    // This 'else' implies args.output_file_path is Some(), due to clap/main validation.
    let output_path = args.output_file_path.as_ref().unwrap();

    if let Some(parent) = output_path.parent() {
      if !parent.exists() {
        fs::create_dir_all(parent)?;
      }
    }
    // The --output-encoding flag is primarily for how to *interpret* the bytes
    // if they were text, not how to write them if they are already bytes.
    write_bytes_to_file(output_path, &decrypted_bytes, args.force)?;
    println!("Decrypted content written to '{}'.", output_path.display());
  }

  Ok(())
}

/// One path segment as it was written, for an error to point at.
fn segment_name(segment: &c5_core::PathSegment) -> String {
  match segment {
    c5_core::PathSegment::Key(key) => (*key).to_owned(),
    c5_core::PathSegment::Index(index) => format!("[{index}]"),
    c5_core::PathSegment::Query { key, value } => format!("[{key}={value}]"),
  }
}
