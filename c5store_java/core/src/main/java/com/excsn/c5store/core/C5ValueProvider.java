package com.excsn.c5store.core;

import java.util.Map;

public interface C5ValueProvider {

  /**
   * Registers key path to be watched and refreshed
   * @param vpData data for value provider that follows the schema from value provider
   */
  void register(Map<String, Object> vpData);

  /**
   *
   * @param keyPath Key path in the value provider
   */
  void unregister(String keyPath);

  /**
   * Fetch data and push into data store
   * @param force Forces changed and unchanged data to be refreshed
   */
  void hydrate(SetDataFn setData, boolean force, HydrateContext context);
}
