package com.excsn.c5store.core;

import com.excsn.c5store.core.serializers.C5ValueDeserializer;
import com.excsn.c5store.core.telemetry.Logger;
import org.junit.jupiter.api.Assertions;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;
import org.mockito.Mockito;
import org.yaml.snakeyaml.Yaml;

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.HashMap;
import java.util.Map;

@TestInstance(TestInstance.Lifecycle.PER_CLASS)
class C5FileValueProviderTest {

  private static String _fp1BarKeyPath = "file_provider1.bar";
  private Map<String, C5ValueDeserializer> _deserializers;
  private Yaml _yaml;
  private Map _providerData;
  private Path _providerConfigPath;

  @BeforeAll
  public void setup() throws Exception {

    _yaml = new Yaml();
    _deserializers = new HashMap<>();
    var configRoot = Paths.get("src","test", "resources", "config");
    _providerConfigPath = Path.of(configRoot.toString(),"foo");

    var providerDataString = Files.readString(Path.of(configRoot.toString(), "file_provider_config.yaml"));
    _providerData = _yaml.loadAs(providerDataString, Map.class);
  }

  @Test
  public void registersVpData() {

    var barProvider = _createBarProvider();
    var schema = barProvider.getSchema(_fp1BarKeyPath);

    Assertions.assertTrue(schema.vProvider.equals("bar_provider"));
    Assertions.assertTrue(schema.path.equals("bar.txt"));
  }

  @Test
  public void hydratesRegisteredData() {

    var logger = Mockito.mock(Logger.class);
    var setDataFn = Mockito.mock(SetDataFn.class);
    var barProvider = _createBarProvider();

    barProvider.hydrate(setDataFn, true, new HydrateContext(logger));

    Mockito.verify(setDataFn, Mockito.times(1)).setData(Mockito.eq(_fp1BarKeyPath), Mockito.any());
  }

  private C5FileValueProvider _createBarProvider() {

    C5FileValueProvider barProvider = new C5FileValueProvider(_providerConfigPath.toString(), _deserializers);
    var vpData = (Map<String, Object>) ((Map<String, Object>)_providerData.get("file_provider1")).get("bar");
    vpData.put(C5Consts.CONFIG_KEY_KEYPATH, _fp1BarKeyPath);

    barProvider.register(vpData);

    return barProvider;
  }
}
