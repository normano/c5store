import { CONFIG_KEY_PROVIDER } from "./providers.js";

export function buildFlatMap(
  rawConfigData: any,
  configData: any,
  keyPath: string
) {

  const keys = Object.keys(rawConfigData);
  for(let key of keys) {

    let value = rawConfigData[key];
    let newKeyPath = (keyPath == null) ? key : `${keyPath}.${key}`;

    if((value instanceof Object) && !Array.isArray(value)) {

      let nextConfigData = rawConfigData[key];

      if(!(CONFIG_KEY_PROVIDER in nextConfigData)) {

        buildFlatMap(nextConfigData, configData, newKeyPath);

        if(Object.keys(rawConfigData[key]).length == 0) {
          delete rawConfigData[key];
        }
      }
    } else {

      configData[newKeyPath] = value;
    }
  }
}