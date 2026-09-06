use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use c5store::providers::{C5ValueProvider, C5ValueProviderSchema};
use c5store::value::C5DataValue;
use c5store::{HydrateContext, SetDataFn};

/// Writes the next value of a fixed sequence to every registered key path on each
/// hydrate, repeating the last one once the sequence is exhausted.
pub struct SequenceProvider {
  key_paths: Mutex<Vec<String>>,
  values: Vec<C5DataValue>,
  hydrations: Arc<AtomicUsize>,
}

impl SequenceProvider {
  pub fn new(values: Vec<C5DataValue>) -> Self {
    assert!(!values.is_empty(), "SequenceProvider needs at least one value");
    Self {
      key_paths: Mutex::new(Vec::new()),
      values,
      hydrations: Arc::new(AtomicUsize::new(0)),
    }
  }

  pub fn hydrations(&self) -> Arc<AtomicUsize> {
    self.hydrations.clone()
  }
}

impl C5ValueProvider for SequenceProvider {
  fn register(&mut self, data: &C5DataValue) {
    if let C5DataValue::Map(map) = data {
      if let Ok(schema) = C5ValueProviderSchema::from_map(map) {
        self.key_paths.lock().unwrap().push(schema.value_key_path);
      }
    }
  }

  fn unregister(&mut self, key: &str) {
    self.key_paths.lock().unwrap().retain(|k| k != key);
  }

  fn hydrate(&self, set_data_fn: &SetDataFn, _force: bool, _context: &HydrateContext) {
    let n = self.hydrations.fetch_add(1, Ordering::SeqCst);
    let value = self.values[n.min(self.values.len() - 1)].clone();

    for key_path in self.key_paths.lock().unwrap().iter() {
      HydrateContext::push_value_to_data_store(set_data_fn, key_path, value.clone());
    }
  }
}
