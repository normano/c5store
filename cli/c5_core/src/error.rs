use std::path::PathBuf;
use thiserror::Error;
use yaml_rust2::ScanError as YamlScanError;

#[derive(Error, Debug)]
pub enum C5CoreError {
  #[error("I/O error: {0}")]
  Io(#[from] std::io::Error),

  #[error("I/O error for path {path:?}: {source}")]
  IoWithPath { path: PathBuf, source: std::io::Error },

  #[error("PEM parsing error: {0}")]
  PemParse(String),

  #[error("Key loading error: {0}")]
  KeyLoad(String),

  #[error("ECIES operation error: {0}")]
  EciesOperation(#[from] ecies_25519::Error),

  #[error("ECIES key parsing error: {0}")]
  EciesKeyParse(#[from] ecies_25519::KeyParsingError),

  #[error("Base64 decoding error: {0}")]
  Base64Decode(#[from] base64::DecodeError),

  #[error("YAML deserialization error: {0}")]
  YamlDeserialize(String),

  #[error("YAML serialization error: {0}")]
  YamlSerialize(String),

  #[error("YAML navigation/manipulation error: {0}")]
  YamlNavigation(String),

  #[error("YAML deserialization error (serde): {0}")]
  SerdeYamlDeserialize(#[from] serde::de::value::Error),

  #[error("YAML parsing error (yaml-rust2): {0}")]
  YamlRust2Parse(#[from] YamlScanError),

  #[error("Unsupported algorithm: {0}")]
  UnsupportedAlgorithm(String),

  #[error("File already exists at path: {0}")]
  FileExists(PathBuf),

  #[error("Encoding/Decoding error for text: {0}")]
  Encoding(String),

  #[error("Invalid input: {0}")]
  InvalidInput(String),
}
