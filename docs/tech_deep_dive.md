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
      password:
        .c5encval: ["ecies_x25519", "test_local", "iQv4jONnOQvzIXqkYPT1q2VbD+A1fCKG6yp+VhCtAQdm3N2J4vhv/Z9rtGVp88gmmXo/rFdG7rGQ9hyIQDB8S6auVagBFPI="]

**production.yaml**

    http:
     host: localhost
     port: 80
    mysql:
      db1:
        .provider: storage
        path: "sfo1/mysql_db1.yaml"
        format: "yaml"

**storage/sfo1/mysql_db1.yaml**

    host: prod.domain
    user: local_user
    password:
      .c5encval: ["ecies_x25519", "test_local", "iQv4jONnOQvzIXqkYPT1q2VbD+A1fCKG6yp+VhCtAQdm3N2J4vhv/Z9rtGVp88gmmXo/rFdG7rGQ9hyIQDB8S6auVagBFPI="]

Expectations are that:

1. On our machine we expect **common.yaml** and **local.yaml** to be loaded
2. On a production machine we expect **common.yaml** and **production.yaml** to be loaded.
 - `storage` is the name of value provider set in code through C5Store Manager. This value provider could be file value provider or user defined value provider.
3. `.c5encval` is a suffix designated by default for key/values to be decrypted. Value is decrypted into binary which the user must transform into the desired data type.
