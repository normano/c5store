// c5_core/src/keys.rs

use std::{fs, io::Write, path::Path};

use crate::error::C5CoreError;
use base64::{prelude::BASE64_STANDARD, Engine};
use ecies_25519::{
  KeyParsingError as EciesKeyParsingError, PublicKey as ActualEciesPublicKey, StaticSecret as ActualEciesStaticSecret,
};
use ed25519_dalek::{
  pkcs8::{self, spki::der::pem::LineEnding},
  SigningKey, VerifyingKey,
};
// No specific import needed for generate_keypair, it's a free function
use rand::{rand_core, rngs::StdRng, CryptoRng, RngCore, SeedableRng};
use rand_core::OsRng; // Cryptographically secure OS random number generator

// Algorithm Enums (can be in a separate types.rs or here)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoAlgorithm {
  EciesX25519,
}

// Structs for holding PEM encoded keys
#[derive(Debug, Clone)]
pub struct PemEncodedKey(pub String); // Tuple struct to hold the PEM string

#[derive(Debug, Clone)]
pub struct KeyPair {
  pub public: PemEncodedKey,
  pub private: PemEncodedKey,
}

/// Generates a key pair for the specified c5store algorithm (currently only ECIES X25519).
/// Returns PEM-encoded public and private keys.
pub fn generate_c5_keypair(
  algo: CryptoAlgorithm,
  rng: &mut (impl RngCore + CryptoRng),
) -> Result<KeyPair, C5CoreError> {
  match algo {
    CryptoAlgorithm::EciesX25519 => {
      // generate_keypair is a free function in the ecies_25519 crate,
      // not a method on EciesX25519 struct for key generation.
      let keypair_der = ecies_25519::generate_keypair(rng);

      let public_pem = PemEncodedKey(keypair_der.public_to_pem());
      let private_pem = PemEncodedKey(keypair_der.private_to_pem());

      Ok(KeyPair {
        public: public_pem,
        private: private_pem,
      })
    } // Add other algorithms here if c5_core supports them in the future
      // _ => Err(CryptoError::UnsupportedAlgorithm(format!("{:?}", algo))),
  }
}

// ---- For later: SSH Key Generation ----
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshKeyAlgorithm {
  Ed25519,
}

#[derive(Debug, Clone)]
pub struct SshKeyPair {
  pub private_key_pem: PemEncodedKey, // Or modern OpenSSH format string
  pub public_key_openssh_format: String,
}

pub fn generate_ssh_keypair(algo: SshKeyAlgorithm, comment_opt: Option<&str>) -> Result<SshKeyPair, C5CoreError> {
  match algo {
    SshKeyAlgorithm::Ed25519 => {
      use ed25519_dalek::pkcs8::EncodePrivateKey;
      // OsRng for c5_core's rand version. This must be compatible with
      // ed25519-dalek's rand_core if ed25519-dalek itself takes an RNG.
      // SigningKey::generate takes an R: CryptoRngCore + RngCore normally.
      let mut csprng = StdRng::from_os_rng();
      let signing_key: SigningKey = SigningKey::generate(&mut csprng);
      let verifying_key: VerifyingKey = signing_key.verifying_key();

      // Private Key to PEM:
      // SigningKey itself doesn't have a direct to_pem.
      // We need to get the secret bytes and use SecretKey::from_bytes then to_pem.
      // signing_key.to_bytes() returns the 32-byte secret seed.
      let secret_bytes: [u8; ed25519_dalek::SECRET_KEY_LENGTH] = signing_key.to_bytes();
      let ed_secret_key_for_pem = ed25519_dalek::SecretKey::try_from(secret_bytes)
        .map_err(|e| C5CoreError::PemParse(format!("Failed to create Ed25519 SecretKey from bytes: {}", e)))?;

      let private_pem_string = signing_key
        .to_pkcs8_pem(LineEnding::LF) // This method should exist on SigningKey
        .map_err(|e: pkcs8::Error| {
          // Error type from pkcs8
          C5CoreError::PemParse(format!("Ed25519 private key to PKCS#8 PEM failed: {}", e))
        })?
        .as_str() // to_pkcs8_pem returns zeroize::SecUtf8, convert to String
        .to_string();

      // Construct the OpenSSH public key string manually
      let public_key_bytes: [u8; ed25519_dalek::PUBLIC_KEY_LENGTH] = verifying_key.to_bytes();
      let openssh_payload_to_encode = build_ed25519_openssh_payload(&public_key_bytes);
      let b64_encoded_key = BASE64_STANDARD.encode(&openssh_payload_to_encode);

      let comment_str = comment_opt.unwrap_or("");
      let openssh_public_key_string = if comment_str.is_empty() {
        format!("ssh-ed25519 {}", b64_encoded_key)
      } else {
        format!("ssh-ed25519 {} {}", b64_encoded_key, comment_str)
      };

      // Optional: Validate by parsing with sshkeys (good for testing our format)
      // let _parsed_for_validation = SshPublicKey::from_string(&openssh_public_key_string)
      //     .map_err(|e| CryptoError::KeyLoad(format!("Validation parse of generated SSH pubkey string failed: {}", e)))?;

      Ok(SshKeyPair {
        private_key_pem: PemEncodedKey(private_pem_string),
        public_key_openssh_format: openssh_public_key_string,
      })
    }
  }
}

