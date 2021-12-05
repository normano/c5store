import fs from "fs-extra";
import path from "path";

import { SetDataFn, HydrateContext } from "./internal";
import { C5ValueDeserializer, C5JSONValueDeserializer, C5YAMLValueDeserializer } from "./serialization";

export const CONFIG_KEY_KEYNAME = ".key";
export const CONFIG_KEY_KEYPATH = ".keyPath";
export const CONFIG_KEY_PROVIDER = ".provider";

export interface C5ValueProvider {

  /**
   * Registers key path to be watched and refreshed
   * @param vpData data for value provider that follows the schema from value provider
   */
  register(vpData: any): Promise<void>;

  /**
   * 
   * @param key Key path in the value provider
   */
  unregister(key: string): Promise<void>;

  /**
   * Fetch data and push into data store
   * @param force Forces changed and unchanged data to be refreshed
   */
  hydrate(setData: SetDataFn, force: boolean, context: HydrateContext): Promise<void>;
}

export class C5ValueProviderSchema {
  vProvider: string;
  vKeyPath: string;
  vKey: string;

  constructor(data: any) {

    this.vProvider = data[CONFIG_KEY_PROVIDER];
    this.vKeyPath = data[CONFIG_KEY_KEYPATH];
    this.vKey = data[CONFIG_KEY_KEYNAME];
  }
}

export class C5FileValueProviderSchema extends C5ValueProviderSchema {
  path: string;
  encoding: string = "utf-8";
  format: string = "raw";

  constructor(data: any) {
    super(data);

    this.path = data.path;

    if ("encoding" in data) {
      this.encoding = data.encoding;
    }

    if ("format" in data) {
      this.format = data.format;
    }
  }
}

/**
 * Contents of a file are provided as a value
 */
export class C5FileValueProvider implements C5ValueProvider {

  private _keyDataMap: Map<string, C5FileValueProviderSchema> = new Map<string, C5FileValueProviderSchema>();

  constructor(
    private _fileRootDir: string,
    private _deserializers: Map<string, C5ValueDeserializer> = new Map<string, C5ValueDeserializer>()
  ) {}

  public static createDefault(fileRootDir: string): C5FileValueProvider {

    let deserializers = new Map<string, C5ValueDeserializer>();
    deserializers.set("json", new C5JSONValueDeserializer());
    deserializers.set("yaml", new C5YAMLValueDeserializer());

    return new C5FileValueProvider(fileRootDir, deserializers);
  }

  public async register(vpData: any): Promise<void> {

    let schema = new C5FileValueProviderSchema(vpData);
    let keyPath = schema.vKeyPath;
    this._keyDataMap.set(keyPath, schema);
  }

  public async unregister(keyPath: string): Promise<void> {

    this._keyDataMap.delete(keyPath);
  }

  public registerDeserializer(formatName: string, deserializer: C5ValueDeserializer) {
    this._deserializers.set(formatName, deserializer);
  }

  public async hydrate(setData: SetDataFn, force: boolean, context: HydrateContext): Promise<void> {
    
    for(let [keyPath, vpData] of this._keyDataMap) {

      let filePath = vpData.path;

      if(!path.isAbsolute(filePath)) {
        filePath = path.resolve(this._fileRootDir, filePath);
      }

      if (!await fs.pathExists(filePath)) {
        setData(keyPath, null);
        continue;
      }

      let fileContents = await fs.readFile(filePath, vpData.encoding);
      let deserializedValue = null;

      if (vpData.format != "raw") {

        if(!this._deserializers.has(vpData.format)) {

          context.logger.warn(`${vpData.vKeyPath} cannot be deserialized since deserializer ${vpData.format} does not exist`);
          
          continue;
        }

        let deserializer = this._deserializers.get(vpData.format);
        deserializedValue = deserializer.deserialize(fileContents);
      } else {
        deserializedValue = fileContents;
      }

      if(deserializedValue == null) {

        context.logger.warn(`${vpData.vKeyPath} deserialized value is null.`);
        continue;
      }

      HydrateContext.pushValueToDataStore(setData, keyPath, deserializedValue,);
    }
  }
}