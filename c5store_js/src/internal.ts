export class C5DataStore {
  private _data: Map<string, any> = new Map<string, any>();

  getData(key: string) {
    return this._data.get(key);
  }

  setData(key: string, value: any) {
    this._data.set(key, value);
  }
}

export type GetDataFn = (key: string) => any;
export type SetDataFn = (key: string, value: any) => void;