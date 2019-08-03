package com.excsn.c5store.core;

import java.util.Map;

public class C5FileValueProviderSchema extends C5ValueProviderSchema {

  public final String path;
  public final String encoding;
  public final String format;

  public C5FileValueProviderSchema(Map<String, Object> data) {
    super(data);

    path = (String) data.get("path");
    encoding = (String) data.getOrDefault("encoding", "UTF-8");
    format = (String) data.getOrDefault("format", "raw");
  }
}
