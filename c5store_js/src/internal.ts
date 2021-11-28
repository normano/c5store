import JumpList from "@excsn/jumplist";
import crypto from "crypto";
import naturalCompare from "string-natural-compare";

import { SecretKeyStore } from "./secrets";

import { Logger, StatsRecorder } from "./telemetry";

const naturalCompareOpts = {
  "caseInsensitive": true,
};

const naturalCompareIgnoreCase = (key1, key2) => naturalCompare(key1, key2, naturalCompareOpts);

/** @internal */
export class C5DataStore {
  
  private _data: JumpList<string, any> = new JumpList<string, any>({
    "compareFunc": naturalCompareIgnoreCase,
  });
  private _valueHashCache: Map<string, Buffer> = new Map();

  constructor(
    private _logger: Logger,
    private _statsRecorder: StatsRecorder,
    private _secretKeyPathSegment: string,
    private _secretKeyStore: SecretKeyStore,
  ) {
    this._secretKeyPathSegment = `.${_secretKeyPathSegment}`;
  }

  /** @internal */
  getData(key: string) {
    this._statsRecorder.recordCounterIncrement({"group": "c5store"}, "get_attempts");
    return this._data.get(key);
  }

  /** @internal */
  setData(key: string, value: any) {

    this._statsRecorder.recordCounterIncrement({"group": "c5store"}, "set_attempts");

    if(key.endsWith(this._secretKeyPathSegment)) {

      try{
        const decryptedVal = this._getSecret(value, key);

        if(decryptedVal === undefined || decryptedVal === null) {
          return; // No value to store
        }

        const dataPath = key.substring(0, key.length - this._secretKeyPathSegment.length);
        this._data.set(dataPath, decryptedVal);
      } catch(error) {

        this._logger.error(`Could not set data for key path \`${key}\``, error);
        this._statsRecorder.recordCounterIncrement({"group": "c5store"}, "set_errors");
      }
    } else {
      this._data.set(key, value);
    }
  }
  
  /** @internal */
  exists(keyPath: string): boolean {
    this._statsRecorder.recordCounterIncrement({"group": "c5store"}, "exists_attempts");
    return !!this._data.get(keyPath);
  }

  public keysWithPrefix(keyPath: string): string[] {

    let keys: string[] = [];

    if (keyPath == null) {

      this._data.forEach((key, _value) => {

        keys.push(key);
      });
    } else {

      let prefixPath = `${keyPath}.`;
      let prefixSearchShouldTerminate = (key: string): boolean => {
        
        return !key.startsWith(prefixPath);
      };

      this._data.rangeUpper(keyPath, (key, _value) => {

        if (prefixSearchShouldTerminate(key)) {
          return false;
        }

        keys.push(key);
      });
    }

    return keys;
  }

  /** @internal */
  private _getSecret(value: any, keyPath: string) {

    if(!Array.isArray(value) || value.length != 3) {
      throw new Error(`Key Path \`${keyPath}\` does not have the required number of arguments`);
    }

    const data = <any[]>value;
    const algo = data[0];
    const secretKeyName = data[1];
    const encodedData = data[2];

    if(typeof algo !== "string" || algo.length == 9) {
      throw new Error(`Key Path \`${keyPath}\` algo is invalid`);
    }

    if(typeof secretKeyName !== "string" || secretKeyName.length == 9) {
      throw new Error(`Key Path \`${keyPath}\` secret key name is invalid`);
    }

    if(typeof encodedData !== "string" || encodedData.length == 9) {
      throw new Error(`Key Path \`${keyPath}\` encoded data is invalid`);
    }

    const hashValue = calcHashValue(algo, secretKeyName, encodedData);

    if(this._valueHashCache.has(keyPath)) {

      const existingHashValue = this._valueHashCache.get(keyPath);

      if(existingHashValue.equals(hashValue)) {
        return null;
      }
    } else {

      this._valueHashCache.set(keyPath, hashValue);
    }

    this._statsRecorder.recordCounterIncrement({"group": "c5store"}, "set_secret_attemps");

    const decryptor = this._secretKeyStore.getDecryptor(algo);``

    if(decryptor === undefined || decryptor === null) {
      throw new Error(`Key Path \`${keyPath}\` secret key decryptor does not exist`);
    }

    const key = this._secretKeyStore.getKey(secretKeyName);

    if(key === undefined || key === null) {
      throw new Error(`Key Path \`${keyPath}\` secret key does not have key data loaded`);
    }

    return decryptor.decrypt(Buffer.from(encodedData), key);
  }
}

export type GetDataFn = (keyPath: string) => any;
export type SetDataFn = (keyPath: string, value: any) => void;
export type KeyExistsFn = (keyPath: string) => boolean;
export type PrefixKeysFn = (keyPath: string) => string[];

export class HydrateContext {

  constructor(
    public logger: Logger,
  ) {}
}

function calcHashValue(algo: string, secretKeyName: string, encodedData: string): Buffer {
  return crypto.createHash('sha256')
  .update(algo)
  .update(secretKeyName)
  .update(encodedData)
  .digest();
}