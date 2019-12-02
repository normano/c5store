package com.excsn.c5store.core;

import java.util.Iterator;

public class C5StoreRoot implements C5Store {

  private final GetDataFn _getDataFn;
  private final KeyExistsFn _keyExistsFn;
  private final PrefixKeysFn _prefixKeysFn;
  private final C5StoreSubscriptions _subscriptions;

  C5StoreRoot(
    GetDataFn getDataFn,
    KeyExistsFn keyExistsFn,
    PrefixKeysFn prefixKeysFn,
    C5StoreSubscriptions subscriptions
  ) {
    _getDataFn = getDataFn;
    _keyExistsFn = keyExistsFn;
    _prefixKeysFn = prefixKeysFn;
    _subscriptions = subscriptions;
  }

  public <T> T get(String keyPath) {

    return _getDataFn.getData(keyPath);
  }

  public boolean exists(String keyPath) {

    return _keyExistsFn.exists(keyPath);
  }

  public void subscribe(String keyPath, ChangeListener listener) {

    _subscriptions.add(keyPath, listener);
  }

  @Override
  public C5Store branch(String prefixKeyPath) {
    return new C5StoreBranch(prefixKeyPath);
  }

  @Override
  public String currentKeyPath() {
    return null;
  }

  @Override
  public Iterator<String> keyPathsWithPrefix(String keyPath) {
    return _prefixKeysFn.keysWithPrefix(keyPath);
  }

  public class C5StoreBranch implements C5Store {

    private String _preFixKeyPath;

    private C5StoreBranch(String prefixKeyPath) {
      _preFixKeyPath = prefixKeyPath;
    }

    public <T> T get(String keyPath) {

      return _getDataFn.getData(_preFixKeyPath + "." + keyPath);
    }

    public boolean exists(String keyPath) {

      return _keyExistsFn.exists(_preFixKeyPath + "." + keyPath);
    }

    public void subscribe(String keyPath, ChangeListener listener) {

      _subscriptions.add(keyPath, listener);
    }

    @Override
    public C5Store branch(String prefixKeyPath) {
      return new C5StoreBranch(_preFixKeyPath + "." + prefixKeyPath);
    }

    @Override
    public String currentKeyPath() {
      return _preFixKeyPath;
    }

    @Override
    public Iterator<String> keyPathsWithPrefix(String keyPath) {
      return _prefixKeysFn.keysWithPrefix(_preFixKeyPath + "." +keyPath);
    }
  }
}
