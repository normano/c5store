package com.excsn.c5store.core.secrets;

import java.security.GeneralSecurityException;
import java.util.Base64;

import com.excsn.security.crypto.ecies_25519.EciesX25519;

public class EciesX25519SecretDecryptor implements SecretDecryptor {

  private final Base64.Decoder b64Decoder = Base64.getDecoder();
  private final EciesX25519 eciesX25519Inst = new EciesX25519();

  @Override
  public byte[] decrypt(byte[] encryptedValue, byte[] key) throws GeneralSecurityException {

    var decodedValue = b64Decoder.decode(encryptedValue);
    return eciesX25519Inst.decrypt(key, decodedValue);
  }
}
