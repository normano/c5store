package com.excsn.c5store.core;

@FunctionalInterface
public interface GetDataFn {
  <T> T getData(String keyPath);
}
