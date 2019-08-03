package com.excsn.c5store.core;

public class C5Store {

  private final GetDataFn _getDataFn;
  private final C5StoreSubscriptions _subscriptions;

  C5Store(GetDataFn getDataFn, C5StoreSubscriptions subscriptions) {
    _getDataFn = getDataFn;
    _subscriptions = subscriptions;
  }

  public <T> T get(String keyPath) {

    return _getDataFn.getData(keyPath);
  }

  public void subscribe(String keyPath, ChangeListener listener) {

    _subscriptions.add(keyPath, listener);
  }
}
