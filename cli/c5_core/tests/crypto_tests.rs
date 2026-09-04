use c5_core::{
  decrypt_data,
  encrypt_data,
  generate_c5_keypair,
  load_ecies_private_key,
  load_ecies_public_key,
  C5CoreError,
  CryptoAlgorithm,
  KeyPair,
  PemEncodedKey,
};
use c5_core::{EciesPublicKey, EciesStaticSecret};
use rand::rngs::{OsRng, StdRng};
use rand::SeedableRng;
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
  let mut rng_for_gen = StdRng::from_os_rng();
  let key_pair: KeyPair = generate_c5_keypair(CryptoAlgorithm::EciesX25519, &mut rng_for_gen)?;

  let pub_key_file = create_temp_pem_file(&key_pair.public.0);
  let priv_key_file = create_temp_pem_file(&key_pair.private.0);

  let loaded_public_key: EciesPublicKey = load_ecies_public_key(pub_key_file.path())?;
  let loaded_private_key: EciesStaticSecret = load_ecies_private_key(priv_key_file.path())?;

  let original_plaintext = b"Hello, c5_core crypto!";

  let mut rng_for_encrypt = StdRng::from_os_rng();
  let ciphertext = encrypt_data(
    original_plaintext,
    &loaded_public_key,
    CryptoAlgorithm::EciesX25519,
    &mut rng_for_encrypt,
  )?;

  assert_ne!(original_plaintext.as_slice(), ciphertext.as_slice());

  let decrypted_plaintext = decrypt_data(&ciphertext, &loaded_private_key, CryptoAlgorithm::EciesX25519)?;

  assert_eq!(original_plaintext.as_slice(), decrypted_plaintext.as_slice());

  Ok(())
}
