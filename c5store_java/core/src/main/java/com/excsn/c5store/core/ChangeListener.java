package com.excsn.c5store.core;

@FunctionalInterface
interface ChangeListener {
  void onChange(String notifyKeyPath, String keyPath, Object value);
}
