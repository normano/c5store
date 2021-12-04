package com.excsn.c5store.example;

import com.excsn.c5store.core.C5FileValueProvider;
import com.excsn.c5store.core.C5StoreBuilder;
import com.excsn.c5store.core.C5StoreUtils;
import com.excsn.c5store.core.telemetry.Logger;
import com.excsn.c5store.core.telemetry.StatsRecorder;

import java.nio.file.Path;
import java.nio.file.Paths;
import java.time.Duration;
import java.util.Map;
import java.util.stream.Collectors;

public class Main {
  public static void main(String[] args) throws Exception {

    var logger = new Logger() {
      @Override
      public void debug(String message) {

        System.out.println(message);
      }

      @Override
      public void info(String message) {

        System.out.println(message);
      }

      @Override
      public void warn(String message) {

        System.out.println(message);
      }

      @Override
      public void error(String message, Throwable throwable) {

        System.err.println(message);
      }
    };

    var statsRecorder = new StatsRecorder() {
      @Override
      public void recordCounterIncrement(Map<String, Object> tags, String name) {

      }

      @Override
      public void recordTimer(Map<String, Object> tags, String name, Duration value) {

      }

      @Override
      public void recordGauge(Map<String, Object> tags, String name, Number value) {

      }
    };

    var configDir = Paths.get("src", "main", "resources", "config").toAbsolutePath();
    var releaseEnv = "development";
    var appEnv = "local";
    var region = "localdc";

    var configFilePaths = C5StoreUtils.defaultConfigFilePaths(configDir.toString(), releaseEnv, appEnv, region);
    System.out.println("Config file paths: " + configFilePaths.stream().map(Path::toAbsolutePath).map(Path::toString).collect(Collectors.joining(", ")));

    var c5StoreHolder = C5StoreBuilder.builder().setChangeDelayPeriod(100).setTelemetry(logger, statsRecorder)
      .setConfigFilePaths(configFilePaths).build();

    var secretsDir = Paths.get("src", "main", "resources", "config", "secrets").toAbsolutePath();
    var secretsProvider = C5FileValueProvider.createDefault(secretsDir.toString());

    c5StoreHolder.configMgr.setVProvider("secrets", secretsProvider, Duration.ofSeconds(3));

    var whatToday = c5StoreHolder.config.get("what.today");
    System.out.println("Output of keypath 'what.today': " + whatToday);

    var whaWhy = c5StoreHolder.config.get("what.why");
    System.out.println("Output of keypath 'what.why': " + whaWhy);

    var secretStore = c5StoreHolder.config.get("secret.store");
    System.out.println("Output of keypath 'secret.store': " + secretStore);

    var aThread = new Thread(() -> {

      c5StoreHolder.config.subscribe("secret.store", (notifyKeyPath, keyPath, value) -> {

        System.err.println("Notify Key" + notifyKeyPath + ", keyPath: " + keyPath + " was sent change notification.");
        System.exit(1);
        throw new RuntimeException("FAILURE: Update should not occur since nothing has changed.");
      });

      try {
        Thread.sleep(500);
      } catch (InterruptedException e) {
        Thread.currentThread().interrupt();
      }
    });

    aThread.run();

    System.out.println("Example program ran successfully");
    c5StoreHolder.stopFn.run();
  }
}
