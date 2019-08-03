package com.excsn.c5store.core.serializers;

public interface C5ValueDeserializer<Input> {

  <Value> Value deserialize(Input data);

  <Value> Value deserialize(Input data, Class<Value> deserializationType);
}
