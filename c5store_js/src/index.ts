import {ArrayMultimap, SetMultimap} from "@teppeis/multimaps";
import fs from "fs-extra";
import yaml from "js-yaml";
import _merge from "lodash.merge";
import nodeSchedule from "node-schedule";
import path from "path";

import {C5DataStore, GetDataFn, SetDataFn} from "./internal";
import { C5ValueProvider, CONFIG_KEY_PROVIDER, CONFIG_KEY_KEYPATH, CONFIG_KEY_KEYNAME } from "./providers";
import { StatsRecorder, Logger } from "./telemetry";

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
export class C5Store {

  constructor(private _getFn: GetDataFn, private _subscriptions: C5StoreSubscriptions) {

  }

  public get(keyPath: string) {

    return this._getFn(keyPath);
  }

  public subscribe(keyPath: string, listener: ChangeListener) {

    this._subscriptions.add(keyPath, listener);
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

  public async setVProvider(
    name: string,
    vProvider: C5ValueProvider,
    refreshPeriodSec: number,
  ): Promise<void> {
    
    this._valueProviders.set(name, vProvider);

    let values = this._providedData.get(name);

    for(let value of values) {
      await vProvider.register(value);
    }

    await vProvider.hydrate(this._set, true);

    if (refreshPeriodSec > 0) {
      this._logger.debug(`Will refresh ${name} Value Provider every ${refreshPeriodSec} seconds.`);

      let refreshRecurrenceRule = new nodeSchedule.RecurrenceRule();
      let minutes = Math.floor(refreshPeriodSec / 60);
      let seconds = (refreshPeriodSec - (minutes * 60));
      
      refreshRecurrenceRule.second = new nodeSchedule.Range(0, 59, seconds);
      refreshRecurrenceRule.minute = new nodeSchedule.Range(0, 59, minutes);

      let scheduledProviderHydate = nodeSchedule.scheduleJob(refreshRecurrenceRule, () => {
        vProvider.hydrate(this._set, true);
      });

      this._scheduledProviderHydates.push(scheduledProviderHydate);
    } else {
      this._logger.debug(`Will not be refreshing ${name} Value Provider`);
    }
  }

  stop() {

    this._logger.info("Stopping C5StoreMgr");
    for (let scheduledProviderHydate of this._scheduledProviderHydates) {
      scheduledProviderHydate.cancel();
    }

    this._scheduledProviderHydates = [];
    this._logger.info("Stopped C5StoreMgr");
  }
}

export async function createC5Store(configFilePaths: Array<string>, logger: Logger, stats: StatsRecorder): Promise<[C5Store, C5StoreMgr]> {

  let changeSubscriptions = new C5StoreSubscriptions();
  let internalStore = new C5DataStore();
  let c5Store = new C5Store(internalStore.getData.bind(internalStore), changeSubscriptions);

  let changeDelayPeriod = 1 * 1000;
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
    
    clearChangeTimer();

    changedKeyPaths.add(key);

    changeTimer = setTimeout(() => {

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
  };

  let setData = (key, value) => {

    // Changes are immediately visible, but not sure if it is the best idea. Maybe should
    // wait until change notfications are resolved to be sent out.
    internalStore.setData(key, value);
    changeNotify(key);
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

export function defaultConfigFiles(configDir: string, releaseEnv: string, env:string, datacenter: string): Array<string> {

  return [
    "common.yaml",
    `${releaseEnv}.yaml`,
    `${env}.yaml`,
    `${datacenter}.yaml`,
    `${env}-${datacenter}.yaml`,
  ].map((configFilePath) => path.resolve(configDir, configFilePath));
}

async function extractProvidedAndConfigData(rawConfigData: object): Promise<[object, ArrayMultimap<string, any>]> {

  let configData = {};
  let providedData = new ArrayMultimap<string, any>();
  traverseConfig(rawConfigData, configData, providedData, null);

  return [configData, providedData];
}

function traverseConfig(rawConfigData: object, configData: any, providedData: ArrayMultimap<string, any>, keyPath: string) {

  let keys = Object.keys(rawConfigData);
  for(let key of keys) {
    
    let value = rawConfigData[key];
    let newKeyPath = (keyPath == null) ? key : `${keyPath}.${key}`;

    if((value instanceof Object)) {

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