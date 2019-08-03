package com.excsn.c5store.core;

import java.util.HashMap;
import java.util.Map;

class C5DataStore {
  private Map<String, Object> _data = new HashMap<>();

  public <T> T getData(String keyPath) {
    return (T) _data.get(keyPath);
  }

  public void setData(String keyPath, Object value) {
    _data.put(keyPath, value);
  }
}
