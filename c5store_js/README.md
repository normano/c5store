# C5Store for NodeJS

C5Store is an all encompassing configuration store. The idea is to have one place to query and dump configuration, secrets and etc.

Read more about it: https://github.com/normano/c5store

# Getting Started

To start using C5Store in yor NodeJS application.

1. Install as a dependency
- `npm i @excsn/c5store`
- `yarn add @excsn/c5store`

2. Import with 

`import {createC5Store, defaultConfigFiles, C5Store, C5StoreMgr} from "@excsn/c5store";`

3. Create a config folder with common.yaml

    foo: bar
    example:
     test:
      it: "today"
      my: 42

4. Create and use the store

   let [c5Store, c5StoreMgr] = await createC5Store(configFilePaths: ["common.yaml"]);
   
   // Use the store
   
   let data = c5Store.get("foo");

   // Use to get nested data by branching.
   
   let nestedData = c5Store.branch("example.test").get("my");

   // Inspect where you are on a branch
   
   console.log(c5Store.branch("example.test").currentKeyPath);

## Examples 
Look at the [example](example) folder for an implementation and how to use the C5FileValueProvider to get data from a file.