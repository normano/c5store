import JumpList from "@excsn/jumplist";
import naturalCompare from "string-natural-compare";

import { Logger } from "./telemetry";

const naturalCompareOpts = {
  "caseInsensitive": true,
};

const naturalCompareIgnoreCase = (key1, key2) => naturalCompare(key1, key2, naturalCompareOpts);

/** @internal */
export class C5DataStore {
  
  private _data: JumpList<string, any> = new JumpList<string, any>({
    "compareFunc": naturalCompareIgnoreCase,
  });

  /** @internal */
  getData(key: string) {
    return this._data.get(key);
  }

  /** @internal */
  setData(key: string, value: any) {
    this._data.set(key, value);
  }
  
  /** @internal */
  exists(keyPath: string): boolean {
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
