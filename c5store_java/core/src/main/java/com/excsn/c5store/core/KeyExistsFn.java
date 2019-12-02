package com.excsn.c5store.core;

@FunctionalInterface
public interface KeyExistsFn {
  boolean exists(String keyPath);
}
