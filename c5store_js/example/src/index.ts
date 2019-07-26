import {createC5Store, defaultConfigFiles} from "@excsn/c5store";
import path from "path";
import * as telemetry from "@excsn/c5store/dist/telemetry";
import { C5FileValueProvider } from "@excsn/c5store/dist/providers";

async function main() {

  let configDir = path.resolve(__dirname, "..", "config");

  let configFilePaths = defaultConfigFiles(configDir, "development", "local", "localdc");

  let logger: telemetry.Logger = {
    debug: console.log,
    info: console.log,
    warn: console.log,
    error: console.log,
  };

  let statsRecorder: telemetry.StatsRecorder = {
    recordCounterIncrement: () => {},
    recordGauge: () => {},
    recordTimer: () => {},
  };

  let [c5Store, c5StoreMgr] = await createC5Store(configFilePaths, logger, statsRecorder);

  logger.info("Subscribed to bill, so associated keys should print out every so often assuming stop is not called.");
  c5Store.subscribe("bill", console.log);

  let resourcesDirPath = path.resolve(__dirname, "..", "resources");
  let resourcesFileProvider = new C5FileValueProvider(resourcesDirPath);
  await c5StoreMgr.setVProvider("resources", resourcesFileProvider, 3);

  logger.info(`example.foo ${c5Store.get("example.foo")}`);
  logger.info(`bill.bullshit ${c5Store.get("bill.bullshit")}`);

  c5StoreMgr.stop();
}

main().then(() => {

  console.log("Finished main func");
});