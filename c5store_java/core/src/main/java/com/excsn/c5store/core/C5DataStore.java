package com.excsn.c5store.core;

import com.excsn.c5store.core.secrets.SecretKeyStore;
import com.excsn.c5store.core.telemetry.Logger;
import com.excsn.c5store.core.telemetry.StatsRecorder;
import org.bouncycastle.util.Bytes;
import se.sawano.java.text.AlphanumericComparator;

import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.security.GeneralSecurityException;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.*;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentMap;
import java.util.concurrent.ConcurrentSkipListMap;

class C5DataStore {

  private final Logger _logger;
  private final StatsRecorder _statsRecorder;
  private final String _secretKeyPathSegment;
  private final SecretKeyStore _secretKeyStore;
  private final NavigableMap<String, Object> _data = new ConcurrentSkipListMap<>(new AlphanumericComparator());
  private final ConcurrentMap<String, byte[]> _valueHashCache = new ConcurrentHashMap<>();

  public C5DataStore(Logger logger, StatsRecorder statsRecorder, String secretKeyPathSegmentSuffix, SecretKeyStore secretKeyStore) {
    _logger = logger;
    _statsRecorder = statsRecorder;
    _secretKeyPathSegment = "." +secretKeyPathSegmentSuffix;
    _secretKeyStore = secretKeyStore;
  }

  public <T> T getData(String keyPath) {
    _statsRecorder.recordCounterIncrement(Map.of("group", "c5store"), "get_attempts");
    //noinspection unchecked
    return (T) _data.get(keyPath);
  }

  public void setData(String keyPath, Object value) {

    _statsRecorder.recordCounterIncrement(Map.of("group", "c5store"), "set_attempts");
    if(keyPath.endsWith(_secretKeyPathSegment)) {

      try {
        // Decrypt data and lop off the secret key path segment
        var decryptedVal = _getSecret(value, keyPath);

        if(decryptedVal == null) {
          return; // No value to store

        }
        var dataPath = keyPath.substring(0, keyPath.length() - _secretKeyPathSegment.length());
        _data.put(dataPath, decryptedVal);
      } catch (GeneralSecurityException | IOException e) {

        _logger.error("Could not set data for key path `" + keyPath + "`", e);
        _statsRecorder.recordCounterIncrement(Map.of("group", "c5store"), "set_errors");
      }
    } else {
      _data.put(keyPath, value);
    }
  }

  public boolean exists(String keyPath) {
    _statsRecorder.recordCounterIncrement(Map.of("group", "c5store"), "exists_attempts");
    return _data.containsKey(keyPath);
  }

  /**
   * This queries the datastore for "{prefixKeyPath}.", so if you had "{prefixKeyPath}.somekey" and "{prefixKeyPath}s"
   * then only "{prefixKeyPath}.somekey" will be returned.
   *
   * Special case is when keyPath is null, then all keys are returned.
   *
   * @return Iterator of keys with "{prefixKeyPath}." or of all keys if keyPath is null
   */
  public Iterator<String> keysWithPrefix(String keyPath) {

    if (keyPath == null) {
      return _data.navigableKeySet().iterator();
    }

    var prefixMap = _data.tailMap(keyPath, true);

    var prefixMapKeySetIterator = prefixMap.keySet().iterator();

    return new Iterator<>() {
      String _nextString;

      @Override
      public boolean hasNext() {
        if(_nextString != null) {
          return true;
        }

        if(!prefixMapKeySetIterator.hasNext()) {
          return false;
        }

        var candidate = prefixMapKeySetIterator.next();

        if(!candidate.startsWith(keyPath + ".")) {
          return false;
        }

        _nextString = candidate;
        return true;
      }

      @Override
      public String next() {
        var nextString = _nextString;
        _nextString = null;

        return nextString;
      }
    };
  }

  public Object _getSecret(Object rawData, String keyPath) throws GeneralSecurityException, IOException {

    var data = (ArrayList<Object>) rawData;
    if(data == null || data.size() != 3) {
      throw new IllegalArgumentException("Key Path '" + keyPath + "' does not have the required number of arguments");
    }

    var algo = data.get(0);
    var secretKeyName = data.get(1);
    var encodedData = data.get(2);

    if(algo == null || !(algo instanceof String && ((String)algo).length() > 0)) {
      throw new IllegalArgumentException("Key Path '" + keyPath + "' algo is invalid");
    }

    if(secretKeyName == null || !(secretKeyName instanceof String && ((String)secretKeyName).length() > 0)) {
      throw new IllegalArgumentException("Key Path '" + keyPath + "' is invalid");
    }

    if(encodedData == null || !(encodedData instanceof String && ((String)encodedData).length() > 0)) {
      throw new IllegalArgumentException("Key Path  '" + keyPath + "' encodedData is invalid");
    }

    var hashValue = _calcValueHash(algo.toString(), secretKeyName.toString(), encodedData.toString());
    if(_valueHashCache.containsKey(keyPath)) {
      var existingHashValue = _valueHashCache.get(keyPath);

      if(Arrays.equals(existingHashValue, hashValue)) {
        return null;
      }
    } else {
      _valueHashCache.put(keyPath, hashValue);
    }

    _statsRecorder.recordCounterIncrement(Map.of("group", "c5store"), "set_secret_attempts");

    var decryptor = _secretKeyStore.getDecryptor((String)algo);

    if(decryptor == null) {
      throw new IllegalArgumentException("Key Path '" + keyPath + "' Secret Key decryptor does not exist");
    }

    var key = _secretKeyStore.getKey((String)secretKeyName);
    if(key == null) {
      throw new IllegalArgumentException("Key Path '" + keyPath + "' Secret Key does not have key data loaded");
    }

    return decryptor.decrypt(((String) encodedData).getBytes(), key);
  }

  private byte[] _calcValueHash(String... values) {

    try {
      var valueBytes = String.join("/", values).getBytes(StandardCharsets.UTF_8);

      return MessageDigest.getInstance("SHA-256").digest(valueBytes);

    } catch (NoSuchAlgorithmException e) {

      return null;
    }
  }
}
