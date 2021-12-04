package com.excsn.c5store.core;

import com.google.common.base.Objects;
import com.google.common.base.Preconditions;
import com.google.common.collect.Multimap;
import com.google.common.collect.MultimapBuilder;

import java.nio.file.Path;
import java.util.*;

public final class C5StoreUtils {

  public static Collection<Path> defaultConfigFilePaths(
    String configDir,
    String releaseEnv,
    String env,
    String datacenter
  ) {

    var filePaths = new ArrayList<Path>();

    filePaths.add(Path.of(configDir, "common.yaml").toAbsolutePath());
    filePaths.add(Path.of(configDir, releaseEnv + ".yaml").toAbsolutePath());
    filePaths.add(Path.of(configDir, env + ".yaml").toAbsolutePath());
    filePaths.add(Path.of(configDir, datacenter + ".yaml").toAbsolutePath());
    filePaths.add(Path.of(configDir, env + "-" + datacenter + ".yaml").toAbsolutePath());

    return filePaths;
  }

  static ExtractedConfigData extractProvidedAndConfigData(Map<String, Object> rawConfigData) {

    var configData = new HashMap<String, Object>();
    var providedData = MultimapBuilder.hashKeys().arrayListValues().<String, Map<String, Object>>build();

    traverseConfig(rawConfigData, configData, providedData, null);

    return new ExtractedConfigData(configData, providedData);
  }

  public static void buildFlatMap(
    Map<String, Object> origData,
    Map<String, Object> flattenData,
    String keyPath
  ) {

    var keysIter = origData.keySet().iterator();

    while (keysIter.hasNext()) {

      var key = keysIter.next();
      var value = origData.get(key);
      var newKeyPath = (keyPath == null) ? key : keyPath + "." + key;

      if(value instanceof Map) {

        var nextConfigData = (Map<String, Object>) value;

        if(!nextConfigData.containsKey(C5Consts.CONFIG_KEY_PROVIDER)) {

          buildFlatMap(nextConfigData, flattenData, newKeyPath);

          if(nextConfigData.size() == 0) {
            keysIter.remove();
          }

          continue;
        } else {

          nextConfigData.put(C5Consts.CONFIG_KEY_KEYPATH, newKeyPath);
          nextConfigData.put(C5Consts.CONFIG_KEY_KEYNAME, key);

          keysIter.remove();
        }

      } else {

        flattenData.put(newKeyPath, value);
      }
    }
  }

  static void traverseConfig(
    Map<String, Object> rawConfigData,
    Map<String, Object> configData,
    Multimap<String, Map<String, Object>> providedData,
    String keyPath
  ) {

    var keysIter = rawConfigData.keySet().iterator();

    while (keysIter.hasNext()) {

      var key = keysIter.next();
      var value = rawConfigData.get(key);
      var newKeyPath = (keyPath == null) ? key : keyPath + "." + key;

      if(value instanceof Map) {

        var nextConfigData = (Map<String, Object>) value;

        if(!nextConfigData.containsKey(C5Consts.CONFIG_KEY_PROVIDER)) {

          traverseConfig(nextConfigData, configData, providedData, newKeyPath);

          if(nextConfigData.size() == 0) {
            keysIter.remove();
          }

          continue;
        } else {

          nextConfigData.put(C5Consts.CONFIG_KEY_KEYPATH, newKeyPath);
          nextConfigData.put(C5Consts.CONFIG_KEY_KEYNAME, key);

          providedData.put(nextConfigData.get(C5Consts.CONFIG_KEY_PROVIDER).toString(), (Map<String, Object>) value);

          keysIter.remove();
        }

      } else {

        configData.put(newKeyPath, value);
      }
    }
  }

  /**
   * Merged newMap into original
   * @param original
   * @param newMap
   */
  static void deepMerge(Map original, Map newMap) {

    if (original == null || newMap == null) {
      return;
    }

    for (var entry : (Set<Map.Entry>) newMap.entrySet()) {

      var key = entry.getKey();
      var value = entry.getValue();

      // unfortunately, if null-values are allowed,
      // we suffer the performance hit of double-lookup
      if (original.containsKey(key)) {
        var originalValue = original.get(key);

        if (Objects.equal(originalValue, value)) {
          continue;
        }

        if (originalValue instanceof Collection) {
          // this could be relaxed to simply to simply add instead of addAll
          // IF it's not a collection (still addAll if it is),
          // this would be a useful approach, but uncomfortably inconsistent, algebraically
          Preconditions.checkArgument(value instanceof Collection,
            "a non-collection collided with a collection: %s%n\t%s",
            value, originalValue);

          ((Collection) originalValue).addAll((Collection) value);

          continue;
        }

        if (originalValue instanceof Map) {
          Preconditions.checkArgument(value instanceof Map,
            "a non-map collided with a map: %s%n\t%s",
            value, originalValue);

          deepMerge((Map) originalValue, (Map) value);

          continue;
        }

        original.put(key, value);

      } else
        original.put(key, value);
    }
  }
}
