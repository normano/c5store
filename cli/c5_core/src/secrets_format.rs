use crate::error::C5CoreError;
use crate::keys::CryptoAlgorithm; // Assuming this enum is in c5_core::keys
use std::path::Path;
use yaml_rust2::Yaml;

/// Represents the parts of a c5store secret array.
#[derive(Debug, PartialEq, Eq)]
pub struct C5SecretValueParts {
  pub algo_str: String, // Algorithm as a string
  pub key_name: String,
  pub b64_ciphertext: String,
}

/// Derives a key name from a public key filename.
/// E.g., "my_service.prod.pub.pem" -> "my_service.prod"
/// E.g., "mykey.pem" -> "mykey"
/// E.g., "mykey" -> "mykey"
fn derive_key_name_from_filename(public_key_file_name: &str) -> String {
  let path = Path::new(public_key_file_name);
  let stem = path
    .file_stem()
    .and_then(|s| s.to_str())
    .unwrap_or(public_key_file_name);

  // If it ends with ".pub", remove that common suffix for public keys
  if let Some(stripped) = stem.strip_suffix(".pub") {
    stripped.to_string()
  } else {
    stem.to_string()
  }
}

/// Formats the necessary components into the c5store secret array structure
/// (a `yaml_rust2::Yaml::Array`).
pub fn format_c5_secret_array(
  algo: CryptoAlgorithm,
  public_key_file_name: &str,
  b64_ciphertext: String,
) -> Result<Yaml, C5CoreError> {
  // <<<< Return yaml_rust2::Yaml
  let algo_str = match algo {
    CryptoAlgorithm::EciesX25519 => "ecies_x25519".to_string(),
  };

  let key_name = derive_key_name_from_filename(public_key_file_name);

  let secret_array_vec = vec![
    Yaml::String(algo_str),
    Yaml::String(key_name),
    Yaml::String(b64_ciphertext),
  ];
  Ok(Yaml::Array(secret_array_vec)) // Construct Yaml::Array
}

/// Parses a `yaml_rust2::Yaml` (expected to be a c5store secret array)
/// into its constituent parts.
pub fn parse_c5_secret_array(
  secret_yaml_value: &Yaml, // <<<< Accept yaml_rust2::Yaml
) -> Result<C5SecretValueParts, C5CoreError> {
  match secret_yaml_value {
    Yaml::Array(seq) => {
      // Match on Yaml::Array
      if seq.len() == 3 {
        let algo_str = seq[0].as_str().ok_or_else(|| {
          C5CoreError::YamlNavigation("First element of secret array (algorithm) is not a string.".to_string())
        })?;
        let key_name = seq[1].as_str().ok_or_else(|| {
          C5CoreError::YamlNavigation("Second element of secret array (key name) is not a string.".to_string())
        })?;
        let b64_ciphertext = seq[2].as_str().ok_or_else(|| {
          C5CoreError::YamlNavigation("Third element of secret array (ciphertext) is not a string.".to_string())
        })?;

        Ok(C5SecretValueParts {
          algo_str: algo_str.to_string(),
          key_name: key_name.to_string(),
          b64_ciphertext: b64_ciphertext.to_string(),
        })
      } else {
        Err(C5CoreError::YamlNavigation(format!(
          "Secret array has incorrect length. Expected 3, got {}.",
          seq.len()
        )))
      }
    }
    _ => Err(C5CoreError::YamlNavigation(
      // Match any other Yaml variant
      "Expected secret value to be a YAML Array.".to_string(),
    )),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::keys::CryptoAlgorithm;

  // ... (derive_key_name_from_filename test remains the same) ...
  #[test]
  fn test_derive_key_name() {
    assert_eq!(derive_key_name_from_filename("mykey.pub.pem"), "mykey");
    assert_eq!(derive_key_name_from_filename("mykey.pem"), "mykey");
    assert_eq!(derive_key_name_from_filename("mykey"), "mykey");
    assert_eq!(derive_key_name_from_filename("complex.name.pub.pem"), "complex.name");
    assert_eq!(derive_key_name_from_filename("nodotpub.pem"), "nodotpub");
  }

  #[test]
  fn test_format_and_parse_secret_array() {
    let algo = CryptoAlgorithm::EciesX25519;
    let pk_filename = "service.prod.pub.pem";
    let ciphertext = "someBase64String==".to_string();

    let formatted_value = format_c5_secret_array(algo, pk_filename, ciphertext.clone()).unwrap();

    assert!(matches!(formatted_value, Yaml::Array(_))); // Check it's a Yaml::Array
    if let Yaml::Array(seq) = formatted_value {
      // Destructure to get the Vec<Yaml>
      assert_eq!(seq.len(), 3);
      assert_eq!(seq[0].as_str().unwrap(), "ecies_x25519");
      assert_eq!(seq[1].as_str().unwrap(), "service.prod");
      assert_eq!(seq[2].as_str().unwrap(), &ciphertext); // Compare with &str

      let parsed_parts = parse_c5_secret_array(&Yaml::Array(seq)).unwrap(); // Re-wrap for parsing
      assert_eq!(
        parsed_parts,
        C5SecretValueParts {
          algo_str: "ecies_x25519".to_string(),
          key_name: "service.prod".to_string(),
          b64_ciphertext: ciphertext,
        }
      );
    } else {
      panic!("Formatted value was not a Yaml::Array");
    }
  }

  #[test]
  fn test_parse_invalid_secret_arrays() {
    // Not an array
    let val_not_seq = Yaml::String("not a sequence".to_string());
    assert!(parse_c5_secret_array(&val_not_seq).is_err());

    // Wrong length
    let val_wrong_len = Yaml::Array(vec![Yaml::String("a".into()), Yaml::String("b".into())]);
    assert!(parse_c5_secret_array(&val_wrong_len).is_err());

    // Non-string element
    let val_non_string = Yaml::Array(vec![
      Yaml::String("algo".into()),
      Yaml::Integer(123.into()), // Yaml::Integer, not a string
      Yaml::String("cipher".into()),
    ]);
    assert!(parse_c5_secret_array(&val_non_string).is_err());
  }
}
