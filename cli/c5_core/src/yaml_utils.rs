// c5_core/src/yaml_utils.rs
use crate::error::C5CoreError;
use hashlink::lru_cache::Entry;
use yaml_rust2::yaml::Hash as YamlHash; // Alias for the LinkedHashMap
use yaml_rust2::{Yaml, YamlEmitter, YamlLoader}; // For loading/emitting

pub fn load_yaml_from_string(yaml_str: &str) -> Result<Yaml, C5CoreError> {
  let docs = YamlLoader::load_from_str(yaml_str)
    .map_err(|e| C5CoreError::YamlDeserialize(format!("YAML loading failed: {:?}", e)))?; // Adjust error mapping
  if docs.is_empty() {
    Ok(Yaml::Hash(YamlHash::new())) // Return empty map for empty input
  } else {
    Ok(docs[0].clone()) // Take the first document
  }
}

pub fn dump_yaml_to_string(yaml_doc: &Yaml) -> Result<String, C5CoreError> {
  let mut out_str = String::new();
  let mut emitter = YamlEmitter::new(&mut out_str);
  emitter
    .dump(yaml_doc)
    .map_err(|e| C5CoreError::YamlSerialize(format!("YAML emitting failed: {:?}", e)))?; // Adjust error mapping
  Ok(out_str)
}

pub fn get_yaml_value_at_path<'a>(root: &'a Yaml, path_str: &str) -> Option<&'a Yaml> {
  if path_str.is_empty() {
    return Some(root);
  }
  let parts: Vec<&str> = path_str.split('.').collect();
  let mut current = root;
  for part_str in parts {
    if part_str.is_empty() {
      return None;
    }
    match current {
      Yaml::Hash(map) => {
        let key_yaml = Yaml::String(part_str.to_string());
        match map.get(&key_yaml) {
          Some(val) => current = val,
          None => return None,
        }
      }
      _ => return None, // Not a hash, cannot go deeper
    }
  }
  Some(current)
}

// Helper to get type name as string for Yaml
// (You might need to add this or use a similar utility if Yaml doesn't have .type_name())
fn yaml_type_name(y: &Yaml) -> &'static str {
  match y {
      Yaml::String(_) => "String",
      Yaml::Integer(_) => "Integer",
      Yaml::Real(_) => "Real", // yaml-rust2 uses Real for f64
      Yaml::Boolean(_) => "Boolean",
      Yaml::Array(_) => "Array",
      Yaml::Hash(_) => "Hash",
      Yaml::Alias(_) => "Alias",
      Yaml::Null => "Null",
      Yaml::BadValue => "BadValue",
  }
}

pub fn set_yaml_value_at_path(root: &mut Yaml, path_str: &str, value_to_set: Yaml) -> Result<(), C5CoreError> {
  if path_str.is_empty() {
      *root = value_to_set;
      return Ok(());
  }
  let parts: Vec<&str> = path_str.split('.').collect();
  if parts.iter().any(|p| p.is_empty()) {
      return Err(C5CoreError::YamlNavigation(format!("Invalid empty segment in path: '{}'", path_str)));
  }

  let mut current_map_ref = root; // This will always point to the Yaml node that *is* the current map

  for (i, part_str) in parts.iter().enumerate() {
      // Ensure current_map_ref is a Hash. If Null, make it one. If other, error.
      if !current_map_ref.is_hash() {
          if current_map_ref.is_null() {
              *current_map_ref = Yaml::Hash(YamlHash::new());
          } else {
              let err_path_context = if i > 0 { parts[..i].join(".") } else { "root".to_string() };
              return Err(C5CoreError::YamlNavigation(format!(
                  "Path '{}' requires segment '{}' to be a Map, but it's a {}.",
                  path_str, err_path_context, yaml_type_name(current_map_ref)
              )));
          }
      }

      // Now current_map_ref is guaranteed to be a Yaml::Hash
      let map = match current_map_ref {
          Yaml::Hash(m) => m,
          _ => unreachable!(), // Should have been handled or errored above
      };
      
      let key_yaml = Yaml::String(part_str.to_string());

      if i == parts.len() - 1 { // Last part, set the value in the current map
          map.insert(key_yaml, value_to_set);
          return Ok(());
      } else { // Intermediate part, get/create the next map and update current_map_ref
          current_map_ref = map.entry(key_yaml).or_insert_with(|| Yaml::Hash(YamlHash::new()));
      }
  }
  unreachable!("Loop should have returned");
}

