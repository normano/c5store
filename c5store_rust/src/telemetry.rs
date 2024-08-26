use std::hash::Hash;
use std::collections::HashMap;
use std::error::Error;
use std::time::Duration;

use log::{debug, error, info, warn};
use num_rational::Rational32;

pub trait Logger: Send + Sync {

  fn debug(&self, message: &str);
  fn info(&self, message: &str);
  fn warn(&self, message: &str);
  fn error(&self, message: &str, backtrace: Option<&dyn Error>);
}

pub struct ConsoleLogger {}

impl Logger for ConsoleLogger {
  fn debug(&self, message: &str) {
    debug!("{}", message);
  }

  fn info(&self, message: &str) {
    info!("{}", message);
  }

  fn warn(&self, message: &str) {
    warn!("{}", message);
  }

  fn error(&self, message: &str, _error: Option<&dyn Error>)
  {
    error!("{}", message);
  }
}

type StatsTags = HashMap<String, TagValue>;

pub enum TagValue {
  String(String),
  TypedBytes(String, Vec<u8>), // (Data type, bytes)
}

pub trait StatsRecorder: Send + Sync {

  fn record_counter_increment(&self, tags: StatsTags, name: String);
  fn record_timer(&self, tags: StatsTags, name: String, value: Duration);
  fn record_gauge(&self, tags: StatsTags, name: String, value: GaugeValue);
}

pub struct StatsRecorderStub {}

impl StatsRecorder for StatsRecorderStub {
  fn record_counter_increment(&self, _tags: StatsTags, _name: String) {}

  fn record_timer(&self, _tags: StatsTags, _name: String, _value: Duration) {}

  fn record_gauge(&self, _tags: StatsTags, _name: String, _value: GaugeValue) {}
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone)]
pub enum GaugeValue {
  Int8(i8),
  UInt8(u8),
  Int16 (i16),
  UInt16(u16),
  Int32(i32),
  UInt32(u32),
  Int64(i64),
  UInt64(u64),
  Int128(i128),
  UInt128(u128),
  Ratio32(Rational32),
}