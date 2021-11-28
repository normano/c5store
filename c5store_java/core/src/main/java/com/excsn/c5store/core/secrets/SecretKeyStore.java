package com.excsn.c5store.core.secrets;

import java.util.HashMap;
import java.util.Map;

/**
 * Stores decryptors and keys for decryption coordination.
 */
public class SecretKeyStore {

  private final Map<String, SecretDecryptor> _secretDecryptors;
  private final Map<String, byte[]> _keys;

  public  SecretKeyStore() {
    _secretDecryptors = new HashMap<>();
    _keys = new HashMap<>();
  }

  public SecretKeyStore(Map<String, SecretDecryptor> secretKeyProviders) {
    _secretDecryptors = secretKeyProviders;
    _keys = new HashMap<>();
  }

  public SecretDecryptor getDecryptor(String name) {
    return _secretDecryptors.get(name);
  }

  public void setDecryptor(String name, SecretDecryptor decryptor) {
    _secretDecryptors.put(name, decryptor);
  }

  public byte[] getKey(String name) {
    return _keys.get(name);
  }

  public void setKey(String name, byte[] key) {
    _keys.put(name, key);
  }
}
