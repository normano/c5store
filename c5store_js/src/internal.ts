import { Logger } from "./telemetry";

export class C5DataStore {
  private _data: Map<string, any> = new Map<string, any>();

  getData(key: string) {
    return this._data.get(key);
  }

  setData(key: string, value: any) {
    this._data.set(key, value);
  }
}

export type GetDataFn = (keyPath: string) => any;
export type SetDataFn = (keyPath: string, value: any) => void;

export class HydrateContext {

  constructor(
    public logger: Logger,
  ) {}
}
