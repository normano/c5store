package com.excsn.c5store.core;

import com.excsn.c5store.core.telemetry.Logger;

import java.util.HashMap;
import java.util.Map;

import static com.excsn.c5store.core.C5StoreUtils.buildFlatMap;

public class HydrateContext {

  public final Logger logger;

  public HydrateContext(Logger logger) {
    this.logger = logger;
  }

  public static void pushValueToDataStore(SetDataFn setData, String keyPath, Object deserializedValue) {
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
