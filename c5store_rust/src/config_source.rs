use std::{fmt, path::PathBuf};

/// Represents the origin of a configuration value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigSource {
  File(PathBuf),              // Source file path
  EnvironmentVariable(String), // Name of the environment variable (e.g., "C5_DB__HOST")
  Provider(String),           // Name of the provider
  SetProgrammatically,        // Value set via a direct API call (future)
  Unknown,                    // Default or fallback
}

impl fmt::Display for ConfigSource {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      ConfigSource::File(path) => write!(f, "File({:?})", path),
      ConfigSource::EnvironmentVariable(name) => write!(f, "EnvVar({})", name),
      ConfigSource::Provider(name) => write!(f, "Provider({})", name),
      ConfigSource::SetProgrammatically => write!(f, "SetProgrammatically"),
      ConfigSource::Unknown => write!(f, "Unknown"),
    }
  }
}