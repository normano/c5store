package com.excsn.c5store.core;

@FunctionalInterface
public interface ChangeListener {
  void onChange(String notifyKeyPath, String keyPath, Object value);
}
