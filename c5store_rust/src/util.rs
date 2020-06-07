use std::collections::HashMap;

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