#[cfg(test)]
mod tests {
  use super::*;
  use yaml_rust2::{yaml::Hash, Yaml}; // For constructing test Yaml values

  fn make_string(s: &str) -> Yaml {
    Yaml::String(s.to_string())
  }
  fn make_int(i: i64) -> Yaml {
    Yaml::Integer(i)
  }
  fn make_map() -> Yaml {
    Yaml::Hash(Hash::new())
  }
  fn make_seq() -> Yaml {
    Yaml::Array(Vec::new())
  }

  #[test]
  fn test_load_and_dump_yaml() -> Result<(), C5CoreError> {
    let yaml_str = "key: value\nnested:\n  item1: 123\n  item2: true\narray:\n  - one\n  - two";
    let doc = load_yaml_from_string(yaml_str)?;
    assert!(matches!(doc, Yaml::Hash(_)));

    let dumped_str = dump_yaml_to_string(&doc)?;
    let reloaded_doc = load_yaml_from_string(&dumped_str)?;
    assert_eq!(doc, reloaded_doc);

    // Test empty string input
    let empty_doc = load_yaml_from_string("")?;
    assert_eq!(empty_doc, Yaml::Hash(Hash::new())); // Expect an empty map

    // Test loading and dumping Yaml::Null
    let null_doc_loaded = load_yaml_from_string("null")?; // Parses "null" string to Yaml::Null
    assert_eq!(null_doc_loaded, Yaml::Null);

    let dumped_yaml_null = dump_yaml_to_string(&Yaml::Null)?; // Dump Yaml::Null directly
    let reloaded_dumped_null = load_yaml_from_string(&dumped_yaml_null)?;
    assert_eq!(reloaded_dumped_null, Yaml::Null); // Check if it reloads as Yaml::Null

    // Test invalid YAML parsing
    let invalid_yaml = "key: [unclosed array";
    let load_result_invalid = load_yaml_from_string(invalid_yaml);
    assert!(matches!(load_result_invalid, Err(C5CoreError::YamlDeserialize(_))));

    Ok(())
  }
  
  #[test]
  fn test_get_yaml_value_at_path() {
    let mut root_map = Hash::new();
    let mut nested_map = Hash::new();
    nested_map.insert(make_string("level2_key"), make_string("level2_value"));
    root_map.insert(make_string("level1_scalar"), make_string("scalar_value"));
    root_map.insert(make_string("level1_map"), Yaml::Hash(nested_map));
    let root = Yaml::Hash(root_map);

    // Get scalar
    assert_eq!(
      get_yaml_value_at_path(&root, "level1_scalar"),
      Some(&make_string("scalar_value"))
    );
    // Get nested scalar
    assert_eq!(
      get_yaml_value_at_path(&root, "level1_map.level2_key"),
      Some(&make_string("level2_value"))
    );
    // Get nested map
    assert!(matches!(
      get_yaml_value_at_path(&root, "level1_map"),
      Some(Yaml::Hash(_))
    ));
    // Get root itself
    assert_eq!(get_yaml_value_at_path(&root, ""), Some(&root));

    // Non-existent paths
    assert_eq!(get_yaml_value_at_path(&root, "non_existent"), None);
    assert_eq!(get_yaml_value_at_path(&root, "level1_scalar.sub_key"), None); // scalar has no sub_key
    assert_eq!(get_yaml_value_at_path(&root, "level1_map.non_existent_level2"), None);

    // Invalid paths
    assert_eq!(get_yaml_value_at_path(&root, "level1_map..level2_key"), None); // Empty segment
    assert_eq!(get_yaml_value_at_path(&root, ".level1_map"), None); // Starts with dot

    // Get from a non-map
    let scalar_root = make_string("iamscalar");
    assert_eq!(get_yaml_value_at_path(&scalar_root, "some.key"), None);
  }

