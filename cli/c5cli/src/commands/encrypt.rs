// c5cli/src/commands/encrypt.rs
use c5_core::{
  base64_string_to_bytes,
  bytes_to_base64_string,
  decrypt_data,
  encrypt_data,
  format_c5_secret_array,
  io_utils::{read_file_to_bytes, /*read_file_to_string,*/ write_string_to_file}, // read_file_to_string not used if value is always bytes for crypto
  load_ecies_private_key,
  load_ecies_public_key,
  parse_c5_secret_array,
  yaml_utils::{dump_yaml_to_string, get_yaml_value_at_path, load_yaml_from_string, set_yaml_value_at_path},
  C5CoreError,
  C5SecretValueParts,
  CryptoAlgorithm as CoreCryptoAlgo,
  EciesPublicKey,
  EciesStaticSecret,
};
use clap::Args;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::fs;
use std::path::{Path, PathBuf};
use yaml_rust2::{Yaml, yaml::Hash as YamlHash}; 

// Assuming CliCryptoAlgorithm is defined in c5cli/src/main.rs or shared types
use crate::CliCryptoAlgorithm;

// EncryptArgs struct as defined before...
#[derive(Args, Debug)]
pub struct EncryptArgs {
  #[arg(value_name = "CONFIG_FILE_NAME")]
  pub config_file_name: String,
  #[arg(value_name = "PUBLIC_KEY_FILE_NAME")]
  pub public_key_file_name: String,
  #[arg(value_name = "KEY_PATH")]
  pub key_path: String,

  #[arg(short = 'v', long = "value", value_name = "PLAINTEXT_VALUE",
        conflicts_with_all = ["file_to_encrypt", "reencrypt"])]
  pub value_to_encrypt: Option<String>,
  #[arg(short = 'f', long = "file", value_name = "INPUT_FILE_PATH",
        conflicts_with_all = ["value_to_encrypt", "reencrypt"])]
  pub file_to_encrypt: Option<PathBuf>,
  #[arg(long, value_name = "ENCODING", default_value = "utf8", requires = "file_to_encrypt")]
  pub encoding: String, // Will be used if file_to_encrypt is text and needs specific interpretation before becoming bytes for encryption

  #[arg(long, conflicts_with_all = ["value_to_encrypt", "file_to_encrypt"], requires = "old_private_key_file")]
  pub reencrypt: bool,
  #[arg(long, value_name = "OLD_PRIVATE_KEY_FILE")]
  pub old_private_key_file: Option<PathBuf>,

  #[arg(long, value_name = "PATH", default_value = "config")]
  pub config_root_dir: PathBuf,
  #[arg(long, value_name = "PATH", default_value = "config/public_keys")]
  pub public_key_dir: PathBuf,

  #[arg(long)]
  pub commit: bool,

  #[arg(value_enum, long, default_value_t = CliCryptoAlgorithm::EciesX25519)]
  pub algo: CliCryptoAlgorithm,
  #[arg(long, value_name = "SEGMENT", default_value = ".c5encval")]
  pub secret_segment: String,
  #[arg(long, value_name = "OUTPUT_FILE_PATH", requires = "commit")]
  pub output_file: Option<PathBuf>,
}

