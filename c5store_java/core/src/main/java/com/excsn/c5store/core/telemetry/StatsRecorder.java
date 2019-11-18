package com.excsn.c5store.core.telemetry;

import java.time.Duration;
import java.util.Map;

public interface StatsRecorder {

  void recordCounterIncrement(Map<String, String> tags, String name);
  void recordTimer(Map<String, String> tags, String name, Duration value);
  void recordGauge(Map<String, String> tags, String name, Number value);
}
