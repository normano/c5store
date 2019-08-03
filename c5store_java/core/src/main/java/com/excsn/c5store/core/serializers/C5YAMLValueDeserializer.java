package com.excsn.c5store.core.serializers;

import org.yaml.snakeyaml.Yaml;

public class C5YAMLValueDeserializer implements C5ValueDeserializer<String> {

  Yaml _yaml;

  public C5YAMLValueDeserializer() {
    _yaml = new Yaml();
  }

  @Override
  public <Value> Value deserialize(String data) {
    return _yaml.load(data);
  }

  @Override
  public <Value> Value deserialize(String data, Class<Value> deserializationType) {
    return _yaml.loadAs(data, deserializationType);
  }
}
