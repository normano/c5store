use std::path::PathBuf;
use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
  #[error("Key not found: {0}")]
  KeyNotFound(String),

  #[error("Type mismatch for key '{key}': expected {expected_type}, found {found_type}")]
  TypeMismatch {
    key: String,
    expected_type: &'static str,
    found_type: &'static str, // Or String representation of C5DataValue type
  },
  
  #[error("Conversion error for key '{key}': {message}")]
  ConversionError{
    key: String,
    message: String,
  },

  #[error("Failed to deserialize into target struct for key '{key}': {source}")]
  DeserializationError {
    key: String,
    #[source]
    source: serde_json::Error, // Using serde_json as intermediate
  },

  #[error("Environment variable parsing error for key '{key}': {message}")]
  EnvVarError {
    key: String,
    message: String,
  },

  #[error("IO error accessing path {path:?}: {source}")]
  IoError {
    path: PathBuf,
    #[source]
    source: io::Error,
  },
  #[error("Failed to parse YAML file {path:?}: {source}")]
  YamlParseError {
     path: PathBuf,
     #[source]
     source: serde_yaml::Error,
  },

  #[cfg(feature = "toml")]
  #[error("Failed to parse TOML file {path:?}: {source}")]
  TomlParseError {
     path: PathBuf,
     #[source]
     source: toml::de::Error,
  },
  #[cfg(feature = "dotenv")]
  #[error("Failed to load .env file {path:?}: {source}")]
  DotEnvLoadError {
      path: PathBuf,
      #[source]
      source: dotenvy::Error,
  },

  #[cfg(feature = "secrets")]
  #[error("Secret key '{key_name}' specified in config path '{config_path}' not found in store")]
  SecretKeyNotFound {
      key_name: String,
      config_path: String,
  },
  #[cfg(feature = "secrets")]
  #[error("Secret algorithm '{algo_name}' specified in config path '{config_path}' not found in store")]
  SecretAlgorithmNotFound {
      algo_name: String,
      config_path: String,
  },
  #[cfg(feature = "secrets")]
  #[error("Decryption failed for config path '{config_path}': {message}")]
  DecryptionError {
      config_path: String,
      message: String, // Or source error from decryptor
  },
   #[cfg(feature = "secrets")]
   #[error("Invalid secret configuration at path '{config_path}': {message}")]
   InvalidSecretConfig {
       config_path: String,
       message: String,
   },

  #[error("Configuration Error: {0}")]
  Message(String),

  // Add other potential errors here (IO, secrets, etc.) later
  #[error("Internal error: {0}")]
  Internal(String),
}