pub fn handle_encrypt(args: EncryptArgs) -> Result<(), C5CoreError> {
  // --- 0. Input Validation (Initial check, more specific handled by clap) ---
  if !args.reencrypt && args.value_to_encrypt.is_none() && args.file_to_encrypt.is_none() {
    return Err(C5CoreError::InvalidInput(
      "For new encryption, you must provide input via -v/--value OR -f/--file.".into(),
    ));
  }

  let core_algo: CoreCryptoAlgo = args.algo.into();
  let full_config_path = args.config_root_dir.join(&args.config_file_name);
  let full_pubkey_path = args.public_key_dir.join(&args.public_key_file_name);

  // --- 1. Load Public Key (for new encryption or as the re-encryption target key) ---
  let public_key = load_ecies_public_key(&full_pubkey_path)?;
  println!("Loaded public key from: {}", full_pubkey_path.display());

  // --- 2. Load existing YAML document (if it exists or if re-encrypting) ---
  let mut yaml_doc_root: Yaml = if args.reencrypt || full_config_path.exists() {
    match fs::read_to_string(&full_config_path) {
      Ok(s) => load_yaml_from_string(&s)?,
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
        if args.reencrypt {
          return Err(C5CoreError::IoWithPath {
            path: full_config_path.clone(),
            source: std::io::Error::new(
              e.kind(),
              format!(
                "Config file not found at '{}' (required for re-encryption).",
                full_config_path.display()
              ),
            ),
          });
        }
        println!(
          "Configuration file '{}' does not exist. A new one will be created if --commit is used.",
          full_config_path.display()
        );
        Yaml::Hash(YamlHash::new()) // Start with an empty map
      }
      Err(e) => {
        return Err(C5CoreError::IoWithPath {
          path: full_config_path.clone(),
          source: e,
        })
      }
    }
  } else {
    println!(
      "Configuration file '{}' does not exist. A new one will be created if --commit is used.",
      full_config_path.display()
    );
    Yaml::Hash(YamlHash::new()) // Start with an empty map
  };

  // --- 3. Determine Plaintext Bytes ---
  let plaintext_bytes: Vec<u8>;
  if args.reencrypt {
    let old_priv_key_path = args
      .old_private_key_file
      .as_ref()
      .expect("--old-private-key-file is required by clap for --reencrypt"); // Clap ensures this

    println!(
      "Re-encrypting secret: key_path='{}', secret_key='{}', config_file='{}'",
      args.key_path,
      args.secret_segment,
      full_config_path.display()
    );
    println!("Using old private key from: {}", old_priv_key_path.display());

    let old_private_key = load_ecies_private_key(old_priv_key_path)?;

    // Navigate to the parent map of the secret for reading
    let mut parent_map_for_read_ref = &yaml_doc_root;
    if !args.key_path.is_empty() {
      for part_str in args.key_path.split('.') {
        if part_str.is_empty() {
          return Err(C5CoreError::YamlNavigation(format!(
            "Invalid empty segment in key_path: '{}'",
            args.key_path
          )));
        }
        parent_map_for_read_ref = match parent_map_for_read_ref.as_hash() {
          Some(map) => map.get(&Yaml::String(part_str.to_string())).ok_or_else(|| {
            C5CoreError::YamlNavigation(format!(
              "Path segment '{}' not found in key path '{}' while looking for existing secret.",
              part_str, args.key_path
            ))
          })?,
          None => {
            return Err(C5CoreError::YamlNavigation(format!(
              "Path segment '{}' in key path '{}' is not a map while looking for existing secret.",
              part_str, args.key_path
            )))
          }
        };
      }
    }

    // Get the existing secret array using args.secret_segment as the key
    let existing_secret_val = match parent_map_for_read_ref.as_hash() {
      Some(map) => map.get(&Yaml::String(args.secret_segment.clone())).ok_or_else(|| {
        C5CoreError::YamlNavigation(format!(
          "Secret key '{}' not found under key path '{}' for re-encryption.",
          args.secret_segment, args.key_path
        ))
      })?,
      None => {
        return Err(C5CoreError::YamlNavigation(format!(
          "Key path '{}' did not resolve to a map for re-encryption.",
          args.key_path
        )))
      }
    };

    let secret_parts = parse_c5_secret_array(existing_secret_val)?;
    let old_ciphertext_bytes = base64_string_to_bytes(&secret_parts.b64_ciphertext)?;
    let algo_for_decryption = match secret_parts.algo_str.as_str() {
      "ecies_x25519" => CoreCryptoAlgo::EciesX25519,
      _ => {
        return Err(C5CoreError::UnsupportedAlgorithm(format!(
          "Algorithm '{}' in existing secret not supported for decryption.",
          secret_parts.algo_str
        )))
      }
    };
    plaintext_bytes = decrypt_data(&old_ciphertext_bytes, &old_private_key, algo_for_decryption)?;
    println!(
      "Successfully decrypted existing value. Plaintext length: {} bytes.",
      plaintext_bytes.len()
    );
  } else if let Some(value_str) = &args.value_to_encrypt {
    println!(
      "Encrypting provided string value for key path: '{}', secret key: '{}'",
      args.key_path, args.secret_segment
    );
    plaintext_bytes = value_str.as_bytes().to_vec();
  } else if let Some(file_to_encrypt_path) = &args.file_to_encrypt {
    println!(
      "Encrypting content of file: '{}' for key path: '{}', secret key: '{}'",
      file_to_encrypt_path.display(),
      args.key_path,
      args.secret_segment
    );
    // If args.encoding != "utf8" (or some binary indicator), and plaintext must be string for some crypto,
    // you might use read_file_to_string here. For ECIES, raw bytes are fine.
    plaintext_bytes = read_file_to_bytes(file_to_encrypt_path)?;
  } else {
    unreachable!("Input validation for encrypt source failed or was bypassed.");
  }

  // --- 4. Encrypt Plaintext (new or decrypted old value) ---
  let mut rng = StdRng::from_os_rng();
  let new_ciphertext_bytes = encrypt_data(&plaintext_bytes, &public_key, core_algo, &mut rng)?;
  let new_b64_ciphertext = bytes_to_base64_string(&new_ciphertext_bytes);
  println!(
    "Encryption successful. Ciphertext length: {} (Base64 encoded).",
    new_b64_ciphertext.len()
  );

  // --- 5. Prepare Secret Array and Update YAML Document ---
  let pk_filename_only = Path::new(&args.public_key_file_name)
    .file_name()
    .and_then(|name| name.to_str())
    .unwrap_or(&args.public_key_file_name); // Fallback to full name if no stem
  let secret_yaml_value_to_set = format_c5_secret_array(core_algo, pk_filename_only, new_b64_ciphertext)?;

  // Get a mutable reference to the parent map specified by args.key_path
  let mut parent_map_mut_ref = &mut yaml_doc_root;
  if !args.key_path.is_empty() {
    for part_str in args.key_path.split('.') {
      if part_str.is_empty() {
        return Err(C5CoreError::YamlNavigation(format!(
          "Invalid empty segment in key_path: '{}'",
          args.key_path
        )));
      }
      let key_yaml_segment = Yaml::String(part_str.to_string());

      if parent_map_mut_ref.is_null() {
        *parent_map_mut_ref = Yaml::Hash(YamlHash::new());
      } else if !parent_map_mut_ref.is_hash() {
        let current_node_type_str = match parent_map_mut_ref {
          /* ... get type string ... */ _ => "Non-Hash",
        };
        return Err(C5CoreError::YamlNavigation(format!(
          "Path segment '{}' in key_path '{}' is not a map (Hash), but a {}.",
          part_str, args.key_path, current_node_type_str
        )));
      }

      match parent_map_mut_ref {
        Yaml::Hash(map) => {
          parent_map_mut_ref = map
            .entry(key_yaml_segment)
            .or_insert_with(|| Yaml::Hash(YamlHash::new()));
        }
        _ => unreachable!("Should be a Hash due to checks above"),
      }
    }
  }

  // Now parent_map_mut_ref refers to the Yaml node that should be the parent map.
  // Insert the actual secret key (e.g., ".c5encval") into this map.
  match parent_map_mut_ref {
    Yaml::Hash(map) => {
      map.insert(Yaml::String(args.secret_segment.clone()), secret_yaml_value_to_set);
    }
    _ => {
      // This handles if key_path was empty and root wasn't a map (e.g. was Null from empty file)
      if args.key_path.is_empty() {
        if yaml_doc_root.is_null() || !yaml_doc_root.is_hash() {
          // If root was null or some scalar
          yaml_doc_root = Yaml::Hash(YamlHash::new()); // Make root a hash
        }
        // Now yaml_doc_root is guaranteed to be a Hash if it was Null, or it was already a Hash.
        // If it was some other type (scalar, array), this won't convert it, leading to an error.
        // A more robust way if root could be non-Null, non-Hash:
        // *yaml_doc_root = Yaml::Hash(new_map_with_secret_inserted);
        match &mut yaml_doc_root {
          // Match again on potentially replaced yaml_doc_root
          Yaml::Hash(root_map) => {
            root_map.insert(Yaml::String(args.secret_segment.clone()), secret_yaml_value_to_set);
          }
          _ => {
            return Err(C5CoreError::YamlNavigation(
              "Root of YAML is not a map and could not be converted, cannot insert secret segment.".to_string(),
            ))
          }
        }
      } else {
        // This implies the path traversal for key_path ended on a non-Hash node that wasn't Null
        return Err(C5CoreError::YamlNavigation(format!(
          "Target for key_path '{}' did not resolve to a writable map.",
          args.key_path
        )));
      }
    }
  }

  let output_yaml_str = dump_yaml_to_string(&yaml_doc_root)?;

  let display_secret_location_info = if args.key_path.is_empty() {
    format!("secret key '{}' at the YAML root", args.secret_segment)
  } else {
    format!(
      "secret key '{}' under YAML path '{}'",
      args.secret_segment, args.key_path
    )
  };

  // --- 6. Commit or Dry Run ---
  if args.commit {
    let write_path = args.output_file.as_ref().unwrap_or(&full_config_path);
    println!("Committing changes to: {}", write_path.display());
    if let Some(parent) = write_path.parent() {
      if !parent.exists() {
        fs::create_dir_all(parent)?;
        println!("Created directory: {}", parent.display());
      }
    }
    write_string_to_file(write_path, &output_yaml_str, true)?;
    println!("Encrypted secret successfully committed.");
  } else {
    println!("\n----- DRY RUN - Encrypt -----");
    println!("Target configuration file would be: {}", full_config_path.display());
    if let Some(out_file) = &args.output_file {
      // Inform about output_file if in dry_run and it's set
      println!(
        "(If committed with --output-file, output would be to: {})",
        out_file.display()
      );
    }
    println!("The {} would be updated/created.", display_secret_location_info);
    println!("\nFull resulting YAML content:");
    println!("{}", output_yaml_str); // This will show ".c5encval" (with quotes)
    println!("\nUse --commit to write these changes.");
  }

  Ok(())
}
