import {ArrayMultimap, SetMultimap} from "@teppeis/multimaps";
import dequal from "dequal";
import fs from "fs-extra";
import yaml from "js-yaml";
import _merge from "lodash.merge";
import nodeSchedule from "node-schedule";
import path from "path";

import {C5DataStore, GetDataFn, SetDataFn, HydrateContext, KeyExistsFn, PrefixKeysFn} from "./internal";
import { C5ValueProvider, CONFIG_KEY_PROVIDER, CONFIG_KEY_KEYPATH, CONFIG_KEY_KEYNAME } from "./providers";
import { StatsRecorder, Logger } from "./telemetry";

const DEFAULT_CHANGE_DELAY_PERIOD = 500;
type ChangeListener = (notifyKeyPath: string, keyPath: string, value: any) => void;

class C5StoreSubscriptions {

  _changeListeners: ArrayMultimap<string, ChangeListener> = new ArrayMultimap<string, ChangeListener>();

  public add(keyPath: string, listener: ChangeListener) {

    this._changeListeners.put(keyPath, listener);
  }

  public getSubscribers(keyPath: string): Array<ChangeListener> {

    return this._changeListeners.get(keyPath);
  }

  public notifyValueChange(notifyKeyPath: string, keyPath: string, value: any) {

    for(let changeListener of this._changeListeners.get(notifyKeyPath)) {

      changeListener(notifyKeyPath, keyPath, value);
    }
  }
}

/**
 * A way to read configuration.
 *
 * Primarily read values and subscribe to keys.
 */
export interface C5Store {

  /**
   * Gets data immediately from the store
   */
  get<Type>(keyPath: string): Type;

  exists(keyPath: string): boolean;

  /**
   * Listens to changes to the given keyPath. keyPath can be any the entire path or ancestors. By listening to an ancestor, one will receive one change event even if two children change.
   */
  subscribe(keyPath: string, listener: ChangeListener): void;

  /**
   * Creates a branch.
   * @param prefixKeyPath relative keypath from current Key Path
   */
  branch(prefixKeyPath: string): C5Store;

  /**
   * @return null if root, prefixKey if branch
   */
  readonly currentKeyPath: string;

  /**
   * Searches for all keypaths that relative to currentKeyPath + given keyPath
   * @return A list of Key Paths
   */
  keyPathsWithPrefix(keyPath: string): string[];
}

export class C5StoreRoot implements C5Store {

  constructor(
    private _getFn: GetDataFn,
    private _existsFn: KeyExistsFn,
    private _prefixKeysFn: PrefixKeysFn,
    private _subscriptions: C5StoreSubscriptions
  ) {}

  public get<Type>(keyPath: string): Type {

    return this._getFn(keyPath);
  }

  public subscribe(keyPath: string, listener: ChangeListener) {

    this._subscriptions.add(keyPath, listener);
  }

  public exists(keyPath: string): boolean {

    return this._existsFn(keyPath);
  }

  public branch(prefixKeyPath: string): C5Store {
    return new C5StoreBranch(this, prefixKeyPath);
  }

  public get currentKeyPath(): string {
    return null;
  }

  public keyPathsWithPrefix(keyPath: string): string[] {

    return this._prefixKeysFn(keyPath);
  }
}

export class C5StoreBranch implements C5Store {

  constructor(
    private _root: C5StoreRoot,
    private _keyPath: string
  ) {}

  public get<Type>(keyPath: string): Type {

    return this._root.get(this._mergeKeyPath(keyPath));
  }

  public exists(keyPath: string): boolean {
    return this._root.exists(this._mergeKeyPath(keyPath));
  }

  public subscribe(keyPath: string, listener: ChangeListener): void {

    this._root.subscribe(this._mergeKeyPath(keyPath), listener);
  }

  public branch(prefixKeyPath: string): C5Store {

    return this._root.branch(this._mergeKeyPath(prefixKeyPath));
  }

  public get currentKeyPath(): string {
    return this._keyPath;
  }

  public keyPathsWithPrefix(keyPath: string): string[] {

    return this._root.keyPathsWithPrefix(this._mergeKeyPath(keyPath));
  }

  private _mergeKeyPath(keyPath: string): string {

    return `${this._keyPath}.${keyPath}`;
  }
}

/**
 * A way to manage configuration oroviders.
 */
export class C5StoreMgr {

