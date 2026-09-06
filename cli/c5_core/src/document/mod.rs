//! A configuration document, edited in place.
//!
//! Reading answers from the format's own parse. Writing replaces the bytes of
//! one value and touches nothing else, so comments, blank lines, key order and
//! indentation all survive an edit. A format is chosen by the file's
//! extension; a document with no extension is YAML, which is what c5store has
//! always assumed.

pub(crate) mod indent;
mod json;
mod toml;
mod yaml;

use std::path::Path;

use crate::error::C5CoreError;
use crate::path::PathSegment;
use crate::value::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
  Yaml,
  Toml,
  Json,
}

impl Format {
  pub fn of(path: &Path) -> Format {
    match path.extension().and_then(|e| e.to_str()) {
      Some("toml") => Format::Toml,
      Some("json") => Format::Json,
      _ => Format::Yaml,
    }
  }

  pub fn name(&self) -> &'static str {
    match self {
      Format::Yaml => "YAML",
      Format::Toml => "TOML",
      Format::Json => "JSON",
    }
  }
}

#[derive(Debug, Clone)]
pub struct Document {
  text: String,
  format: Format,
}

impl Document {
  /// The file's text, parsed only far enough to know it is well formed. A
  /// file that does not exist is an empty document of its extension's format.
  pub fn load(path: &Path) -> Result<Document, C5CoreError> {
    let format = Format::of(path);
    match std::fs::read_to_string(path) {
      Ok(text) => Document::parse(text, format),
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Document::empty(format)),
      Err(e) => Err(C5CoreError::IoWithPath { path: path.to_path_buf(), source: e }),
    }
  }

  pub fn parse(text: impl Into<String>, format: Format) -> Result<Document, C5CoreError> {
    let document = Document { text: text.into(), format };
    document.check()?;
    Ok(document)
  }

  pub fn empty(format: Format) -> Document {
    let text = match format {
      Format::Json => "{}\n".to_owned(),
      _ => String::new(),
    };
    Document { text, format }
  }

  pub fn format(&self) -> Format {
    self.format
  }

  pub fn text(&self) -> &str {
    &self.text
  }

  fn check(&self) -> Result<(), C5CoreError> {
    match self.format {
      Format::Yaml => yaml::check(&self.text),
      Format::Toml => toml::check(&self.text),
      Format::Json => json::check(&self.text),
    }
  }

  /// The value at `segments`, or `None` when the path names nothing.
  pub fn get(&self, segments: &[PathSegment]) -> Result<Option<Value>, C5CoreError> {
    match self.format {
      Format::Yaml => yaml::get(&self.text, segments),
      Format::Toml => toml::get(&self.text, segments),
      Format::Json => json::get(&self.text, segments),
    }
  }

  /// How many leading segments the document actually holds, for an error that
  /// has to say where a path stopped being true.
  pub fn depth_of(&self, segments: &[PathSegment]) -> usize {
    (0..segments.len())
      .take_while(|end| matches!(self.get(&segments[..end + 1]), Ok(Some(_))))
      .count()
  }

  /// Puts `value` at `segments`, creating the maps along the way that do not
  /// exist yet. Every byte outside the value written is left as it was.
  pub fn set(&mut self, segments: &[PathSegment], value: &Value) -> Result<(), C5CoreError> {
    if segments.is_empty() {
      return Err(C5CoreError::InvalidInput("An empty key path names no value to set.".into()));
    }
    self.text = match self.format {
      Format::Yaml => yaml::set(&self.text, segments, value)?,
      Format::Toml => toml::set(&self.text, segments, value)?,
      Format::Json => json::set(&self.text, segments, value)?,
    };
    Ok(())
  }
}
