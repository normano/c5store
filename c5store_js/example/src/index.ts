import {createC5Store, defaultConfigFiles} from "@excsn/c5store";
import * as telemetry from "@excsn/c5store/dist/telemetry";
import { C5FileValueProvider } from "@excsn/c5store/dist/providers";
import path from "path";
import util from "util";

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

  let [c5Store, c5StoreMgr] = await createC5Store(configFilePaths, {
    logger,
    "stats": statsRecorder,
    "changeDelayPeriod": 100,
  });

  logger.info("Subscribed to bill, so associated keys should print out every so often assuming stop is not called.");
  c5Store.subscribe("bill", console.log);

  let resourcesDirPath = path.resolve(__dirname, "..", "resources");
  let resourcesFileProvider = C5FileValueProvider.createDefault(resourcesDirPath);
  await c5StoreMgr.setVProvider("resources", resourcesFileProvider, 3);

  logger.info(`example.foo ${c5Store.get("example.foo")}`);
  logger.info(`bill.bullshit ${c5Store.get("bill.bullshit")}`);
  logger.info(`example.junk ${util.inspect(c5Store.get("example.junk"))}`);
  logger.info(`example.secret ${util.inspect(c5Store.get("example.secret"))}`);
  logger.info(`list_of_items ${util.inspect(c5Store.get("list_of_items"))}`);

  let exampleTestConfig = c5Store.branch("example.test");
  logger.info(`Direct branch: example.test.my ${util.inspect(exampleTestConfig.get("my"))}`);

  let exampleTestMy = c5Store.branch("example").branch("test").get("my");

  if(!exampleTestConfig.exists("my")) {
    throw new Error("example.test.my must exist")
  }

  if(exampleTestConfig.get("my") !== exampleTestMy) {
    throw new Error("example.test.my from direct branch and two branch must be equal");
  }

  logger.info(`Two branch: example.test.my ${util.inspect(exampleTestMy)}`);

  let keyPrefixes = c5Store.keyPathsWithPrefix("example");
  logger.info(`example key prefixes ${util.inspect(keyPrefixes)}`);

  await new Promise((resolve, reject) => {
    
    let successTimeout = setTimeout(resolve, 500);

    c5Store.subscribe("example.junk", (notifyKeyPath, keyPath, value) => {
      console.log(`Notify Key ${notifyKeyPath}, keyPath: ${keyPath} was sent change notification.`);
      reject("FAILURE: Update should not occur since nothing has changed.");
      clearTimeout(successTimeout);
    });
  });

  console.log("Example program successfully ran");
  
  c5StoreMgr.stop();
}

main().then(() => {

  console.log("Finished main func.");
});