  _valueProviders: Map<string, C5ValueProvider> = new Map<string, C5ValueProvider>();
  _scheduledProviderHydates = [];

  constructor(
    private _set: SetDataFn,
    private _providedData: ArrayMultimap<string, any>,
    private _logger: Logger,
    private _stats: StatsRecorder,
  ) {

  }

  /**
   * Registers the value provider, immediately fetches all values it will provide and schedules periodic refreshes. refreshPeriodSec if 0 will not perform any scheduling.
   */
  public async setVProvider(
    name: string,
    vProvider: C5ValueProvider,
    refreshPeriodSec: number,
  ): Promise<void> {

    let hydrateContext = new HydrateContext(this._logger);

    this._valueProviders.set(name, vProvider);

    let values = this._providedData.get(name);

    for(let value of values) {
      await vProvider.register(value);
    }

    await vProvider.hydrate(this._set, true, hydrateContext);

    if (refreshPeriodSec > 0) {
      this._logger.debug(`Will refresh ${name} Value Provider every ${refreshPeriodSec} seconds.`);

      let refreshRecurrenceRule = new nodeSchedule.RecurrenceRule();
      let minutes = Math.floor(refreshPeriodSec / 60);
      let seconds = (refreshPeriodSec - (minutes * 60));

      refreshRecurrenceRule.second = new nodeSchedule.Range(0, 59, seconds);
      refreshRecurrenceRule.minute = new nodeSchedule.Range(0, 59, minutes);

      let scheduledProviderHydate = nodeSchedule.scheduleJob(refreshRecurrenceRule, () => {
        vProvider.hydrate(this._set, true, hydrateContext);
      });

      this._scheduledProviderHydates.push(scheduledProviderHydate);
    } else {
      this._logger.debug(`Will not be refreshing ${name} Value Provider`);
    }
  }

  /**
   * Stops all of the scheduled refreshes.
   */
  stop() {

    this._logger.info("Stopping C5StoreMgr");
    for (let scheduledProviderHydate of this._scheduledProviderHydates) {
      scheduledProviderHydate.cancel();
    }

    this._scheduledProviderHydates = [];
    this._logger.info("Stopped C5StoreMgr");
  }
}

/**
 * Creates a 2-tuple containing C5Store and C5Store manager.
 */
