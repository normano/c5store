use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::{IntoIter, Keys, RandomState};
use std::collections::hash_map::Iter;
use std::collections::hash_map::IterMut;
use std::fmt::{self, Debug};
use std::hash::{BuildHasher, Hash};
use std::iter::{FromIterator, IntoIterator, Iterator};

#[macro_export]
///
/// Create a `HashsetMultiMap` from a list of key value pairs
///
macro_rules! hashsetmultimap {
  ($($key:expr => $value:expr),*)=>{
    {
      let mut _map = HashsetMultiMap::new();
      $(
          map.insert($key,$value);
        )*
      _map
    }
  }
}

#[derive(Clone)]
pub struct HashsetMultiMap<K, V, S = RandomState> {
  _inner_map: HashMap<K, HashSet<V>, S>,
}

impl<K, V> HashsetMultiMap<K, V>
where K: Eq + Hash,
      V: Eq + Hash
{
  ///
  /// Creates an empty HashsetMultiMap
  ///
  pub fn new() -> HashsetMultiMap<K, V> {
    HashsetMultiMap { _inner_map: HashMap::new() }
  }

  ///
  /// Creates HashsetMultiMap with the given initial capacity
  ///
  pub fn with_capacity(capacity: usize) -> HashsetMultiMap<K, V> {
    HashsetMultiMap { _inner_map: HashMap::with_capacity(capacity) }
  }
}

impl<K, V, S> HashsetMultiMap<K, V, S>
where K: Eq + Hash,
      V: Eq + Hash,
      S: BuildHasher,
{
  ///
  /// Creates HashsetMultiMap with hasher
  ///
  pub fn with_hasher(hash_builder: S) -> HashsetMultiMap<K, V, S> {
    HashsetMultiMap {
      _inner_map: HashMap::with_hasher(hash_builder)
    }
  }

  ///
  /// Creates an empty HashsetMultiMap wit capacity and hasher.
  ///
  pub fn with_capacity_and_hasher(capacity: usize, hash_builder: S) -> HashsetMultiMap<K, V, S> {
    HashsetMultiMap {
      _inner_map: HashMap::with_capacity_and_hasher(capacity, hash_builder)
    }
  }

  ///
  /// Inserts key value pair into the HashsetMultiMap.
  ///
  pub fn insert(&mut self, key: K, value: V) {

    match self._inner_map.get_mut(&key) {
      Some(set) => {
        set.insert(value);
      },
      None => {
        let mut hashset = HashSet::new();
        hashset.insert(value);
        self._inner_map.insert(key, hashset);
      }
    };
  }

  ///
  /// Returns true if the map contains a value for the specified key.
  ///
  pub fn contains_key<Q: ?Sized>(&self, k: &Q) -> bool
  where K: Borrow<Q>,
        Q: Eq + Hash
  {
    self._inner_map.contains_key(k)
  }

  ///
  /// Returns the number of elements in the map.
  ///
  pub fn len(&self) -> usize {
    self._inner_map.len()
  }

  ///
  /// Removes a key from the map, returning the hash of values if available
  ///
  pub fn remove<Q: ?Sized>(&mut self, k: &Q) -> Option<HashSet<V>>
  where K: Borrow<Q>,
        Q: Eq + Hash
  {
    self._inner_map.remove(k)
  }

  ///
  /// Returns a reference to the hashset at key if available
  ///
  pub fn get<Q: ?Sized>(&self, k: &Q) -> Option<&HashSet<V>>
  where K: Borrow<Q>,
        Q: Eq + Hash
  {
    self._inner_map.get(k)
  }

  ///
  /// Returns a mutable reference to the hashset at key if available
  ///
  pub fn get_mut<Q: ?Sized>(&mut self, k: &Q) -> Option<&mut HashSet<V>>
  where K: Borrow<Q>,
        Q: Eq + Hash
  {
    self._inner_map.get_mut(k)
  }

  ///
  /// Returns number of elements the map can hold
  ///
  pub fn capacity(&self) -> usize {
    self._inner_map.capacity()
  }

  ///
  /// Returns true if the map contains no elements
  ///
  pub fn is_empty(&self) -> bool {
    self._inner_map.is_empty()
  }

  ///
  /// Remove all key-value pairs
  /// Does not reset capacity
  ///
  pub fn clear(&mut self) {
    self._inner_map.clear();
  }

  ///
  /// Iterator of all keys
  ///
  pub fn keys(&self) -> Keys<K, HashSet<V>> {
    self._inner_map.keys()
  }

  /// An iterator visiting all key-value pairs in arbitrary order. The iterator returns
  /// a reference to the key and the corresponding key's vector.
  pub fn iter(&self) -> Iter<K, HashSet<V>> {
    self._inner_map.iter()
  }

  /// An iterator visiting all key-value pairs in arbitrary order. The iterator returns
  /// a reference to the key and the corresponding key's vector.
  pub fn iter_mut(&mut self) -> IterMut<K, HashSet<V>> {
    self._inner_map.iter_mut()
  }

  ///
  /// Retains only the elements specified by the predicate.
  ///
  /// In other words, remove all pairs `(k, v)` such that `f(&k,&mut v)` returns `false`.
  ///
  pub fn retain<F>(&mut self, mut f: F)
  where F: FnMut(&K, &V) -> bool
  {
    for (key, vector) in &mut self._inner_map {
      vector.retain(|ref value| f(key, value));
    }
    self._inner_map.retain(|&_, ref v| !v.is_empty());
  }
}

