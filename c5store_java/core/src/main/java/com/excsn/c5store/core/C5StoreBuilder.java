package com.excsn.c5store.core;

import com.excsn.c5store.core.secrets.ECUtils;
import com.excsn.c5store.core.secrets.SecretKeyStore;
import com.excsn.c5store.core.telemetry.Logger;
import com.excsn.c5store.core.telemetry.StatsRecorder;
import com.excsn.c5store.core.utils.DeepEquals;
import com.google.common.base.Strings;
import org.bouncycastle.crypto.params.X25519PrivateKeyParameters;
import org.bouncycastle.crypto.util.PrivateKeyFactory;
import org.yaml.snakeyaml.Yaml;

import java.io.IOException;
import java.io.StringReader;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Collection;
import java.util.Collections;
import java.util.HashMap;
import java.util.Map;
import java.util.function.Consumer;

public class C5StoreBuilder {

  private Collection<Path> _configSeedFilePaths = Collections.EMPTY_LIST;
  private Logger _logger;
  private StatsRecorder _statsRecorder;
  private int _changeDelayPeriod = 500;
  private String _secretKeyPathSegment = ".c5encval";
  private Path _secretKeysPath = null;
  private final SecretKeyStore secretKeyStore = new SecretKeyStore();

  private C5StoreBuilder() {}

  public static C5StoreBuilder builder() {
    return new C5StoreBuilder();
  }

  public C5StoreBuilder setConfigFilePaths(Collection<Path> paths) {
    this._configSeedFilePaths = paths;
    return this;
  }

  /**
   * @param pathToSecretKeys Path containing all the secret key files
   */
  public C5StoreBuilder setSecretKeysPath(Path pathToSecretKeys) {
    this._secretKeysPath = pathToSecretKeys;
    return this;
  }

  /**
   * @param secretKeyStoreConsumer configures secret key store
   */
  public C5StoreBuilder configureKeyStore(Consumer<SecretKeyStore> secretKeyStoreConsumer) {

    if(secretKeyStoreConsumer == null) {
      throw new IllegalArgumentException("secretKeyStoreConsumer is null");
    }

    secretKeyStoreConsumer.accept(secretKeyStore);
    return this;
  }

  /**
   * @param secretKeyPathSegment The suffix segment to use for designating the key/value as a secret for decryption
   */
  public C5StoreBuilder setSecretKeyPathSegment(String secretKeyPathSegment) {

    if(Strings.isNullOrEmpty(secretKeyPathSegment)) {
      throw new IllegalArgumentException("secretKeyPathSegment is null");
    }

    this._secretKeyPathSegment = secretKeyPathSegment;
    return this;
  }

  public C5StoreBuilder setTelemetry(Logger logger, StatsRecorder statsRecorder) {
    this._logger = logger;
    this._statsRecorder = statsRecorder;
    return this;
  }

  public C5StoreBuilder setChangeDelayPeriod(int changeDelayPeriod) {
    _changeDelayPeriod = changeDelayPeriod;
    return this;
  }

  public C5InitHolder build() {
    var c5StoreSubscriptions = new C5StoreSubscriptions();
    var c5DataStore = new C5DataStore(_logger, _statsRecorder, _secretKeyPathSegment, secretKeyStore);
    var changeNotifier = new ChangeNotifier(c5DataStore, c5StoreSubscriptions, _changeDelayPeriod);

    var c5Store = new C5StoreRoot(
      c5DataStore::getData,
      c5DataStore::exists,
      c5DataStore::keysWithPrefix,
      c5StoreSubscriptions
    );

    SetDataFn setDataFn = (keyPath, value) -> {

      var alreadyExists = c5DataStore.exists(keyPath);
      if(!alreadyExists) {

        c5DataStore.setData(keyPath, value);
      } else {

        var oldValue = c5DataStore.getData(keyPath);
        var isSameValue = DeepEquals.deepEquals(oldValue, value);

        if(!isSameValue) {
          c5DataStore.setData(keyPath, value);
          changeNotifier.changeNotify(keyPath);
        }
      }
    };

    var extractedConfigData = _extractConfigFromSeedFiles();
    var c5StoreMgr = new C5StoreMgr(setDataFn, extractedConfigData.providedData, _logger, _statsRecorder);
    loadSecretKeyFiles(secretKeyStore);

    for (var configDataEntry : extractedConfigData.configData.entrySet()) {

      setDataFn.setData(configDataEntry.getKey(), configDataEntry.getValue());
    }
    Runnable stopFn = () -> {
      c5StoreMgr.stop();
      changeNotifier.stop();
    };

    return new C5InitHolder(c5Store, c5StoreMgr, stopFn);
  }

  private ExtractedConfigData _extractConfigFromSeedFiles() {

    var yaml = new Yaml();
    var rawConfigData = new HashMap<String, Object>();

    for (var configFilePath : _configSeedFilePaths) {

      if (!Files.exists(configFilePath)) {

        continue;
      }

      try {
        var fileContents = Files.readString(configFilePath);
        var configFileYaml = yaml.loadAs(fileContents, Map.class);

        C5StoreUtils.deepMerge(rawConfigData, configFileYaml);
      } catch(IOException e) {
        _logger.error("Error while loading config from `" + configFilePath + "`", e);
      }
    }

    return C5StoreUtils.extractProvidedAndConfigData(rawConfigData);
  }

  void loadSecretKeyFiles(SecretKeyStore secretKeyStore) {
    if(_secretKeysPath == null || !Files.exists(_secretKeysPath)) {
      return;
    }

    try {
      var secretKeysPath = _secretKeysPath.toRealPath();
      Files.list(secretKeysPath).forEach((keyFilePath) -> {

        var fileName = keyFilePath.getFileName().toString();
        var fileExt = com.google.common.io.Files.getFileExtension(fileName);
        var keyName = com.google.common.io.Files.getNameWithoutExtension(fileName);

        try {
          var keyContents = Files.readAllBytes(keyFilePath);

          byte[] key;
          if("pem".equals(fileExt)) {

            var privKey = ECUtils.readPrivateKey(new StringReader(new String(keyContents)));
            key = ((X25519PrivateKeyParameters) PrivateKeyFactory.createKey(privKey.getEncoded())).getEncoded();
          } else {

            key = keyContents;
          }

          secretKeyStore.setKey(keyName, key);
        } catch (IOException e) {

          _logger.error("Could not read contents of key file path `" + keyFilePath + "`", e);
        }
      });
    } catch (IOException e) {
      _logger.error("Tried to load key files from path `" + _secretKeysPath + "`", e);
    }
  }
}
