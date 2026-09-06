use c5_core::{
  C5CoreError, CryptoAlgorithm as CoreCryptoAlgo, Document, Value, base64_string_to_bytes, bytes_to_base64_string,
  decrypt_data, encrypt_data, format_c5_secret_array,
  io_utils::{read_file_to_bytes, write_string_to_file},
  load_ecies_private_key, load_ecies_public_key, parse_c5_secret_array, parse_path,
};
use clap::Args;
use rand::SeedableRng;
use rand::rngs::StdRng;
use std::fs;
use std::path::{Path, PathBuf};

use crate::CliCryptoAlgorithm;

#[derive(Args, Debug)]
#[clap(after_help = "EXAMPLES:\n\
    # Dry-run: Encrypt a password into 'config/dev.yaml' at path 'db.password'\n\
    c5cli encrypt dev.yaml my_key.pub.pem db.password -v 's3cr3t!'\n\n\
    # Commit the encryption of a file's content into an array element\n\
    c5cli encrypt prod.yaml prod.pub.pem 'users[0].ssh_key' -f ~/.ssh/id_rsa.pub --commit\n\n\
    # Re-encrypt an existing secret with a new key\n\
    c5cli encrypt app.yaml new.pub.pem app.token --reencrypt --old-private-key-file config/keys/old.key.pem --commit")]
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

  // --- 2. Load the configuration, in whatever format it is written ---
  if args.reencrypt && !full_config_path.exists() {
    return Err(C5CoreError::IoWithPath {
      path: full_config_path.clone(),
      source: std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "configuration file not found (required for re-encryption)",
      ),
    });
  }
  if !full_config_path.exists() {
    println!(
      "Configuration file '{}' does not exist. A new one will be created if --commit is used.",
      full_config_path.display()
    );
  }
  let mut document = Document::load(&full_config_path)?;
  let segments = parse_path(&args.key_path)?;
  if segments.is_empty() {
    return Err(C5CoreError::InvalidInput(
      "An empty key path is not valid for encryption.".into(),
    ));
  }

  // --- 3. Determine Plaintext Bytes ---
  let plaintext_bytes: Vec<u8>;
  if args.reencrypt {
    let old_priv_key_path = args
      .old_private_key_file
      .as_ref()
      .expect("--old-private-key-file is required by clap for --reencrypt");

    println!(
      "Re-encrypting secret: key_path='{}', secret_key='{}', config_file='{}'",
      args.key_path,
      args.secret_segment,
      full_config_path.display()
    );
    println!("Using old private key from: {}", old_priv_key_path.display());

    let old_private_key = load_ecies_private_key(old_priv_key_path)?;

    let holder = document.get(&segments)?.ok_or_else(|| {
      C5CoreError::YamlNavigation(format!(
        "Path '{}' names nothing, so there is no secret to re-encrypt.",
        args.key_path
      ))
    })?;
    let existing_secret_val = holder.get(&args.secret_segment).ok_or_else(|| {
      C5CoreError::YamlNavigation(format!(
        "Secret segment '{}' not found under path '{}' for re-encryption.",
        args.secret_segment, args.key_path
      ))
    })?;

    let secret_parts = parse_c5_secret_array(existing_secret_val)?;
    let old_ciphertext_bytes = base64_string_to_bytes(&secret_parts.b64_ciphertext)?;
    let algo_for_decryption = match secret_parts.algo_str.as_str() {
      "ecies_x25519" => CoreCryptoAlgo::EciesX25519,
      _ => {
        return Err(C5CoreError::UnsupportedAlgorithm(format!(
          "Algorithm '{}' in existing secret not supported for decryption.",
          secret_parts.algo_str
        )));
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
    .unwrap_or(&args.public_key_file_name);
  let secret_value_to_set = format_c5_secret_array(core_algo, pk_filename_only, new_b64_ciphertext)?;

  document.set(&segments, &Value::Map(vec![(args.secret_segment.clone(), secret_value_to_set)]))?;

  let output_text = document.text().to_owned();

  let display_secret_location_info = if args.key_path.is_empty() {
    format!("secret key '{}' at the {} root", args.secret_segment, document.format().name())
  } else {
    format!(
      "secret key '{}' under {} path '{}'",
      args.secret_segment,
      document.format().name(),
      args.key_path
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
    write_string_to_file(write_path, &output_text, true)?;
    println!("Encrypted secret successfully committed.");
  } else {
    println!("\n----- DRY RUN - Encrypt -----");
    println!("Target configuration file would be: {}", full_config_path.display());
    if let Some(out_file) = &args.output_file {
      println!(
        "(If committed with --output-file, output would be to: {})",
        out_file.display()
      );
    }
    println!("The {} would be updated/created.", display_secret_location_info);
    println!("\nFull resulting {} content:", document.format().name());
    println!("{}", output_text);
    println!("\nUse --commit to write these changes.");
  }

  Ok(())
}
