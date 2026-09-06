//! TOML read and in-place write, over `toml_edit`, which keeps the document
//! it parsed: comments, spacing, key order and quoting all survive an edit
//! because the document is never regenerated from a value model.

use toml_edit::{Array, DocumentMut, InlineTable, Item, Value as TomlValue};

use crate::error::C5CoreError;
use crate::path::PathSegment;
use crate::value::Value;

fn parse(text: &str) -> Result<DocumentMut, C5CoreError> {
  text
    .parse::<DocumentMut>()
    .map_err(|e| C5CoreError::YamlDeserialize(format!("TOML parsing failed: {e}")))
}

pub fn check(text: &str) -> Result<(), C5CoreError> {
  parse(text).map(|_| ())
}

fn step<'a>(item: &'a Item, segment: &PathSegment) -> Option<&'a Item> {
  match segment {
    PathSegment::Key(key) => item.get(*key),
    PathSegment::Index(index) => item.get(*index),
    PathSegment::Query { key, value } => {
      let array = item.as_array_of_tables();
      match array {
        Some(tables) => tables
          .iter()
          .position(|table| table.get(*key).and_then(|v| v.as_str()) == Some(*value))
          .and_then(|position| item.get(position)),
        None => item
          .as_array()?
          .iter()
          .position(|v| v.as_inline_table().and_then(|t| t.get(*key)).and_then(|v| v.as_str()) == Some(*value))
          .and_then(|position| item.get(position)),
      }
    }
  }
}

fn value_of(item: &Item) -> Value {
  if let Some(table) = item.as_table_like() {
    return Value::Map(table.iter().map(|(k, v)| (k.to_owned(), value_of(v))).collect());
  }
  match item.as_value() {
    Some(TomlValue::String(s)) => Value::String(s.value().to_owned()),
    Some(TomlValue::Integer(n)) => Value::Int(*n.value()),
    Some(TomlValue::Float(n)) => Value::Float(*n.value()),
    Some(TomlValue::Boolean(b)) => Value::Bool(*b.value()),
    Some(TomlValue::Datetime(d)) => Value::String(d.value().to_string()),
    Some(TomlValue::Array(array)) => Value::Array(array.iter().map(|v| value_of(&Item::Value(v.clone()))).collect()),
    Some(TomlValue::InlineTable(table)) => {
      Value::Map(table.iter().map(|(k, v)| (k.to_owned(), value_of(&Item::Value(v.clone())))).collect())
    }
    None => Value::Null,
  }
}

pub fn get(text: &str, segments: &[PathSegment]) -> Result<Option<Value>, C5CoreError> {
  let document = parse(text)?;
  let mut item: &Item = document.as_item();
  for segment in segments {
    match step(item, segment) {
      Some(next) => item = next,
      None => return Ok(None),
    }
  }
  Ok(Some(value_of(item)))
}

fn toml_value(value: &Value) -> TomlValue {
  match value {
    Value::Null => TomlValue::from(""),
    Value::Bool(b) => TomlValue::from(*b),
    Value::Int(n) => TomlValue::from(*n),
    Value::Float(n) => TomlValue::from(*n),
    Value::String(s) => TomlValue::from(s.as_str()),
    Value::Array(items) => TomlValue::Array(items.iter().map(toml_value).collect::<Array>()),
    Value::Map(entries) => {
      let mut table = InlineTable::new();
      for (key, item) in entries {
        table.insert(key, toml_value(item));
      }
      TomlValue::InlineTable(table)
    }
  }
}

pub fn set(text: &str, segments: &[PathSegment], value: &Value) -> Result<String, C5CoreError> {
  let mut document = parse(text)?;
  let (last, parents) = segments.split_last().expect("a path with no segments is refused above");

  let mut item: &mut Item = document.as_item_mut();
  for segment in parents {
    let PathSegment::Key(key) = segment else {
      // An index or a query names something that has to exist already.
      let exists = step(item, segment).is_some();
      if !exists {
        return Err(C5CoreError::YamlNavigation(format!(
          "`{}` does not exist, and an index or a query cannot be created.",
          name(segments)
        )));
      }
      item = index_mut(item, segment).expect("just checked it exists");
      continue;
    };
    if item.get(*key).is_none() {
      let table = toml_edit::Table::new();
      item[*key] = Item::Table(table);
    }
    item = &mut item[*key];
  }

  let PathSegment::Key(key) = last else {
    return Err(C5CoreError::YamlNavigation(format!(
      "`{}` ends in an index or a query, which names no key to write.",
      name(segments)
    )));
  };
  item[*key] = Item::Value(toml_value(value));
  Ok(document.to_string())
}

fn index_mut<'a>(item: &'a mut Item, segment: &PathSegment) -> Option<&'a mut Item> {
  match segment {
    PathSegment::Index(index) => item.get_mut(*index),
    PathSegment::Query { key, value } => {
      let position = item
        .as_array_of_tables()
        .and_then(|tables| tables.iter().position(|t| t.get(*key).and_then(|v| v.as_str()) == Some(*value)))?;
      item.get_mut(position)
    }
    PathSegment::Key(key) => item.get_mut(*key),
  }
}

fn name(segments: &[PathSegment]) -> String {
  segments
    .iter()
    .map(|s| match s {
      PathSegment::Key(k) => (*k).to_owned(),
      PathSegment::Index(i) => format!("[{i}]"),
      PathSegment::Query { key, value } => format!("[{key}={value}]"),
    })
    .collect::<Vec<_>>()
    .join(".")
}
