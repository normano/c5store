package com.excsn.c5store.core.telemetry;

import java.time.Duration;
import java.util.Map;

public interface StatsRecorder {

  void recordCounterIncrement(Map<String, Object> tags, String name);
  void recordTimer(Map<String, Object> tags, String name, Duration value);
  void recordGauge(Map<String, Object> tags, String name, Number value);
}