  #[test]
  fn test_set_yaml_value_at_path() -> Result<(), C5CoreError> {
    // 1. Set on empty root (becomes the root)
    let mut root1 = Yaml::Null;
    set_yaml_value_at_path(&mut root1, "", make_string("new_root_value"))?;
    assert_eq!(root1, make_string("new_root_value"));

    // 2. Set top-level key in an empty map
    let mut root2 = make_map();
    set_yaml_value_at_path(&mut root2, "new_key", make_int(123))?;
    assert_eq!(get_yaml_value_at_path(&root2, "new_key"), Some(&make_int(123)));

    // 3. Set nested key, creating intermediate maps
    let mut root3 = make_map();
    set_yaml_value_at_path(&mut root3, "a.b.c", make_string("deep_value"))?;
    assert_eq!(
      get_yaml_value_at_path(&root3, "a.b.c"),
      Some(&make_string("deep_value"))
    );
    assert!(matches!(get_yaml_value_at_path(&root3, "a"), Some(Yaml::Hash(_))));
    assert!(matches!(get_yaml_value_at_path(&root3, "a.b"), Some(Yaml::Hash(_))));

    // 4. Overwrite existing scalar
    let mut root4 = make_map();
    set_yaml_value_at_path(&mut root4, "key", make_string("old"))?;
    set_yaml_value_at_path(&mut root4, "key", make_string("new"))?;
    assert_eq!(get_yaml_value_at_path(&root4, "key"), Some(&make_string("new")));

    // 5. Overwrite existing map with scalar
    let mut root5 = make_map();
    set_yaml_value_at_path(&mut root5, "key.sub", make_string("sub_value"))?;
    set_yaml_value_at_path(&mut root5, "key", make_string("now_scalar"))?;
    assert_eq!(get_yaml_value_at_path(&root5, "key"), Some(&make_string("now_scalar")));
    assert_eq!(get_yaml_value_at_path(&root5, "key.sub"), None); // sub should be gone

    // 6. Attempt to set sub-key on a scalar (should fail if path is deeper)
    let mut root6 = make_map();
    set_yaml_value_at_path(&mut root6, "key", make_string("iamscalar"))?;
    assert!(matches!(
      set_yaml_value_at_path(&mut root6, "key.sub", make_string("fail")),
      Err(C5CoreError::YamlNavigation(_))
    ));

    // 7. Set on a path where intermediate is Null, should turn to map
    let mut root7 = make_map(); // root7 is Yaml::Hash(empty_map)
                                // To insert into root7, we need to match to get its &mut Hash
    match &mut root7 {
      Yaml::Hash(map) => {
        map.insert(make_string("a"), Yaml::Null);
      }
      _ => panic!("root7 was expected to be a Hash"),
    }
    // Now root7 is Yaml::Hash({"a": Yaml::Null})

    // This call should turn "a": Yaml::Null into "a": Yaml::Hash({"b": Yaml::String("worked")})
    set_yaml_value_at_path(&mut root7, "a.b", make_string("worked"))?;
    assert_eq!(get_yaml_value_at_path(&root7, "a.b"), Some(&make_string("worked")));
    // Also check that "a" is now a map
    match get_yaml_value_at_path(&root7, "a") {
      Some(Yaml::Hash(_)) => { /* good */ }
      other => panic!("Expected 'a' to be a Hash, got {:?}", other),
    }

    // 8. Invalid path (empty segment)
    let mut root8 = make_map();
    assert!(matches!(
      set_yaml_value_at_path(&mut root8, "a..b", make_string("fail")),
      Err(C5CoreError::YamlNavigation(_))
    ));

    Ok(())
  }
}