impl<K, V, S> Debug for HashsetMultiMap<K, V, S>
where K: Eq + Hash + Debug,
      V: Eq + Hash + Debug,
      S: BuildHasher
{
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {

    let items: Vec<(&K, &HashSet<V>)> = self.iter().map(|(key, value)| (key, value)).collect();
    let mut debug_map = f.debug_map();

    for item in items {
      debug_map.entry(&item.0, &item.1);
    }

    return debug_map.finish();
  }
}

impl<K, V, S> PartialEq for HashsetMultiMap<K, V, S>
where K: Eq + Hash,
      V: PartialEq + Eq + Hash,
      S: BuildHasher
{
  fn eq(&self, other: &HashsetMultiMap<K, V, S>) -> bool {
    if self.len() != other.len() {
      return false;
    }

    self.iter().all(|(key, value)| other.get(key).map_or(false, |v| value == v))
  }
}

impl<K, V, S> Eq for HashsetMultiMap<K, V, S>
where K: Eq + Hash,
      V: Eq + Hash,
      S: BuildHasher
{
}

impl<K, V, S> Default for HashsetMultiMap<K, V, S>
where K: Eq + Hash,
      V: Eq + Hash,
      S: BuildHasher + Default
{
  fn default() -> HashsetMultiMap<K, V, S> {
    HashsetMultiMap { _inner_map: Default::default() }
  }
}

impl<K, V, S> FromIterator<(K, V)> for HashsetMultiMap<K, V, S>
where K: Eq + Hash,
      V: Eq + Hash,
      S: BuildHasher + Default
{
  fn from_iter<T: IntoIterator<Item = (K, V)>>(iterable: T) -> HashsetMultiMap<K, V, S> {
    let iter = iterable.into_iter();
    let hint = iter.size_hint().0;

    let mut map = HashsetMultiMap::with_capacity_and_hasher(hint, S::default());
    for (k, v) in iter {
      map.insert(k, v);
    }

    map
  }
}

impl<'a, K, V, S> IntoIterator for &'a HashsetMultiMap<K, V, S>
where K: Eq + Hash,
      V: Eq + Hash,
      S: BuildHasher
{
  type Item = (&'a K, &'a HashSet<V>);
  type IntoIter = Iter<'a, K, HashSet<V>>;

  fn into_iter(self) -> Iter<'a, K, HashSet<V>> {
    self.iter()
  }
}

impl<'a, K, V, S> IntoIterator for &'a mut HashsetMultiMap<K, V, S>
where K: Eq + Hash,
      V: Eq + Hash,
      S: BuildHasher
{
  type Item = (&'a K, &'a mut HashSet<V>);
  type IntoIter = IterMut<'a, K, HashSet<V>>;

  fn into_iter(self) -> Self::IntoIter {
    self._inner_map.iter_mut()
  }
}

impl<K, V, S> IntoIterator for HashsetMultiMap<K, V, S>
where K: Eq + Hash,
      V: Eq + Hash,
      S: BuildHasher
{
  type Item = (K, HashSet<V>);
  type IntoIter = IntoIter<K, HashSet<V>>;

  fn into_iter(self) -> Self::IntoIter {
    self._inner_map.into_iter()
  }
}

impl<K, V, S> Extend<(K, V)> for HashsetMultiMap<K, V, S>
where K: Eq + Hash,
      V: Eq + Hash,
      S: BuildHasher
{
  fn extend<T: IntoIterator<Item = (K, V)>>(&mut self, iter: T) {
    for (k, v) in iter {
      self.insert(k, v);
    }
  }
}

impl<'a, K, V, S> Extend<(&'a K, &'a V)> for HashsetMultiMap<K, V, S>
where K: Eq + Hash + Copy,
      V: Eq + Hash + Copy,
      S: BuildHasher
{
  fn extend<T: IntoIterator<Item = (&'a K, &'a V)>>(&mut self, iter: T) {
    self.extend(iter.into_iter().map(|(&key, &value)| (key, value)));
  }
}