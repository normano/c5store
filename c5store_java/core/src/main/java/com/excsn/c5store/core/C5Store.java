package com.excsn.c5store.core;

import java.util.Iterator;

public interface C5Store {
  <T> T get(String keyPath);

  boolean exists(String keyPath);

  void subscribe(String keyPath, ChangeListener listener);

  C5Store branch(String prefixKeyPath);

  /**
   * @return null if root, prefixKey if branch
   */
  String currentKeyPath();

  Iterator<String> keyPathsWithPrefix(String keyPath);
}
