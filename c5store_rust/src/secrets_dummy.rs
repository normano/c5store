#[derive(Default, Clone, Debug)]
pub struct SecretKeyStore {}

impl SecretKeyStore {
  pub fn new() -> Self { Self {} }
  pub fn set_key(&mut self, _name: &str, _key: Vec<u8>) {}
}

pub type SecretKeyStoreConfiguratorFn = dyn FnMut(&mut SecretKeyStore);

#[derive(Default, Debug, Clone)]
pub struct SecretOptions {}