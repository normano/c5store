import fs from "fs-extra";
import path from "path";

import { SetDataFn } from "./internal";

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
  hydrate(setData: SetDataFn, force: boolean): Promise<void>;
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

  constructor(data: any) {
    super(data);

    this.path = data.path;

    if ("encoding" in data) {
      this.encoding = data.encoding;
    }
  }
}

/**
 * Contents of a file are provided as a value
 */
export class C5FileValueProvider implements C5ValueProvider {

  private _keyDataMap: Map<string, C5FileValueProviderSchema> = new Map<string, C5FileValueProviderSchema>();

  constructor(
    private _fileRootDir: string
  ) {}

  public async register(vpData: any): Promise<void> {

    let schema = new C5FileValueProviderSchema(vpData);
    let keyPath = schema.vKeyPath;
    this._keyDataMap.set(keyPath, schema);
  }

  public async unregister(keyPath: string): Promise<void> {

    this._keyDataMap.delete(keyPath);
  }

  public async hydrate(setData: SetDataFn, force: boolean): Promise<void> {
    
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
      setData(keyPath, fileContents);
    }
  }
}