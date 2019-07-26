# C5Store for NodeJS

C5Store is an all encompassing configuration store. The idea is to have one place to query and dump configuration, secrets and etc.

Read more about it: https://github.com/normano/c5store

# Getting Started

To start using C5Store in yor NodeJS application.

1. Install as a dependency
- `npm i @exforte/c5store`
- `yarn add @exforte/c5store`

2. Import with 

`import {createC5Store, defaultConfigFiles, C5Store, C5StoreMgr} from "@exforte/c5store";`

3. Create a config folder with common.yaml

`foo: bar`

4. Create the store

`let [c5Store, c5StoreMgr] = await createC5Store(configFilePaths: Array<string>, logger: Logger, stats: StatsRecorder)`

5. Use the store

`let data = c5Store.get("foo")`

IMPORTANT: Look at the example folder for a implementation and how to use the C5FileValueProvider to get data from a  file.

# API

createC5Store(configFilePaths: Array<string>, logger: Logger, stats: StatsRecorder)
- Creates a 2-tuple containing C5Store and C5Store manager.

C5Store
- get(keyPath: string)
  - Gets data immediately from the store
- subscribe(keyPath: string, listener: ChangeListener) 
  - Listens to changes to the given keyPath. keyPath can be any the entire path or ancestors. By listening to an ancestor, one will receive one change event even if two children change.

C5StoreMgr
- setVProvider(
  name: string,
  vProvider: C5ValueProvider,
  refreshPeriodSec: number,
)
  - Registers the value provider, immediately fetches all values it will provide and schedules periodic refreshes. refreshPeriodSec if 0 will not perform any scheduling.
- stop()
  - Stops all of the scheduled refreshes.