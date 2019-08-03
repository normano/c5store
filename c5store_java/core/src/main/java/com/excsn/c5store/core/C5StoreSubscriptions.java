package com.excsn.c5store.core;

import com.google.common.collect.Multimap;
import com.google.common.collect.MultimapBuilder;

import java.util.Collection;

class C5StoreSubscriptions {

  private Multimap<String, ChangeListener> _changeListeners;

  public C5StoreSubscriptions() {
    _changeListeners = MultimapBuilder.hashKeys().arrayListValues().build();
  }

  public void add(String keyPath, ChangeListener listener) {
    _changeListeners.put(keyPath, listener);
  }

  public Collection<ChangeListener> getSubscribers(String keyPath) {

    return _changeListeners.get(keyPath);
  }

  public void notifyValueChange(String notifyKeyPath, String keyPath, Object value) {

    var subscribers = _changeListeners.get(notifyKeyPath);

    if (subscribers == null || subscribers.isEmpty()) {
      return;
    }

    for (var listener : subscribers) {

      listener.onChange(notifyKeyPath, keyPath, value);
    }
  }
}
