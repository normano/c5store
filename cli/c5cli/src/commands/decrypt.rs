use atty;
use c5_core::{
  base64_string_to_bytes,
  decrypt_data,
  io_utils::write_bytes_to_file,
  load_ecies_private_key,
  parse_c5_secret_array,
  yaml_utils::load_yaml_from_string,
  C5CoreError,
  CryptoAlgorithm as CoreCryptoAlgo,
};
use clap::Args;
use std::fs;
use std::io::{self, Write as IoWrite}; // For writing to stdout
use std::path::PathBuf; // For checking if stdout is a TTY

use crate::{path_parser::{parse_path, PathSegment}, CliCryptoAlgorithm};

#[derive(Args, Debug)]
#[clap(
    after_help = "EXAMPLES:\n\
    # Decrypt a secret and print it to the console\n\
    c5cli decrypt prod.yaml app.api_key my_key.key.pem --to-stdout\n\n\
    # Decrypt a secret from an array and save it to a file, overwriting if it exists\n\
    c5cli decrypt config.yaml 'users[name=\"admin\"].token' admin.key.pem decrypted_token.txt -y"
)]
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

  // --- NEW: ADVANCED PATH TRAVERSAL FOR DECRYPTION ---
  let segments = parse_path(&args.key_path)?;
  let mut current_node = &yaml_doc_root;

  for (i, segment) in segments.iter().enumerate() {
    let current_path_trace = || {
      segments[..=i]
        .iter()
        .map(|s| format!("{:?}", s))
        .collect::<Vec<_>>()
        .join("")
    };
    match segment {
      PathSegment::Key(key) => {
        current_node = match current_node.as_hash() {
          Some(map) => map.get(&yaml_rust2::Yaml::String(key.to_string())).ok_or_else(|| {
            C5CoreError::YamlNavigation(format!(
              "Key '{}' not found (at path trace: {}).",
              key,
              current_path_trace()
            ))
          })?,
          None => {
            return Err(C5CoreError::YamlNavigation(format!(
              "Expected a Map to access key '{}' (at path trace: {}), but found a different type.",
              key,
              current_path_trace()
            )))
          }
        };
      }
      PathSegment::Index(index) => {
        current_node = match current_node.as_vec() {
          Some(arr) => arr.get(*index).ok_or_else(|| {
            C5CoreError::YamlNavigation(format!(
              "Index {} is out of bounds (at path trace: {}).",
              index,
              current_path_trace()
            ))
          })?,
          None => {
            return Err(C5CoreError::YamlNavigation(format!(
              "Expected an Array for index access [{}] (at path trace: {}), but found a different type.",
              index,
              current_path_trace()
            )))
          }
        };
      }
      PathSegment::Query { key, value } => {
        let mut found_node = None;
        if let Some(arr) = current_node.as_vec() {
          for item in arr.iter() {
            if let Some(map) = item.as_hash() {
              if let Some(val_node) = map.get(&yaml_rust2::Yaml::String(key.to_string())) {
                if val_node.as_str() == Some(value) {
                  if found_node.is_some() {
                    return Err(C5CoreError::YamlNavigation(format!(
                      "Query '[{}={}]' matched multiple objects. Path must be unique for decryption.",
                      key, value
                    )));
                  }
                  found_node = Some(item);
                }
              }
            }
          }
        } else {
          return Err(C5CoreError::YamlNavigation(format!(
            "Expected an Array for query '[{}={}]' (at path trace: {}), but found a different type.",
            key,
            value,
            current_path_trace()
          )));
        }

        if let Some(node) = found_node {
          current_node = node;
        } else {
          return Err(C5CoreError::YamlNavigation(format!(
            "Query '[{}={}]' matched no objects. Cannot decrypt.",
            key, value
          )));
        }
      }
    }
  }

  // `current_node` now points to the map that CONTAINS the secret segment.
  let secret_val_yaml = match current_node.as_hash() {
    Some(map) => map
      .get(&yaml_rust2::Yaml::String(args.secret_segment.clone()))
      .ok_or_else(|| {
        C5CoreError::YamlNavigation(format!(
          "Secret segment key '{}' not found under YAML path '{}' in {}.",
          args.secret_segment,
          args.key_path,
          full_config_path.display()
        ))
      })?,
    None => {
      return Err(C5CoreError::YamlNavigation(format!(
        "Expected a map at YAML path '{}' to find secret segment '{}', but found a different type.",
        args.key_path, args.secret_segment
      )));
    }
  };

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
