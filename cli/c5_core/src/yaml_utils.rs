use crate::error::C5CoreError;
use yaml_rust2::yaml::Hash as YamlHash;
use yaml_rust2::{Yaml, YamlEmitter, YamlLoader};

// --- Load / Dump Utilities ---

pub fn load_yaml_from_string(yaml_str: &str) -> Result<Yaml, C5CoreError> {
  let docs = YamlLoader::load_from_str(yaml_str)
    .map_err(|e| C5CoreError::YamlDeserialize(format!("YAML loading failed: {:?}", e)))?;
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
    .map_err(|e| C5CoreError::YamlSerialize(format!("YAML emitting failed: {:?}", e)))?;
  Ok(out_str)
}

// --- Path Parsing Logic ---

/// Splits a path string by '.', respecting backslash escaping.
/// E.g., "a.b\.c.d" -> ["a", "b.c", "d"]
fn split_and_unescape_path(path: &str) -> Vec<String> {
  let mut parts = Vec::new();
  let mut current = String::new();
  let mut is_escaped = false;

  for c in path.chars() {
    if is_escaped {
      current.push(c);
      is_escaped = false;
    } else if c == '\\' {
      is_escaped = true;
    } else if c == '.' {
      parts.push(current);
      current = String::new();
    } else {
      current.push(c);
    }
  }
  parts.push(current);
  parts
}

// --- Get Logic ---

pub fn get_yaml_value_at_path<'a>(root: &'a Yaml, path_str: &str) -> Option<&'a Yaml> {
  if path_str.is_empty() {
    return Some(root);
  }

  let parts = split_and_unescape_path(path_str);
  let mut current = root;

  for part_str in parts {
    if part_str.is_empty() {
      return None; // Invalid empty segment "a..b"
    }

    match current {
      Yaml::Hash(map) => {
        let key_yaml = Yaml::String(part_str);
        match map.get(&key_yaml) {
          Some(val) => current = val,
          None => return None,
        }
      }
      Yaml::Array(arr) => {
        // Try to parse the segment as a usize index
        if let Ok(index) = part_str.parse::<usize>() {
          if index < arr.len() {
            current = &arr[index];
          } else {
            return None; // Index out of bounds
          }
        } else {
          return None; // Cannot use non-integer key on Array
        }
      }
      _ => return None, // Scalar or Null cannot be traversed
    }
  }
  Some(current)
}

// --- Set Logic ---