export async function createC5Store(
  configFilePaths: Array<string>,
  options?: {
    logger?: Logger,
    stats?: StatsRecorder,
    changeDelayPeriod?: number,
  }
): Promise<[C5Store, C5StoreMgr]> {

  if(!options) {
    options = {};
  }

  let logger = null;
  if(!options.logger) {

    logger = {
      "debug": console.log,
      "info": console.log,
      "warn": console.log,
      "error": console.log,
    };
  } else {
    logger = options.logger;
  }

  let stats = null;
  if(!options.stats) {
    stats = {
      "recordCounterIncrement": () => {},
      "recordGauge": () => {},
      "recordTimer": () => {},
    };
  } else {
    stats = options.stats;
  }

  const changeSubscriptions = new C5StoreSubscriptions();
  const internalStore = new C5DataStore();
  const c5Store = new C5StoreRoot(
    internalStore.getData.bind(internalStore),
    internalStore.exists.bind(internalStore),
    internalStore.keysWithPrefix.bind(internalStore),
    changeSubscriptions
  );

  let changeDelayPeriod = DEFAULT_CHANGE_DELAY_PERIOD;
  if(options.changeDelayPeriod > -1) {
    changeDelayPeriod = options.changeDelayPeriod;
  }
  let changeTimer = null;
  let changedKeyPaths = new Set<string>();

  const clearChangeTimer = () => {

    if (changeTimer != null) {
      clearTimeout(changeTimer);
      changeTimer = null;
    }
  };

  const changeNotify = (key: string) => {

    // Split key into parts then notify up the tree if any listeners

    // Batch and Dedup: If keys in the same ancestors are being updated, then send only one update for the
    // ancestors.
    // Can use a timer of maybe 2 seconds and reset it everytime a change notify comes in until
    // the 2 seconds is elapsed then perform change notifications.

    changedKeyPaths.add(key);

    if(changeTimer == null) {
      changeTimer = setTimeout(() => {

        clearChangeTimer();

        let savedChangedKeyPaths = changedKeyPaths;
        changedKeyPaths = new Set();
        changeTimer = null;

        let dedupedSavedChangedKeyPathsMap = new SetMultimap<string, string>();

        for (let savedChangedKeyPath of savedChangedKeyPaths) {

          dedupedSavedChangedKeyPathsMap.put(savedChangedKeyPath, savedChangedKeyPath);

          let splitSavedChangedKeyPath = savedChangedKeyPath.split(".");
          let keyAncestorPath = "";

          for(let savedChangedKeyPathPart of splitSavedChangedKeyPath) {

            if (keyAncestorPath != "") {
              keyAncestorPath += ".";
            }

            keyAncestorPath += savedChangedKeyPathPart;

            dedupedSavedChangedKeyPathsMap.put(savedChangedKeyPath, keyAncestorPath);
          }
        }

        for(let savedChangedKeyPath of dedupedSavedChangedKeyPathsMap.keys()) {

          let dedupedSavedChangedKeyPaths = dedupedSavedChangedKeyPathsMap.get(savedChangedKeyPath);

          let value = internalStore.getData(savedChangedKeyPath);
          for(let dedupedSavedChangedKeyPath of dedupedSavedChangedKeyPaths) {

            changeSubscriptions.notifyValueChange(dedupedSavedChangedKeyPath, savedChangedKeyPath, value);
          }
        }
      }, changeDelayPeriod);
    }
  };

  const setData = (key, value) => {

    //TODO Changes are immediately visible, but not sure if it is the best idea. Maybe should
    // wait until change notfications are resolved to be sent out first.

    // Do not send notification if it doesn't already exist
    const alreadyExists = internalStore.exists(key);
    if(!alreadyExists) {

      internalStore.setData(key, value);

    } else {

      // Do not do anything if value is equal
      const oldValue = internalStore.getData(key);

      if(!dequal(oldValue, value)) {
        internalStore.setData(key, value);
        changeNotify(key);
      }
    }
  };

  let rawConfigData = {};

  for(let configFilePath of configFilePaths) {

    if(!await fs.pathExists(configFilePath)) {
      continue;
    }

    let fileContents = await fs.readFile(configFilePath, "utf-8");
    let configFileYaml = yaml.safeLoad(fileContents);

    _merge(rawConfigData, configFileYaml);
  }

  let [configData, providedData] = await extractProvidedAndConfigData(rawConfigData);

  let c5StoreMgr = new C5StoreMgr(setData, providedData, logger, stats);

  let configDataKeys = Object.keys(configData);
  for (let configDataKey of configDataKeys) {

    setData(configDataKey, configData[configDataKey]);
  }

  return [c5Store, c5StoreMgr];
}

/**
 * Returns array of yaml file paths that are prefixed with config dir with file names constructed using the rest of the args
 */
export function defaultConfigFiles(configDir: string, releaseEnv: string, env:string, region: string): Array<string> {

  return [
    "common.yaml",
    `${releaseEnv}.yaml`,
    `${env}.yaml`,
    `${region}.yaml`,
    `${env}-${region}.yaml`,
  ].map((configFilePath) => path.resolve(configDir, configFilePath));
}

async function extractProvidedAndConfigData(rawConfigData: object): Promise<[object, ArrayMultimap<string, any>]> {

  let configData = {};
  let providedData = new ArrayMultimap<string, any>();
  traverseConfig(rawConfigData, configData, providedData, null);

  return [configData, providedData];
}

function traverseConfig(
  rawConfigData: object,
  configData: any,
  providedData: ArrayMultimap<string, any>,
  keyPath: string
) {

  let keys = Object.keys(rawConfigData);
  for(let key of keys) {

    let value = rawConfigData[key];
    let newKeyPath = (keyPath == null) ? key : `${keyPath}.${key}`;

    if((value instanceof Object) && !Array.isArray(value)) {

      let nextConfigData = rawConfigData[key];

      if(!(CONFIG_KEY_PROVIDER in nextConfigData)) {

        traverseConfig(nextConfigData, configData, providedData, newKeyPath);

        if(Object.keys(rawConfigData[key]).length == 0) {
          delete rawConfigData[key];
        }

        continue;
      } else {

        value[CONFIG_KEY_KEYPATH] = newKeyPath;
        value[CONFIG_KEY_KEYNAME] = key;

        providedData.put(value[CONFIG_KEY_PROVIDER], value);

        delete rawConfigData[key];
      }
    } else {

      configData[newKeyPath] = value;
    }
  }
}