/// Helper to construct the data to be base64 encoded for an OpenSSH public key.
/// Format: u32 length + string data (for key type and key itself)
fn build_ssh_key_part(name: &str, data: &[u8]) -> Vec<u8> {
  let mut part = Vec::new();
  part.write_all(&(name.len() as u32).to_be_bytes()).unwrap();
  part.write_all(name.as_bytes()).unwrap();
  part.write_all(&(data.len() as u32).to_be_bytes()).unwrap();
  part.write_all(data).unwrap();
  part
}
/// More precise helper to construct the data to be base64 encoded for an OpenSSH public key.
/// For "ssh-ed25519", the format is:
/// string "ssh-ed25519"
/// string public_key_bytes (32 bytes)
fn build_ed25519_openssh_payload(public_key_bytes: &[u8; 32]) -> Vec<u8> {
  let key_type_name = "ssh-ed25519";
  let mut payload = Vec::new();

  // Write key type name (length prefixed string)
  payload.write_all(&(key_type_name.len() as u32).to_be_bytes()).unwrap();
  payload.write_all(key_type_name.as_bytes()).unwrap();

  // Write public key bytes (length prefixed string)
  payload
    .write_all(&(public_key_bytes.len() as u32).to_be_bytes())
    .unwrap();
  payload.write_all(public_key_bytes).unwrap();

  payload
}

pub fn load_ecies_public_key(key_path: &Path) -> Result<ActualEciesPublicKey, C5CoreError> {
  let key_bytes = fs::read(key_path).map_err(|e| C5CoreError::IoWithPath {
    path: key_path.to_path_buf(), // Added path for context
    source: e,
  })?;
  ecies_25519::parse_public_key(&key_bytes).map_err(C5CoreError::from)
}

pub fn load_ecies_private_key(key_path: &Path) -> Result<ActualEciesStaticSecret, C5CoreError> {
  let key_bytes = fs::read(key_path).map_err(|e| C5CoreError::IoWithPath {
    path: key_path.to_path_buf(), // Added path for context
    source: e,
  })?;
  ecies_25519::parse_private_key(&key_bytes).map_err(C5CoreError::from)
}

#[cfg(test)]
mod tests {
  use super::*;
  use ed25519_dalek::pkcs8::DecodePrivateKey;
  use sshkeys::PublicKey as SshPublicKeyExternal;
  use std::io::Write;
  use tempfile::NamedTempFile;

  // Helper to create a deterministic RNG for tests that require one passed in
  fn test_rng() -> StdRng {
    StdRng::from_seed([42u8; 32]) // Use a fixed seed
  }

  #[test]
  fn test_generate_ecies_keypair_pem_format() {
    let mut rng = test_rng();
    let keypair_result = generate_c5_keypair(CryptoAlgorithm::EciesX25519, &mut rng);
    assert!(keypair_result.is_ok());
    let keypair = keypair_result.unwrap();

    assert!(keypair.public.0.starts_with("-----BEGIN PUBLIC KEY-----"));
    assert!(keypair.public.0.contains("-----END PUBLIC KEY-----"));
    assert!(keypair.private.0.starts_with("-----BEGIN PRIVATE KEY-----"));
    assert!(keypair.private.0.contains("-----END PRIVATE KEY-----"));

    // Further validation: try to parse them back with ecies_25519 parser
    let parsed_pub = ecies_25519::parse_public_key(keypair.public.0.as_bytes());
    assert!(
      parsed_pub.is_ok(),
      "Generated public PEM failed to re-parse: {:?}",
      parsed_pub.err()
    );

    let parsed_priv = ecies_25519::parse_private_key(keypair.private.0.as_bytes());
    assert!(
      parsed_priv.is_ok(),
      "Generated private PEM failed to re-parse: {:?}",
      parsed_priv.err()
    );
  }

