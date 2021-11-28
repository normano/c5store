package com.excsn.c5store.core;

@FunctionalInterface
public interface SetDataFn {
  void setData(String keyPath, Object value);
}
