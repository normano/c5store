package com.excsn.c5store.core;

import com.excsn.c5store.core.serializers.C5JSONValueDeserializer;
import com.excsn.c5store.core.serializers.C5ValueDeserializer;
import com.excsn.c5store.core.serializers.C5YAMLValueDeserializer;

import java.io.IOException;
import java.nio.charset.Charset;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.Map;

import static com.excsn.c5store.core.C5StoreUtils.buildFlatMap;

public class C5FileValueProvider implements C5ValueProvider {

  private Map<String, C5FileValueProviderSchema> _keyDataMap;
  private String _fileRootDir;
  private Map<String, C5ValueDeserializer> _deserializers;

  public C5FileValueProvider(String fileRootDir, Map<String, C5ValueDeserializer> deserializers) {
    _keyDataMap = new HashMap<>();
    _fileRootDir = fileRootDir;
    _deserializers = deserializers;
  }

  public static C5FileValueProvider createDefault(String fileRootDir) {

    var deserializers = new HashMap<String, C5ValueDeserializer>();
    deserializers.put("json", C5JSONValueDeserializer.create());
    deserializers.put("yaml", new C5YAMLValueDeserializer());

    return new C5FileValueProvider(fileRootDir, deserializers);
  }

  @Override
  public void register(Map<String, Object> vpData) {

    var schema = new C5FileValueProviderSchema(vpData);
    var keyPath = schema.vKeyPath;
    _keyDataMap.put(keyPath, schema);
  }

  @Override
  public void unregister(String keyPath) {

    _keyDataMap.remove(keyPath);
  }

  @Override
  public void hydrate(SetDataFn setData, boolean force, HydrateContext context) {

    for (var entry : _keyDataMap.entrySet()) {

      var keyPath = entry.getKey();
      var vpData = entry.getValue();
      var filePath = Path.of(vpData.path);

      if (!filePath.isAbsolute()) {
        filePath = Path.of(_fileRootDir, filePath.toString()).toAbsolutePath();
      }

      if (!Files.exists(filePath)) {
        setData.setData(keyPath, null);
        continue;
      }

      String fileContents;
      try {

        fileContents = Files.readString(filePath, Charset.forName(vpData.encoding));
      } catch (IOException e) {
        context.logger.error("Could not read from file '" + filePath.toString() + "'", e);
        continue;
      }

      Object deserializedValue;

      if (!"raw".equals(vpData.format)) {

        if (!_deserializers.containsKey(vpData.format)) {

          context.logger.warn(vpData.vKeyPath + " cannot be deserialized since deserializer " + vpData.format
            + " does not exist");
          continue;
        }

        var deserializer = _deserializers.get(vpData.format);
        deserializedValue = deserializer.deserialize(fileContents);

      } else {
        deserializedValue = fileContents;
      }

      if(deserializedValue instanceof Map) {

        var configDataMap = new HashMap<String, Object>();
        buildFlatMap((Map<String, Object>) deserializedValue, configDataMap, keyPath);

        for(var configEntry : configDataMap.entrySet()) {
          setData.setData(configEntry.getKey(), configEntry.getValue());
        }
      } else {

        setData.setData(keyPath, deserializedValue);
      }
    }
  }

  C5FileValueProviderSchema getSchema(String schemaName) {
    return _keyDataMap.get(schemaName);
  }
}
