use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use std::error::Error;
use std::time::Duration;

use num_rational::{Rational32, Rational64, BigRational};
use log::{debug, info, warn, error};

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

pub trait StatsRecorder: Send + Sync {

  fn record_counter_increment(&self, tags: HashMap<Box<str>, Box<str>>, name: Box<str>);
  fn record_timer(&self, tags: HashMap<Box<str>, Box<str>>, name: Box<str>, value: Duration);
  fn record_gauge(&self, tags: HashMap<Box<str>, Box<str>>, name: Box<str>, value: GaugeValue);
}

pub struct StatsRecorderStub {}

impl StatsRecorder for StatsRecorderStub {
  fn record_counter_increment(&self, tags: HashMap<Box<str>, Box<str>, RandomState>, name: Box<str>) {}

  fn record_timer(&self, tags: HashMap<Box<str>, Box<str>, RandomState>, name: Box<str>, value: Duration) {}

  fn record_gauge(&self, tags: HashMap<Box<str>, Box<str>, RandomState>, name: Box<str>, value: GaugeValue) {}
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
  R32(Rational32),
  R64(Rational64),
  BRt(BigRational),
}