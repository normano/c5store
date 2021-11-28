package com.excsn.c5store.core.serializers;

import com.fasterxml.jackson.databind.ObjectMapper;

import java.io.IOException;
import java.util.Map;

public class C5JSONValueDeserializer implements C5ValueDeserializer<String> {

  private final ObjectMapper _objectMapper;

  public C5JSONValueDeserializer(ObjectMapper objectMapper) {
    _objectMapper = objectMapper;
  }

  public static C5JSONValueDeserializer create() {

    var objectMapper = new ObjectMapper();

    return new C5JSONValueDeserializer(objectMapper);
  }

  @Override
  public Map deserialize(String data) {

    try {
      return _objectMapper.readValue(data, Map.class);
    } catch (IOException e) {
      return null;
    }
  }

  @Override
  public <Value> Value deserialize(String data, Class<Value> deserializationType) {

    try {
      return _objectMapper.readValue(data, deserializationType);
    } catch (IOException e) {
      return null;
    }
  }
}
