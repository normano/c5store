package com.excsn.c5store.core;

import java.util.Map;

public class C5ValueProviderSchema {
  public final String vProvider;
  public final String vKeyPath;
  public final String vKey;

  public C5ValueProviderSchema(Map<String, Object> data) {

    vProvider = (String) data.get(C5Consts.CONFIG_KEY_PROVIDER);
    vKeyPath = (String) data.get(C5Consts.CONFIG_KEY_KEYPATH);
    vKey = (String) data.get(C5Consts.CONFIG_KEY_KEYNAME);
  }
}
