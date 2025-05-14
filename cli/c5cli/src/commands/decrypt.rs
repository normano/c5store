// c5cli/src/commands/decrypt.rs
use atty;
use c5_core::{
  base64_string_to_bytes,
  decrypt_data,
  io_utils::write_bytes_to_file, // write_string_to_file might be needed if converting to string first
  load_ecies_private_key,
  parse_c5_secret_array,
  yaml_utils::{get_yaml_value_at_path, load_yaml_from_string},
  C5CoreError,
  CryptoAlgorithm as CoreCryptoAlgo,
};
use clap::Args;
use std::fs;
use std::io::{self, Write as IoWrite}; // For writing to stdout
use std::path::PathBuf; // For checking if stdout is a TTY

// Assuming CliCryptoAlgorithm is defined elsewhere (e.g., main.rs)
use crate::CliCryptoAlgorithm;

// DecryptArgs struct as defined before...
#[derive(Args, Debug)]
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

  // --- 2. Load and Parse YAML ---
  let yaml_str = match fs::read_to_string(&full_config_path) {
    Ok(s) => s,
    Err(e) => {
      return Err(C5CoreError::IoWithPath {
        path: full_config_path.clone(),
        source: e,
      })
    }
  };
  let yaml_doc_root = load_yaml_from_string(&yaml_str)?;

  // --- CORRECTED YAML NAVIGATION FOR DECRYPT ---
  let parent_map_node: &yaml_rust2::Yaml;
  if args.key_path.is_empty() {
    // If key_path is empty, the secret segment is at the root of the YAML document
    parent_map_node = &yaml_doc_root;
  } else {
    // Navigate to the parent map specified by args.key_path
    parent_map_node = get_yaml_value_at_path(&yaml_doc_root, &args.key_path).ok_or_else(|| {
      C5CoreError::YamlNavigation(format!(
        "Key path '{}' not found in config file '{}'.",
        args.key_path,
        full_config_path.display()
      ))
    })?;
  }

  // Now, get the secret segment from the parent_map_node
  let secret_val_yaml = match parent_map_node.as_hash() {
    Some(map) => {
      // The key for the secret segment itself (e.g., ".c5encval")
      map
        .get(&yaml_rust2::Yaml::String(args.secret_segment.clone())) // Use args.secret_segment directly as key
        .ok_or_else(|| {
          C5CoreError::YamlNavigation(format!(
            "Secret segment key '{}' not found under YAML path '{}' in {}.",
            args.secret_segment,
            args.key_path,
            full_config_path.display()
          ))
        })?
    }
    None => {
      // parent_map_node was not a Hash (map)
      return Err(C5CoreError::YamlNavigation(format!(
        "Expected a map at YAML path '{}' to find secret segment '{}', but found a different type.",
        args.key_path, args.secret_segment
      )));
    }
  };
  // --- END OF CORRECTED YAML NAVIGATION ---

  let secret_parts = parse_c5_secret_array(secret_val_yaml)?;
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
        )))
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

    // Attempt to interpret as UTF-8 based on the --output-encoding flag.
    // For simplicity, this example directly checks for utf-8.
    // A more robust solution for various text encodings would use a crate like `encoding_rs`.
    let output_encoding_lower = args.output_encoding.to_lowercase();
    if output_encoding_lower == "utf-8" || output_encoding_lower == "utf8" {
      match String::from_utf8(decrypted_bytes.clone()) {
        Ok(s) => {
          print!("{}", s); // Print string without an extra newline from print!
                           // print! itself doesn't add a newline.
        }
        Err(_) => {
          // If UTF-8 decoding fails, it's likely binary or wrong encoding.
          // Print a more specific warning and then output raw bytes.
          eprintln!("[Warning] Decrypted data is not valid UTF-8. Outputting raw bytes.");
          io::stdout().write_all(&decrypted_bytes)?; // Write raw bytes
        }
      }
    } else {
      // If encoding is not UTF-8, treat as binary for stdout for now.
      // Proper handling of other text encodings would require specific decoding.
      eprintln!(
        "[Info] Output encoding is '{}'. Outputting raw bytes to stdout.",
        args.output_encoding
      );
      io::stdout().write_all(&decrypted_bytes)?; // Write raw bytes
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
    // write_bytes_to_file handles the force_overwrite logic and writes raw bytes.
    // The --output-encoding flag is primarily for how to *interpret* the bytes
    // if they were text, not how to write them if they are already bytes.
    write_bytes_to_file(output_path, &decrypted_bytes, args.force)?;
    println!("Decrypted content written to '{}'.", output_path.display());
  }

  Ok(())
}
