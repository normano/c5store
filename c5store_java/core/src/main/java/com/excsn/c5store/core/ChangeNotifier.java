package com.excsn.c5store.core;

import com.google.common.collect.MultimapBuilder;

import java.util.HashSet;
import java.util.Set;
import java.util.concurrent.TimeUnit;

class ChangeNotifier {

  private int _changeDelayPeriod;
  private Debouncer _debouncer;
  private Set<String> _changeKeyPaths;
  private C5DataStore _c5DataStore;
  private C5StoreSubscriptions _subscriptions;

  ChangeNotifier(C5DataStore c5DataStore, C5StoreSubscriptions subscriptions, int changeDelayPeriod) {
    _debouncer = new Debouncer();
    _changeKeyPaths = new HashSet<>();
    _c5DataStore = c5DataStore;
    _subscriptions = subscriptions;
    _changeDelayPeriod = changeDelayPeriod;
  }

  void changeNotify(String key) {

    _changeKeyPaths.add(key);

    _debouncer.debounce(Void.class, this::doChangeNotify, _changeDelayPeriod, TimeUnit.MILLISECONDS);
  }

  private void doChangeNotify() {

    var savedChangedKeyPaths = _changeKeyPaths;
    _changeKeyPaths = new HashSet<>();

    var dedupedSavedChangedKeyPathsMap = MultimapBuilder.SetMultimapBuilder.hashKeys().hashSetValues()
      .<String, String>build();

    for (var savedChangedKeyPath : savedChangedKeyPaths) {

      dedupedSavedChangedKeyPathsMap.put(savedChangedKeyPath, savedChangedKeyPath);

      var splitSavedChangedKeyPath = savedChangedKeyPath.split(".");
      StringBuilder keyAncestorPath = new StringBuilder();

      for (var savedChangedKeyPathPart : splitSavedChangedKeyPath) {

        if (!keyAncestorPath.toString().isBlank()) {
          keyAncestorPath.append(".");
        }

        keyAncestorPath.append(savedChangedKeyPathPart);

        dedupedSavedChangedKeyPathsMap.put(savedChangedKeyPath, keyAncestorPath.toString());
      }
    }

    for (var savedChangedKeyPath : dedupedSavedChangedKeyPathsMap.keys()) {

      var dedupedSavedChangedKeyPaths = dedupedSavedChangedKeyPathsMap.get(savedChangedKeyPath);

      var value = _c5DataStore.getData(savedChangedKeyPath);

      for (var dedupedSavedChangedKeyPath : dedupedSavedChangedKeyPaths) {

        _subscriptions.notifyValueChange(dedupedSavedChangedKeyPath, savedChangedKeyPath, value);
      }
    }
  }

  void stop() {
    _debouncer.shutdown();
  }
}