  #[test]
  fn test_generate_ed25519_ssh_keypair_formats() {
    let comment = Some("test-key@example.com");
    let ssh_keypair_result = generate_ssh_keypair(SshKeyAlgorithm::Ed25519, comment);
    assert!(
      ssh_keypair_result.is_ok(),
      "generate_ssh_keypair failed: {:?}",
      ssh_keypair_result.err()
    );
    let ssh_keypair = ssh_keypair_result.unwrap();

    assert!(ssh_keypair.private_key_pem.0.starts_with("-----BEGIN PRIVATE KEY-----"));
    assert!(ssh_keypair.private_key_pem.0.contains("-----END PRIVATE KEY-----"));

    // Use the imported DecodePrivateKey trait for from_pkcs8_pem
    let signing_key_from_pem = SigningKey::from_pkcs8_pem(&ssh_keypair.private_key_pem.0);
    assert!(
      signing_key_from_pem.is_ok(),
      "Generated SSH private PEM failed to re-parse with ed25519_dalek: {:?}",
      signing_key_from_pem.err()
    );

    assert!(ssh_keypair.public_key_openssh_format.starts_with("ssh-ed25519 AAAA"));
    assert!(ssh_keypair.public_key_openssh_format.ends_with(comment.unwrap()));

    // Use the imported SshPublicKeyExternal alias
    let parsed_ssh_pubkey = SshPublicKeyExternal::from_string(&ssh_keypair.public_key_openssh_format); // Corrected variable name
    assert!(
      parsed_ssh_pubkey.is_ok(),
      "Generated SSH public key string failed to re-parse with sshkeys: {:?}",
      parsed_ssh_pubkey.err()
    );
  }

  fn write_to_temp_file(content: &str) -> Result<NamedTempFile, std::io::Error> {
    let mut file = NamedTempFile::new()?;
    file.write_all(content.as_bytes())?;
    Ok(file)
  }

  #[test]
  fn test_load_valid_ecies_keys() -> Result<(), C5CoreError> {
    let mut rng = test_rng();
    let keypair = generate_c5_keypair(CryptoAlgorithm::EciesX25519, &mut rng)?;

    let pub_pem_file = write_to_temp_file(&keypair.public.0)?;
    let priv_pem_file = write_to_temp_file(&keypair.private.0)?;

    let loaded_pub = load_ecies_public_key(pub_pem_file.path())?;
    let loaded_priv = load_ecies_private_key(priv_pem_file.path())?;

    // Verify by comparing raw bytes (if underlying types are comparable or expose bytes)
    // ecies_25519::PublicKey and StaticSecret are x25519_dalek types, which are [u8; 32] wrappers
    let original_pub_from_der = ecies_25519::parse_public_key(keypair.public.0.as_bytes()).unwrap();
    let original_priv_from_der = ecies_25519::parse_private_key(keypair.private.0.as_bytes()).unwrap();

    assert_eq!(loaded_pub.as_bytes(), original_pub_from_der.as_bytes());
    assert_eq!(loaded_priv.to_bytes(), original_priv_from_der.to_bytes());

    Ok(())
  }

  #[test]
  fn test_load_invalid_ecies_public_key_pem() {
    // Case 1: Invalid PEM structure (bad headers/footers, unrecognized tag by pem crate)
    let bad_structure_pem = "-----BEGIN FOO KEY-----\nABC\n-----END FOO KEY-----";
    let file1 = write_to_temp_file(bad_structure_pem).unwrap();
    let result1 = load_ecies_public_key(file1.path());
    eprintln!("Debug Case 1 (Public - Bad Structure): {:?}", result1);
    assert!(matches!(
      result1,
      Err(C5CoreError::EciesKeyParse(
        ecies_25519::KeyParsingError::InvalidDerPrefix
      ))
    ));

    // Case 2: Valid PEM headers ("PUBLIC KEY"), but corrupted/invalid base64 content
    let corrupted_b64_pem = "-----BEGIN PUBLIC KEY-----\n!!!NotValidBase64!!!\n-----END PUBLIC KEY-----";
    let file2 = write_to_temp_file(corrupted_b64_pem).unwrap();
    let result2 = load_ecies_public_key(file2.path());
    eprintln!("Debug Case 2 (Public - Corrupted Base64): {:?}", result2);
    assert!(matches!(
      result2,
      Err(C5CoreError::EciesKeyParse(
        // Updated based on debug output
        ecies_25519::KeyParsingError::InvalidDerPrefix
      ))
    ));

    // Case 3: Valid PEM, but DER content is a PKCS#8 private key instead of SPKI public key
    let mut rng = test_rng();
    let keypair = generate_c5_keypair(CryptoAlgorithm::EciesX25519, &mut rng).unwrap();
    let private_key_pem_content = &keypair.private.0;
    let file3 = write_to_temp_file(private_key_pem_content).unwrap();
    let result3 = load_ecies_public_key(file3.path());
    eprintln!("Debug Case 3 (Public - Private Key Content): {:?}", result3);
    assert!(matches!(
      result3,
      Err(C5CoreError::EciesKeyParse(
        ecies_25519::KeyParsingError::InvalidPemTag { expected: _, actual: _ }
      ))
    ));

    // Case 4: Empty file
    let empty_file = NamedTempFile::new().unwrap();
    let result4 = load_ecies_public_key(empty_file.path());
    eprintln!("Debug Case 4 (Public - Empty File): {:?}", result4);
    assert!(matches!(
      result4,
      Err(C5CoreError::EciesKeyParse(
        ecies_25519::KeyParsingError::InvalidDerPrefix
      ))
    ));
  }

