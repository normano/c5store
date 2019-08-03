package com.excsn.c5store.example;

import com.excsn.c5store.core.C5FileValueProvider;
import com.excsn.c5store.core.C5StoreBuilder;
import com.excsn.c5store.core.C5StoreUtils;
import com.excsn.c5store.core.telemetry.Logger;
import com.excsn.c5store.core.telemetry.StatsRecorder;

import java.nio.file.Paths;
import java.time.Duration;
import java.util.Map;

public class Main {
  public static void main(String[] args) {

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
      public void error(String message) {

        System.err.println(message);
      }
    };

    var statsRecorder = new StatsRecorder() {
      @Override
      public void recordCounterIncrement(Map<String, String> tags, String name) {

      }

      @Override
      public void recordTimer(Map<String, String> tags, String name, Number value) {

      }

      @Override
      public void recordGauge(Map<String, String> tags, String name, Number value) {

      }
    };

    var configDir = Paths.get("example", "src", "main", "resources", "config").toAbsolutePath();
    var releaseEnv = "development";
    var appEnv = "local";
    var region = "localdc";

    var configFilePaths = C5StoreUtils.defaultConfigFilePaths(configDir.toString(), releaseEnv, appEnv, region);
    var c5StoreHolder = C5StoreBuilder.builder().setTelemetry(logger, statsRecorder)
      .setConfigFilePaths(configFilePaths).build();

    var secretsDir = Paths.get("example", "src", "main", "resources", "config", "secrets").toAbsolutePath();
    var secretsProvider = C5FileValueProvider.createDefault(secretsDir.toString());

    c5StoreHolder.configMgr.setVProvider("secrets", secretsProvider, Duration.ofSeconds(60));

    var whatToday = c5StoreHolder.config.get("what.today");
    System.out.println("Output of keypath 'what.today': " + whatToday);

    var whaWhy = c5StoreHolder.config.get("what.why");
    System.out.println("Output of keypath 'what.why': " + whaWhy);

    var secretStore = c5StoreHolder.config.get("secret.store");
    System.out.println("Output of keypath 'secret.store': " + secretStore);

    c5StoreHolder.stopFn.run();
  }
}
