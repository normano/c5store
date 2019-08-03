package com.excsn.c5store.core;

import com.google.common.collect.Multimap;

import java.util.Map;

class ExtractedConfigData {

  public final Map<String, Object> configData;
  public final Multimap<String, Map<String, Object>> providedData;

  ExtractedConfigData(
    Map<String, Object> configData,
    Multimap<String, Map<String, Object>> providedData
  ) {
    this.configData = configData;
    this.providedData = providedData;
  }
}
