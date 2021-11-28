package com.excsn.c5store.core.secrets;

import java.io.IOException;
import java.security.GeneralSecurityException;

@FunctionalInterface
public interface SecretDecryptor {
  byte[] decrypt(byte[] encryptedValue, byte[] key) throws GeneralSecurityException, IOException;
}
