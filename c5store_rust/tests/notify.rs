mod common;

use std::fs;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use c5store::value::C5DataValue;
use c5store::{C5Store, C5StoreOptions, create_c5store};
use common::SequenceProvider;
use tempfile::TempDir;

const DEBOUNCE_MS: u64 = 20;
const RECV_TIMEOUT: Duration = Duration::from_secs(5);

fn store_with_provider_section(section: &str) -> (TempDir, C5StoreOptions) {
  let dir = TempDir::new().unwrap();
  fs::write(dir.path().join("config.yaml"), section).unwrap();

  let mut options = C5StoreOptions::default();
  options.change_delay_period = Some(DEBOUNCE_MS);

  (dir, options)
}

fn drain(rx: &Receiver<(String, String)>) -> Vec<(String, String)> {
  let mut seen = vec![rx.recv_timeout(RECV_TIMEOUT).expect("no notification arrived")];
  while let Ok(next) = rx.recv_timeout(Duration::from_millis(DEBOUNCE_MS * 10)) {
    seen.push(next);
  }
  seen
}

#[test]
fn registering_a_provider_notifies_a_prior_subscriber() {
  let (dir, options) = store_with_provider_section("market:\n  regions:\n    .provider: seq\n");
  let (store, mut mgr) = create_c5store(vec![dir.path().join("config.yaml")], Some(options)).unwrap();

  let (tx, rx) = channel();
  store.subscribe(
    "market.regions",
    Box::new(move |notify_path, changed_key, _value| {
      tx.send((notify_path.to_string(), changed_key.to_string())).unwrap();
    }),
  );

  mgr.set_value_provider(
    "seq",
    SequenceProvider::new(vec![C5DataValue::String("first".into())]),
    0,
  );

  let seen = drain(&rx);
  assert_eq!(seen, vec![("market.regions".to_string(), "market.regions".to_string())]);
}

#[test]
fn an_ancestor_subscription_hears_each_changed_descendant() {
  let (dir, options) = store_with_provider_section(
    "market:\n  east:\n    .provider: seq\n  west:\n    .provider: seq\n",
  );
  let (store, mut mgr) = create_c5store(vec![dir.path().join("config.yaml")], Some(options)).unwrap();

  let (tx, rx) = channel();
  store.subscribe(
    "market",
    Box::new(move |_notify_path, changed_key, _value| {
      tx.send((String::new(), changed_key.to_string())).unwrap();
    }),
  );

  mgr.set_value_provider(
    "seq",
    SequenceProvider::new(vec![C5DataValue::String("filled".into())]),
    0,
  );

  let mut changed: Vec<String> = drain(&rx).into_iter().map(|(_, key)| key).collect();
  changed.sort();
  assert_eq!(changed, vec!["market.east".to_string(), "market.west".to_string()]);
}

#[test]
fn subscribe_detailed_reports_no_previous_value_on_the_first_set() {
  let (dir, options) = store_with_provider_section("market:\n  regions:\n    .provider: seq\n");
  let (store, mut mgr) = create_c5store(vec![dir.path().join("config.yaml")], Some(options)).unwrap();

  let (tx, rx) = channel();
  store.subscribe_detailed(
    "market.regions",
    Box::new(move |_notify_path, _changed_key, new_value, old_value| {
      tx.send((new_value.clone(), old_value.cloned())).unwrap();
    }),
  );

  mgr.set_value_provider(
    "seq",
    SequenceProvider::new(vec![C5DataValue::String("first".into())]),
    0,
  );

  let (new_value, old_value) = rx.recv_timeout(RECV_TIMEOUT).expect("no notification arrived");
  assert_eq!(new_value, C5DataValue::String("first".into()));
  assert_eq!(old_value, None);
}

#[test]
fn a_refresh_reports_the_previous_value() {
  let (dir, options) = store_with_provider_section("market:\n  regions:\n    .provider: seq\n");
  let (store, mut mgr) = create_c5store(vec![dir.path().join("config.yaml")], Some(options)).unwrap();

  let (tx, rx) = channel();
  store.subscribe_detailed(
    "market.regions",
    Box::new(move |_notify_path, _changed_key, new_value, old_value| {
      tx.send((new_value.clone(), old_value.cloned())).unwrap();
    }),
  );

  // The refresh timer is the only path that hydrates a second time, and its period is
  // whole seconds, so this test cannot run faster than one refresh interval.
  mgr.set_value_provider(
    "seq",
    SequenceProvider::new(vec![
      C5DataValue::String("first".into()),
      C5DataValue::String("second".into()),
    ]),
    1,
  );

  let (first_new, first_old) = rx.recv_timeout(RECV_TIMEOUT).expect("no initial notification");
  assert_eq!(first_new, C5DataValue::String("first".into()));
  assert_eq!(first_old, None);

  let (second_new, second_old) = rx.recv_timeout(RECV_TIMEOUT).expect("no refresh notification");
  assert_eq!(second_new, C5DataValue::String("second".into()));
  assert_eq!(second_old, Some(C5DataValue::String("first".into())));
}

#[test]
fn an_unchanged_value_notifies_nothing_on_refresh() {
  let (dir, options) = store_with_provider_section("market:\n  regions:\n    .provider: seq\n");
  let (store, mut mgr) = create_c5store(vec![dir.path().join("config.yaml")], Some(options)).unwrap();

  let (tx, rx) = channel();
  store.subscribe(
    "market.regions",
    Box::new(move |_notify_path, changed_key, _value| {
      tx.send(changed_key.to_string()).unwrap();
    }),
  );

  mgr.set_value_provider(
    "seq",
    SequenceProvider::new(vec![C5DataValue::String("same".into())]),
    1,
  );

  assert_eq!(rx.recv_timeout(RECV_TIMEOUT).unwrap(), "market.regions");
  assert!(
    rx.recv_timeout(Duration::from_millis(1500)).is_err(),
    "a refresh writing the same value should notify nothing"
  );
}

#[test]
fn repeated_changes_inside_the_debounce_window_collapse_into_one() {
  let dir = TempDir::new().unwrap();
  fs::write(
    dir.path().join("config.yaml"),
    "market:\n  regions:\n    .provider: seq\n",
  )
  .unwrap();

  let mut options = C5StoreOptions::default();
  options.change_delay_period = Some(2500);

  let (store, mut mgr) = create_c5store(vec![dir.path().join("config.yaml")], Some(options)).unwrap();

  let (tx, rx) = channel();
  store.subscribe_detailed(
    "market.regions",
    Box::new(move |_notify_path, _changed_key, new_value, old_value| {
      tx.send((new_value.clone(), old_value.cloned())).unwrap();
    }),
  );

  let provider = SequenceProvider::new(vec![
    C5DataValue::String("first".into()),
    C5DataValue::String("second".into()),
    C5DataValue::String("third".into()),
  ]);
  let hydrations = provider.hydrations();

  mgr.set_value_provider("seq", provider, 1);

  let (new_value, old_value) = rx.recv_timeout(RECV_TIMEOUT).expect("no notification arrived");

  assert!(
    hydrations.load(std::sync::atomic::Ordering::SeqCst) >= 3,
    "expected the refresh timer to have run at least twice inside the debounce window"
  );
  assert_eq!(new_value, C5DataValue::String("third".into()));
  assert_eq!(old_value, Some(C5DataValue::String("second".into())));
  assert!(
    rx.recv_timeout(Duration::from_millis(500)).is_err(),
    "the collapsed changes should produce exactly one notification"
  );
}
