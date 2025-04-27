use std::collections::HashMap;

use crate::{value::C5DataValue, providers::CONFIG_KEY_PROVIDER};

pub fn expand_vars(template_str: &str, variables: &HashMap<String, String>) -> String {

  let interpolator: Box<dyn Fn(&str) -> Result<Option<String>, ()>> = Box::new(|var_name: &str| {

    let lower_var_name: String = var_name.to_lowercase();

    if variables.contains_key(&lower_var_name) {
      return Ok(variables.get(&lower_var_name).map(|value| value.clone()));
    }

    panic!("Could not find variable: '{}' in string: '{}'", var_name, template_str);
  });

  return shellexpand::env_with_context(
    template_str,
    &*interpolator,
  ).unwrap().to_string();
}

// Recursive helper for flattening maps. Doesn't modify the source map.
fn build_flat_map_recursive(
  source_map: &HashMap<String, C5DataValue>, // Takes immutable ref
  flat_map_out: &mut HashMap<String, C5DataValue>, // Output map
  current_path: &str, // Use &str for efficiency
) {
  for (key, value) in source_map.iter() {
    let new_keypath = if current_path.is_empty() {
        key.clone()
    } else {
        format!("{}.{}", current_path, key)
    };

    match value {
      C5DataValue::Map(sub_map) => {
         // Avoid flattening provider config maps - their structure should be preserved
         // until handled by the provider itself.
         if !sub_map.contains_key(CONFIG_KEY_PROVIDER) {
            // Recurse for non-provider maps
            build_flat_map_recursive(sub_map, flat_map_out, &new_keypath);
         }
         // NOTE: We do NOT insert the C5DataValue::Map itself into the flat map.
         // If it was a provider map, it's skipped entirely here.
         // If it was a regular map, its children are added via recursion.
      }
      // Includes Primitives, Bytes, Strings, Booleans, Null, and Arrays
      _ => {
        // Insert non-map values (including arrays) directly into the flat map.
        flat_map_out.insert(new_keypath, value.clone());
      }
    }
  }
}

/// Flattens a nested `HashMap<String, C5DataValue>` into a single-level map
/// where keys represent the full path (e.g., "a.b.c").
///
/// This function does NOT modify the input `raw_config_data` map.
/// It populates the output `config_data` map.
/// Provider configurations (maps containing a `.provider` key) are skipped during flattening.
pub fn build_flat_map(
  raw_config_data: &HashMap<String, C5DataValue>, // Changed to immutable ref
  config_data: &mut HashMap<String, C5DataValue>, // Output map
  keypath: String, // Base path (often empty string)
) {
    // Call the recursive helper starting with the base path
    build_flat_map_recursive(raw_config_data, config_data, &keypath);
}