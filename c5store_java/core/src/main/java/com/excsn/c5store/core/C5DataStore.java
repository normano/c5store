package com.excsn.c5store.core;

import se.sawano.java.text.AlphanumericComparator;

import java.util.Iterator;
import java.util.NavigableMap;
import java.util.concurrent.ConcurrentSkipListMap;

class C5DataStore {

  private NavigableMap<String, Object> _data = new ConcurrentSkipListMap<>(new AlphanumericComparator());

  public <T> T getData(String keyPath) {
    return (T) _data.get(keyPath);
  }

  public void setData(String keyPath, Object value) {
    _data.put(keyPath, value);
  }

  public boolean exists(String keyPath) {
    return _data.containsKey(keyPath);
  }

  /**
   * This queries the datastore for "{prefixKeyPath}.", so if you had "{prefixKeyPath}.somekey" and "{prefixKeyPath}s"
   * then only "{prefixKeyPath}.somekey" will be returned.
   *
   * Special case is when keyPath is null, then all keys are returned.
   *
   * @return Iterator of keys with "{prefixKeyPath}." or of all keys if keyPath is null
   */
  public Iterator<String> keysWithPrefix(String keyPath) {

    if (keyPath == null) {
      return _data.navigableKeySet().iterator();
    }

    var prefixMap = _data.tailMap(keyPath, true);

    var prefixMapKeySetIterator = prefixMap.keySet().iterator();

    return new Iterator<>() {
      String _nextString;

      @Override
      public boolean hasNext() {
        if(_nextString != null) {
          return true;
        }

        if(!prefixMapKeySetIterator.hasNext()) {
          return false;
        }

        var candidate = prefixMapKeySetIterator.next();

        if(!candidate.startsWith(keyPath + ".")) {
          return false;
        }

        _nextString = candidate;
        return true;
      }

      @Override
      public String next() {
        var nextString = _nextString;
        _nextString = null;
        this.hasNext();

        return nextString;
      }
    };
  }
}
