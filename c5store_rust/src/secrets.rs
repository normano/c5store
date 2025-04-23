#[cfg(feature = "secrets")]
use std::collections::HashMap;
use ecies_25519::{EciesX25519, StaticSecret};
use base64::Engine;

#[derive(Debug)]
pub enum SecretDescryptorError {
  EncryptionFailed,
  DecryptionFailed,
  DecodeFailed,
  BadKeyPubPriv,
}

pub trait SecretDecryptor: Sync + Send {
  fn decrypt(&self, encrypted_value: &Vec<u8>, key: &Vec<u8>) -> Result<Vec<u8>, SecretDescryptorError>;
}

pub (in crate) struct Base64SecretDecryptor {}

impl SecretDecryptor for Base64SecretDecryptor {
  fn decrypt(&self, encrypted_value: &Vec<u8>, _key_bytes: &Vec<u8>) -> Result<Vec<u8>, SecretDescryptorError> {
    
    let output_result = base64::decode(encrypted_value);

    if output_result.is_err() {
      return Err(SecretDescryptorError::DecodeFailed);
    }

    return Ok(output_result.unwrap());
  }
}

pub struct EciesX25519SecretDecryptor {
  _ecies25519: EciesX25519,
}

impl EciesX25519SecretDecryptor {
  pub fn new(ecies25519: EciesX25519) -> Self {

    return Self {
      _ecies25519: ecies25519,
    }
  }
}

impl SecretDecryptor for EciesX25519SecretDecryptor {
  fn decrypt(&self, encrypted_value: &Vec<u8>, key_bytes: &Vec<u8>) -> Result<Vec<u8>, SecretDescryptorError> {

    let decoded_value_result = base64::decode(&encrypted_value);

    if decoded_value_result.is_err() {
      return Err(SecretDescryptorError::DecodeFailed);
    }

    let decoded_value = decoded_value_result.unwrap();

    let mut key_32bytes = [0u8; 32];
    key_32bytes[..32].clone_from_slice(&key_bytes);
    let key = StaticSecret::from(key_32bytes);

    match self._ecies25519.decrypt(&key, &decoded_value) {
      Ok(value) => {

        return Ok(value);
      },
      Err(ecies_25519::Error::EncryptionFailed | ecies_25519::Error::EncryptionFailedRng) => {

        return Err(SecretDescryptorError::EncryptionFailed);
      },
      Err(ecies_25519::Error::DecryptionFailed | ecies_25519::Error::DecryptionFailedCiphertextShort) => {

        return Err(SecretDescryptorError::DecryptionFailed);
      },
      Err(ecies_25519::Error::InvalidPublicKeyBytes | ecies_25519::Error::InvalidSecretKeyBytes) => {

        return Err(SecretDescryptorError::BadKeyPubPriv);
      },
    }
  }
}

pub struct SecretKeyStore {
  _secret_decryptors: HashMap<String, Box<dyn SecretDecryptor>>,
  _keys: HashMap<String, Vec<u8>>,
}

impl SecretKeyStore {

  pub fn new() -> Self {

    let secret_decryptors = HashMap::new();
    let keys = HashMap:: new();

    return SecretKeyStore {
      _secret_decryptors: secret_decryptors,
      _keys: keys,
    };
  }

  pub fn get_decryptor(&self, name: &str) -> Option<&Box<dyn SecretDecryptor>> {
    return self._secret_decryptors.get(name);
  }

  pub fn set_decryptor(&mut self, name: &str, decryptor: Box<dyn SecretDecryptor>) {
    self._secret_decryptors.insert(name.to_string(), decryptor);
  }

  pub fn get_key(&self, name: &str) -> Option<&Vec<u8>> {
    return self._keys.get(name);
  }

  pub fn set_key(&mut self, name: &str, key: Vec<u8>) {
    self._keys.insert(name.to_string(), key);
  }
}