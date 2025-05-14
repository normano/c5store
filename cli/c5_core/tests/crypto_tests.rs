// c5_core/tests/crypto_tests.rs
use c5_core::{
  decrypt_data,
  encrypt_data,
  generate_c5_keypair,
  load_ecies_private_key, // Assuming these are in keys module
  load_ecies_public_key,
  C5CoreError,
  CryptoAlgorithm,
  KeyPair,
  PemEncodedKey,
};
use c5_core::{EciesPublicKey, EciesStaticSecret}; // Types from ecies_25519 via c5_core's re-export
use rand::rngs::{OsRng, StdRng};
use rand::SeedableRng; // For functions in c5_core that might still take an RNG, like generate_c5_keypair
use std::fs;
use std::path::Path;
use tempfile::NamedTempFile;

fn create_temp_pem_file(content: &str) -> NamedTempFile {
  use std::io::Write;
  let mut file = NamedTempFile::new().unwrap();
  file.write_all(content.as_bytes()).unwrap();
  file
}

#[test]
fn test_encrypt_decrypt_round_trip() -> Result<(), C5CoreError> {
  // 1. Generate a keypair using c5_core's function
  //    generate_c5_keypair itself calls ecies_25519::generate_keypair.
  //    If your forked ecies_25519::generate_keypair needs an RNG,
  //    then c5_core::generate_c5_keypair must also take one or create one.
  //    Let's assume c5_core::generate_c5_keypair now needs an RNG.
  let mut rng_for_gen = StdRng::from_os_rng();
  let key_pair: KeyPair = generate_c5_keypair(CryptoAlgorithm::EciesX25519, &mut rng_for_gen)?; // Pass RNG

  // Create temporary files for the PEM keys
  let pub_key_file = create_temp_pem_file(&key_pair.public.0);
  let priv_key_file = create_temp_pem_file(&key_pair.private.0);

  // 2. Load them back using c5_core's functions
  let loaded_public_key: EciesPublicKey = load_ecies_public_key(pub_key_file.path())?;
  let loaded_private_key: EciesStaticSecret = load_ecies_private_key(priv_key_file.path())?;

  let original_plaintext = b"Hello, c5_core crypto!";

  // 3. Encrypt
  //    encrypt_data in c5_core takes the RNG as per our current design for it
  //    (which it then passes to the forked ecies_25519::encrypt)
  let mut rng_for_encrypt = StdRng::from_os_rng();
  let ciphertext = encrypt_data(
    original_plaintext,
    &loaded_public_key,
    CryptoAlgorithm::EciesX25519,
    &mut rng_for_encrypt, // Pass RNG
  )?;

  assert_ne!(original_plaintext.as_slice(), ciphertext.as_slice());

  // 4. Decrypt
  let decrypted_plaintext = decrypt_data(&ciphertext, &loaded_private_key, CryptoAlgorithm::EciesX25519)?;

  assert_eq!(original_plaintext.as_slice(), decrypted_plaintext.as_slice());

  Ok(())
}