// Helper to get type name as string for Yaml
fn yaml_type_name(y: &Yaml) -> &'static str {
  match y {
    Yaml::String(_) => "String",
    Yaml::Integer(_) => "Integer",
    Yaml::Real(_) => "Real",
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

  let parts = split_and_unescape_path(path_str);
  if parts.iter().any(|p| p.is_empty()) {
    return Err(C5CoreError::YamlNavigation(format!(
      "Invalid empty segment in path: '{}'",
      path_str
    )));
  }

  let mut current_node = root;

  for (i, part_str) in parts.iter().enumerate() {
    let is_last = i == parts.len() - 1;

    // 1. Auto-Vivification Strategy: Map by Default.
    // If we hit a Null, we ALWAYS turn it into a Hash, even if the key looks like "0".
    // This prevents sparse arrays. The runtime (c5store) will decide later if
    // a Map like { "0": "val", "1": "val" } should be treated as an Array.
    if current_node.is_null() {
      *current_node = Yaml::Hash(YamlHash::new());
    }

    // 2. Traversal / Modification
    match current_node {
      Yaml::Hash(map) => {
        let key_yaml = Yaml::String(part_str.to_string());
        if is_last {
          map.insert(key_yaml, value_to_set);
          return Ok(());
        } else {
          // Descend or create next Null slot
          current_node = map.entry(key_yaml).or_insert(Yaml::Null);
        }
      }
      Yaml::Array(arr) => {
        // Parse index
        let idx = part_str.parse::<usize>().map_err(|_| {
          C5CoreError::YamlNavigation(format!(
            "Cannot navigate into Array with non-integer key '{}'. Path: {}",
            part_str, path_str
          ))
        })?;

        // Strict Bounds Checking
        if idx > arr.len() {
          return Err(C5CoreError::YamlNavigation(format!(
            "Index {} out of bounds (len is {}). Sparse arrays are not supported. Path: {}",
            idx,
            arr.len(),
            path_str
          )));
        }

        if is_last {
          if idx == arr.len() {
            // Append
            arr.push(value_to_set);
          } else {
            // Overwrite
            arr[idx] = value_to_set;
          }
          return Ok(());
        } else {
          // Traversal
          if idx == arr.len() {
            // Cannot descend into a slot that doesn't exist yet
            return Err(C5CoreError::YamlNavigation(format!(
              "Cannot traverse into index {} because it does not exist yet. Path: {}",
              idx, path_str
            )));
          }
          current_node = &mut arr[idx];
        }
      }
      _ => {
        // Scalar conflict
        let err_path_context = if i > 0 {
          parts[..i].join(".")
        } else {
          "root".to_string()
        };
        return Err(C5CoreError::YamlNavigation(format!(
          "Path '{}' requires segment '{}' to be a Container (Map/Array), but it's a {}.",
          path_str,
          err_path_context,
          yaml_type_name(current_node)
        )));
      }
    }
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use yaml_rust2::{yaml::Hash, Yaml};

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

  #[test]
  fn test_path_unescaping() {
    // Basic
    assert_eq!(split_and_unescape_path("a.b.c"), vec!["a", "b", "c"]);
    // Escaped dot
    assert_eq!(
      split_and_unescape_path("google\\.com.apiKey"),
      vec!["google.com", "apiKey"]
    );
    // Escaped backslash
    assert_eq!(split_and_unescape_path("folder\\\\.file"), vec!["folder\\", "file"]);
    // No separator
    assert_eq!(split_and_unescape_path("simple"), vec!["simple"]);
  }

  #[test]
  fn test_get_with_escaping_and_arrays() {
    // Setup: { "sites": [ { "google.com": "true" } ] }
    let mut inner_map = Hash::new();
    inner_map.insert(make_string("google.com"), make_string("found_it"));

    let mut root_map = Hash::new();
    root_map.insert(make_string("sites"), Yaml::Array(vec![Yaml::Hash(inner_map)]));
    let root = Yaml::Hash(root_map);

    // Test escaped get
    let val = get_yaml_value_at_path(&root, "sites.0.google\\.com");
    assert_eq!(val, Some(&make_string("found_it")));

    // Test array out of bounds
    assert_eq!(get_yaml_value_at_path(&root, "sites.1"), None);

    // Test bad type for array index
    assert_eq!(get_yaml_value_at_path(&root, "sites.foo"), None);
  }

  #[test]
  fn test_set_auto_vivification_map_by_default() -> Result<(), C5CoreError> {
    let mut root = Yaml::Null;

    // Setting "0" on Null should create a Hash, NOT an Array
    set_yaml_value_at_path(&mut root, "items.0", make_string("val"))?;

    // Verify items is a Hash
    match root {
      Yaml::Hash(m) => {
        let items = m.get(&make_string("items")).unwrap();
        match items {
          Yaml::Hash(inner_m) => {
            assert_eq!(inner_m.get(&make_string("0")), Some(&make_string("val")));
          }
          _ => panic!("Expected items to be a Hash"),
        }
      }
      _ => panic!("Expected root to be a Hash"),
    }
    Ok(())
  }

  #[test]
  fn test_set_array_logic() -> Result<(), C5CoreError> {
    // Setup: items: ["a", "b"]
    let mut root_map = Hash::new();
    root_map.insert(
      make_string("items"),
      Yaml::Array(vec![make_string("a"), make_string("b")]),
    );
    let mut root = Yaml::Hash(root_map);

    // 1. Overwrite existing index
    set_yaml_value_at_path(&mut root, "items.0", make_string("updated_a"))?;

    // 2. Append (idx == len)
    set_yaml_value_at_path(&mut root, "items.2", make_string("c"))?;

    // 3. Sparse Array Error (idx > len)
    let err = set_yaml_value_at_path(&mut root, "items.5", make_string("e"));
    assert!(matches!(err, Err(C5CoreError::YamlNavigation(_))));

    // Verify final state: ["updated_a", "b", "c"]
    if let Some(Yaml::Array(arr)) = get_yaml_value_at_path(&root, "items") {
      assert_eq!(arr.len(), 3);
      assert_eq!(arr[0], make_string("updated_a"));
      assert_eq!(arr[1], make_string("b"));
      assert_eq!(arr[2], make_string("c"));
    } else {
      panic!("items lost its array type");
    }

    Ok(())
  }

  #[test]
  fn test_set_escaped_key() -> Result<(), C5CoreError> {
    let mut root = Yaml::Hash(Hash::new());

    // Should create key "my.key" not map "my" -> key "key"
    set_yaml_value_at_path(&mut root, "my\\.key", make_string("val"))?;

    match &root {
      Yaml::Hash(m) => {
        assert_eq!(m.get(&make_string("my.key")), Some(&make_string("val")));
        assert_eq!(m.get(&make_string("my")), None);
      }
      _ => panic!("Root not hash"),
    }
    Ok(())
  }
}
