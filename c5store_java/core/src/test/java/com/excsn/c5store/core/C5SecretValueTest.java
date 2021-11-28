package com.excsn.c5store.core;

import com.excsn.c5store.core.secrets.EciesX25519SecretDecryptor;
import com.excsn.c5store.core.telemetry.Logger;
import com.excsn.c5store.core.telemetry.StatsRecorder;
import org.junit.jupiter.api.Assertions;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;
import org.mockito.Mockito;

import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Base64;
import java.util.List;

@TestInstance(TestInstance.Lifecycle.PER_CLASS)
public class C5SecretValueTest {

  private C5StoreRoot _c5Store;

  @BeforeAll
  public void setup() {

    var configRoot = Paths.get("src","test", "resources", "config");
    var secretKeysDir = Path.of(configRoot.toString(),"secret_keys");
    var logger = Mockito.mock(Logger.class);
    var statsRecorder = Mockito.mock(StatsRecorder.class);
    _c5Store = (C5StoreRoot) C5StoreBuilder.builder()
      .setTelemetry(logger, statsRecorder)
      .setSecretKeysPath(secretKeysDir)
      .configureKeyStore((secretKeyStore -> {
        secretKeyStore.setDecryptor("base64", (encryptedValue, key) -> Base64.getDecoder().decode(encryptedValue));
        secretKeyStore.setDecryptor("ecies_x25519", new EciesX25519SecretDecryptor());
        return;
      }))
      .setConfigFilePaths(List.of(
        Path.of(configRoot.toString(), "secret_config.yaml")
      ))
      .build()
      .config;
  }

  @Test
  public void decryptedBase64Secret() {

    var expectedValue = "abcd";
    var secretValue = new String((byte[])_c5Store.get("a_secret"));

    Assertions.assertEquals(expectedValue, secretValue);
  }

  @Test
  public void decryptEciesX25519Secret() {

    var expectedValue = "Hello World";
    var secretValue = new String((byte[])_c5Store.get("hello_secret"));

    Assertions.assertEquals(expectedValue, secretValue);
  }
}
