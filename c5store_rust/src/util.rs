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

pub fn build_flat_map(
  raw_config_data: &mut HashMap<String, C5DataValue>,
  config_data: &mut HashMap<String, C5DataValue>,
  keypath: String,
) {
  let keys: Vec<String> = raw_config_data.keys().into_iter().cloned().collect();

  for key in keys {
    let mut value = raw_config_data.get_mut(&key).unwrap();
    let new_keypath: String;

    if keypath.is_empty() {
      new_keypath = key.clone();
    } else {
      new_keypath = keypath.clone() + "." + &key;
    }

    if let C5DataValue::Map(ref mut data_map) = &mut value {
      if !data_map.contains_key(CONFIG_KEY_PROVIDER) {
        build_flat_map(data_map, config_data, new_keypath);

        if data_map.len() == 0 {
          raw_config_data.remove(&key);
        }
      }
    } else {
      config_data.insert(new_keypath.clone(), value.clone());
    }
  }
}