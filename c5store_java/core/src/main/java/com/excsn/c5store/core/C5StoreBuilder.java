package com.excsn.c5store.core;

import com.excsn.c5store.core.telemetry.Logger;
import com.excsn.c5store.core.telemetry.StatsRecorder;
import org.yaml.snakeyaml.Yaml;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Collection;
import java.util.HashMap;
import java.util.Map;

public class C5StoreBuilder {

  private Collection<Path> _configFilePaths;
  private Logger _logger;
  private StatsRecorder _statsRecorder;

  private C5StoreBuilder() {}

  public static C5StoreBuilder builder() {

    return new C5StoreBuilder();
  }

  public C5StoreBuilder setConfigFilePaths(Collection<Path> paths) {

    this._configFilePaths = paths;
    return this;
  }

  public C5StoreBuilder setTelemetry(Logger logger, StatsRecorder statsRecorder) {

    this._logger = logger;
    this._statsRecorder = statsRecorder;
    return this;
  }

  public C5InitHolder build() {

    var yaml = new Yaml();
    var c5StoreSubscriptions = new C5StoreSubscriptions();
    var c5DataStore = new C5DataStore();
    var changeNotifier = new ChangeNotifier(c5DataStore, c5StoreSubscriptions);

    var c5Store = new C5Store(c5DataStore::getData, c5StoreSubscriptions);

    SetDataFn setDataFn = (keyPath, value) -> {
      c5DataStore.setData(keyPath, value);
      changeNotifier.changeNotify(keyPath);
    };

    var rawConfigData = new HashMap<String, Object>();

    for (var configFilePath : _configFilePaths) {

      if (!Files.exists(configFilePath)) {

        continue;
      }

      try {
        var fileContents = Files.readString(configFilePath);
        var configFileYaml = yaml.loadAs(fileContents, Map.class);

        C5StoreUtils.deepMerge(rawConfigData, configFileYaml);
      } catch(IOException e) {

      }
    }

    var extractedConfigData = C5StoreUtils.extractProvidedAndConfigData(rawConfigData);
    var c5StoreMgr = new C5StoreMgr(setDataFn, extractedConfigData.providedData, _logger, _statsRecorder);

    for (var configDataEntry : extractedConfigData.configData.entrySet()) {

      setDataFn.setData(configDataEntry.getKey(), configDataEntry.getValue());
    }

    Runnable stopFn = () -> {
      c5StoreMgr.stop();
      changeNotifier.stop();
    };

    return new C5InitHolder(c5Store, c5StoreMgr, stopFn);
  }
}
