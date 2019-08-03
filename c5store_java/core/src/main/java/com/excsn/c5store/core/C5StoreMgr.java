package com.excsn.c5store.core;

import com.excsn.c5store.core.telemetry.Logger;
import com.excsn.c5store.core.telemetry.StatsRecorder;
import com.google.common.collect.Multimap;

import java.time.Duration;
import java.util.HashMap;
import java.util.Map;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;

public class C5StoreMgr {

  private Map<String, C5ValueProvider> _valueProviders;
  private ScheduledExecutorService _scheduledExecutorService;
  private SetDataFn _setDataFn;
  private Multimap<String, Map<String, Object>> _providedData;
  private Logger _logger;
  private StatsRecorder _statsRecorder;

  C5StoreMgr(
    SetDataFn setDataFn,
    Multimap<String, Map<String, Object>> providedData,
    Logger logger,
    StatsRecorder statsRecorder
  ) {

    _valueProviders = new HashMap<>();
    _scheduledExecutorService = Executors.newSingleThreadScheduledExecutor();
    _setDataFn = setDataFn;
    _providedData = providedData;
    _logger = logger;
    _statsRecorder = statsRecorder;
  }

  public void setVProvider(String name, C5ValueProvider vProvider, Duration refreshPeriodDuration) {

    var hydrateContext = new HydrateContext(_logger);

    _valueProviders.put(name, vProvider);

    var values = _providedData.get(name);

    for (var value : values) {
      vProvider.register(value);
    }

    vProvider.hydrate(_setDataFn, true, hydrateContext);

    if (refreshPeriodDuration != null && refreshPeriodDuration.toMillis() > 0) {

      _logger.debug("Will refresh " + name + " Value Provider every " + refreshPeriodDuration.toSeconds()
        + " seconds");

      _scheduledExecutorService.scheduleAtFixedRate(() -> {

        vProvider.hydrate(_setDataFn, true, hydrateContext);
      }, refreshPeriodDuration.toSeconds(), refreshPeriodDuration.toSeconds(), TimeUnit.SECONDS);
    } else {

      _logger.debug("Will not refresh " + name + " Value Provider");
    }
  }

  void stop() {

    _logger.info("Stopping C5StoreMgr");

    _scheduledExecutorService.shutdownNow();
    _scheduledExecutorService = null;

    _logger.info("Stopped C5StoreMgr");
  }
}
