# Technical Deep Dive

## Terms

**Key Path** - Represents a path in the config tree. i.e. "first.big.thing"

**Config Tree** - Internal storage of key paths and values. SkipList Map with Alphanumeric key sort.

**C5DataStore** - Internal APIs to the config tree

**C5Store** - Read only Interface to C5DataStore to get data and subscribe to changes

**Value Provider** - Gathers data from external store and pushes it to C5DataStore

**C5StoreMgr** - Management interface to add value providers

## Configuration

Let's talk about application configuration expectations.

For example, you have three initial (seed) configuration files. common.yaml, local.yaml and production.yaml

**common.yaml**

    app_name: "My great app"
    http:
     host: localhost
     port: 3000

**local.yaml**

    mysql:
     db1:
      host: localhost
      user: local_user
      password: test

**production.yaml**

    http:
     host: localhost
     port: 80
    mysql:
     db1:
      host: prod.domain

Expectations are that:
 1. On our machine we expect **common.yaml** and **local.yaml** to be loaded
 2. On a production machine we expect **common.yaml** and **production.yaml** to be loaded.

Notice that the user and password on production were not overridden. That's because a value provider can be define to override those.
