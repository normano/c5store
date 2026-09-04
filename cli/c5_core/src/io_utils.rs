use crate::error::C5CoreError;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use std::fs;
use std::path::Path;

pub fn bytes_to_base64_string(data: &[u8]) -> String {
  BASE64_STANDARD.encode(data)
}

pub fn base64_string_to_bytes(s: &str) -> Result<Vec<u8>, C5CoreError> {
  BASE64_STANDARD.decode(s).map_err(C5CoreError::from)
}

pub fn read_file_to_bytes(file_path: &Path) -> Result<Vec<u8>, C5CoreError> {
  fs::read(file_path).map_err(|e| C5CoreError::IoWithPath {
    path: file_path.to_path_buf(),
    source: e,
  })
}

pub fn read_file_to_string(file_path: &Path, encoding_name: &str) -> Result<String, C5CoreError> {
  let encoding_name_lower = encoding_name.to_lowercase();
  if encoding_name_lower == "utf-8" || encoding_name_lower == "utf8" {
    fs::read_to_string(file_path).map_err(|e| C5CoreError::IoWithPath {
      path: file_path.to_path_buf(),
      source: e,
    })
  } else {
    // For now, only strictly support UTF-8 for reading to string.
    Err(C5CoreError::UnsupportedAlgorithm(format!(
      // Reusing UnsupportedAlgorithm for encoding
      "Encoding '{}' not directly supported for reading to string. Please use UTF-8 or read as bytes.",
      encoding_name
    )))
  }
}

pub fn write_bytes_to_file(file_path: &Path, data: &[u8], force_overwrite: bool) -> Result<(), C5CoreError> {
  if file_path.exists() && !force_overwrite {
    return Err(C5CoreError::FileExists(file_path.to_path_buf()));
  }
  fs::write(file_path, data).map_err(|e| C5CoreError::IoWithPath {
    path: file_path.to_path_buf(),
    source: e,
  })
}

pub fn write_string_to_file(
  file_path: &Path,
  content: &str,
  force_overwrite: bool,
) -> Result<(), C5CoreError> {
  if file_path.exists() && !force_overwrite {
    return Err(C5CoreError::FileExists(file_path.to_path_buf()));
  }
  fs::write(file_path, content).map_err(|e| C5CoreError::IoWithPath {
    path: file_path.to_path_buf(),
    source: e,
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use serial_test::serial;
  use tempfile::NamedTempFile;

  #[test]
  fn test_base64_round_trip() {
    let original = b"hello world! 123 $%^";
    let encoded = bytes_to_base64_string(original);
    let decoded = base64_string_to_bytes(&encoded).unwrap();
    assert_eq!(original.as_slice(), decoded.as_slice());
  }

  #[test]
  fn test_base64_invalid_decode() {
    let invalid_b64 = "not_base64===";
    assert!(base64_string_to_bytes(invalid_b64).is_err());
  }

  #[test]
  #[serial]
  fn test_read_write_bytes_to_file() -> Result<(), C5CoreError> {
    let temp_file = NamedTempFile::new().map_err(C5CoreError::Io)?;
    let file_path = temp_file.path();

    let original_data = vec![1, 2, 3, 4, 5, 255, 0];
    write_bytes_to_file(file_path, &original_data, true)?;

    let read_data = read_file_to_bytes(file_path)?;
    assert_eq!(original_data, read_data);

    let new_data = vec![10, 20, 30];
    write_bytes_to_file(file_path, &new_data, true)?;
    let read_new_data = read_file_to_bytes(file_path)?;
    assert_eq!(new_data, read_new_data);

    assert!(matches!(
      write_bytes_to_file(file_path, &original_data, false),
      Err(C5CoreError::FileExists(_))
    ));

    Ok(())
  }

  #[test]
  #[serial]
  fn test_read_write_string_to_file_utf8() -> Result<(), C5CoreError> {
    let temp_file = NamedTempFile::new().map_err(C5CoreError::Io)?;
    let file_path = temp_file.path();

    let original_string = "Hello, UTF-8 string with emojis 😊 and ñ!".to_string();
    write_string_to_file(file_path, &original_string, true)?;

    let read_string = read_file_to_string(file_path, "utf-8")?;
    assert_eq!(original_string, read_string);

    assert!(matches!(
      read_file_to_string(file_path, "latin1"),
      Err(C5CoreError::UnsupportedAlgorithm(_))
    ));

    let new_string = "New string content".to_string();
    write_string_to_file(file_path, &new_string, true)?;
    let read_new_string = read_file_to_string(file_path, "UTF-8")?; // Case insensitive
    assert_eq!(new_string, read_new_string);

    Ok(())
  }

  #[test]
  #[serial]
  fn test_read_non_existent_file() {
    let non_existent_path = Path::new("hopefully_this_file_does_not_exist_for_test.txt");
    assert!(matches!(
      read_file_to_bytes(non_existent_path),
      Err(C5CoreError::IoWithPath { .. })
    ));
    assert!(matches!(
      read_file_to_string(non_existent_path, "utf-8"),
      Err(C5CoreError::IoWithPath { .. })
    ));
  }
}
