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
  PemParse(String), // Or from a specific PEM error type

  #[error("Key loading error: {0}")]
  KeyLoad(String), // Could also wrap specific key parsing errors from ecies_25519::KeyParsingError

  #[error("ECIES operation error: {0}")] // More specific for ecies_25519::Error
  EciesOperation(#[from] ecies_25519::Error), // Assuming ecies_25519::Error is an error type

  #[error("ECIES key parsing error: {0}")]
  EciesKeyParse(#[from] ecies_25519::KeyParsingError), // Assuming ecies_25519::KeyParsingError is an error type

  #[error("Base64 decoding error: {0}")]
  Base64Decode(#[from] base64::DecodeError),

  // Corrected based on your input for deserialization errors from serde_yaml2
  #[error("YAML deserialization error: {0}")]
  YamlDeserialize(String),

  // You might still want a more general one for other YAML issues if needed,
  // or if serde_yaml2::to_string() (serialization) yields a different error type.
  // For example, serde_yaml2::Error itself might be what to_string() returns.
  #[error("YAML serialization error: {0}")]
  YamlSerialize(String), // Placeholder for serialization errors; check serde_yaml2::to_string() error type

  #[error("YAML navigation/manipulation error: {0}")]
  YamlNavigation(String),

  #[error("YAML deserialization error (serde): {0}")]
  // Keep this if serde_yaml2 is still used elsewhere for struct deserialization
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