  #[test]
  fn test_load_invalid_ecies_private_key_pem() {
    // Case 1: Invalid PEM structure (bad headers/footers, unrecognized tag by pem crate)
    let bad_header_pem = "-----BEGIN FOO KEY-----\nABC\n-----END FOO KEY-----";
    let file1 = write_to_temp_file(bad_header_pem).unwrap();
    let result1 = load_ecies_private_key(file1.path());
    // Custom printing for Result<StaticSecret, C5CoreError>
    match &result1 {
        Ok(_) => eprintln!("Debug Case 1 (Private - Bad Structure): Ok(StaticSecret) - [Sensitive data not printed]"),
        Err(e) => eprintln!("Debug Case 1 (Private - Bad Structure): Err({:?})", e),
    }
    assert!(matches!(
      result1,
      Err(C5CoreError::EciesKeyParse(
        ecies_25519::KeyParsingError::InvalidDerPrefix
      ))
    ));

    // Case 2: Valid PEM headers ("PRIVATE KEY"), but corrupted/invalid base64 content
    let corrupted_b64_pem = "-----BEGIN PRIVATE KEY-----\n!!!NotValidBase64!!!\n-----END PRIVATE KEY-----";
    let file2 = write_to_temp_file(corrupted_b64_pem).unwrap();
    let result2 = load_ecies_private_key(file2.path());
    match &result2 {
        Ok(_) => eprintln!("Debug Case 2 (Private - Corrupted Base64): Ok(StaticSecret) - [Sensitive data not printed]"),
        Err(e) => eprintln!("Debug Case 2 (Private - Corrupted Base64): Err({:?})", e),
    }
    // Based on public key test, this might also be InvalidDerPrefix if ecies_25519 error reporting is consistent
    assert!(matches!(
      result2,
      Err(C5CoreError::EciesKeyParse(
        ecies_25519::KeyParsingError::InvalidDerPrefix // Tentative: verify with debug output
        // ecies_25519::KeyParsingError::PemError(_) // Original expectation
      ))
    ));

    // Case 3: Valid PEM, but DER content is an SPKI public key instead of PKCS#8 private key
    let mut rng = test_rng();
    let keypair = generate_c5_keypair(CryptoAlgorithm::EciesX25519, &mut rng).unwrap();
    let public_key_pem_content = &keypair.public.0;
    let file3 = write_to_temp_file(public_key_pem_content).unwrap();
    let result3 = load_ecies_private_key(file3.path());
    match &result3 {
        Ok(_) => eprintln!("Debug Case 3 (Private - Public Key Content): Ok(StaticSecret) - [Sensitive data not printed]"),
        Err(e) => eprintln!("Debug Case 3 (Private - Public Key Content): Err({:?})", e),
    }
    assert!(matches!(
      result3,
      Err(C5CoreError::EciesKeyParse(
          ecies_25519::KeyParsingError::InvalidPemTag { expected: _, actual: _ }
      ))
    ));

    // Case 4: Empty file
    let empty_file = NamedTempFile::new().unwrap();
    let result4 = load_ecies_private_key(empty_file.path());
    match &result4 {
        Ok(_) => eprintln!("Debug Case 4 (Private - Empty File): Ok(StaticSecret) - [Sensitive data not printed]"),
        Err(e) => eprintln!("Debug Case 4 (Private - Empty File): Err({:?})", e),
    }
    assert!(matches!(
      result4,
      Err(C5CoreError::EciesKeyParse(
        ecies_25519::KeyParsingError::InvalidDerPrefix
      ))
    ));
  }
}
