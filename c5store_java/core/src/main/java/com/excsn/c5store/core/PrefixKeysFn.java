package com.excsn.c5store.core;

import java.util.Iterator;

@FunctionalInterface
public interface PrefixKeysFn {
  Iterator<String> keysWithPrefix(String keyPath);
}
