use std::{hash::Hash};
use std::collections::HashMap;
use std::error::Error;
use std::time::Duration;

use log::{debug, error, info, warn};
use num_rational::{BigRational, Rational32, Rational64};

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

type StatsTags = HashMap<Box<str>, TagValue>;

pub enum TagValue {
  String(Box<str>),
  TypedBytes(Box<str>, Vec<u8>), // (Data type, bytes)
}

pub trait StatsRecorder: Send + Sync {

  fn record_counter_increment(&self, tags: StatsTags, name: Box<str>);
  fn record_timer(&self, tags: StatsTags, name: Box<str>, value: Duration);
  fn record_gauge(&self, tags: StatsTags, name: Box<str>, value: GaugeValue);
}

pub struct StatsRecorderStub {}

impl StatsRecorder for StatsRecorderStub {
  fn record_counter_increment(&self, _tags: StatsTags, _name: Box<str>) {}

  fn record_timer(&self, _tags: StatsTags, _name: Box<str>, _value: Duration) {}

  fn record_gauge(&self, _tags: StatsTags, _name: Box<str>, _value: GaugeValue) {}
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