use crate::error::C5CoreError;
use crate::keys::CryptoAlgorithm;

use ecies_25519::{
  EciesX25519,
  Error as EciesError,              
  PublicKey as ActualEciesPublicKey,  
  StaticSecret as ActualEciesStaticSecret,
};
use rand_core::{CryptoRng, RngCore};

pub fn encrypt_data(
  plaintext: &[u8],
  public_key: &ActualEciesPublicKey,
  algo: CryptoAlgorithm,
  rng: &mut (impl RngCore + CryptoRng), 
) -> Result<Vec<u8>, C5CoreError> {
  match algo {
    CryptoAlgorithm::EciesX25519 => {
      let ecies_inst = EciesX25519::new();
      ecies_inst.encrypt(public_key, plaintext, rng).map_err(EciesError::into)
    }
  }
}

pub fn decrypt_data(
  ciphertext: &[u8],
  private_key: &ActualEciesStaticSecret,
  algo: CryptoAlgorithm,
) -> Result<Vec<u8>, C5CoreError> {
  match algo {
    CryptoAlgorithm::EciesX25519 => {
      let ecies_inst = EciesX25519::new();
      ecies_inst.decrypt(private_key, ciphertext).map_err(EciesError::into)
    }
  }
}
