# C5Store

Stands for ConfigStore. This library provides a unified store for configuration and secrets.

# Background

Essentially, there should be one place to read configuration values while gathering values from multiple sources. If any changes occur while the application is running then the application should have the choice to reconfigure itself on the flu.

Reading a "map" from a yaml file is great, but how does an application read from outside datasources? How does it respond to value changes? Keep writing custom handlers or use a framework to do this? Passing the tree (map) or branches (maps of maps, submaps) around is riddled with mistakes. Where are you in the tree? Just never made sense to me every time I started on a new project at home or work.

What if could embed my pet peeves into a library. One where I could pass around a store where I could do the things I was doing with a map get data, check if the key exists and pass around sub maps? Bonus would be that I could subscribe to changes and reconfigure the application without intrusive restarts. C5Store is what I call it. Now I can just pick this library and not ask what to use, where or how to use a configuration store.

# Concept

Applications desire to get configuration from multiple sources and formats (file, network, database, etc.). C5Store enables every application to have their own configuration store where data is fed in by Value Providers. Value Providers bring in data from multiple sources and feed into the configuration store.
- Look at the [javascript implementation](c5store_js) for an example.

Also, C5Store enables applications to reconfigure if a value changes via subscription.
- [Tech Example] For example, if a secret was entered into a file and it changed every so often, then we'd want to make sure our configs dynamically update with the latest data. First, you'd want to create a WatchedFileValueProvider and have it watch the secrets file. Whenever the file changes, you'd update the internal datastore of the provider for when the provider is called to push data to the configuration store will provide the new value.

## Secrets

Secrets are provided in the format ["decryptor name", "key name", "encrypted value"] where the key path ends with ".c5enval".

Secrets are encrypted application configuration values like database passwords or google client secret keys. In C5Store, it is implemented in a highly effective way utilizating existing code paths. Secrets are decrypted at the same time the data is loaded in, so a seed configuration file or value provider can bring in secrets and there will be decrypted on the fly. This means secrets are updatable and the application can reconfigure itself in response. 

In terms of setting up the secrets decryption, the application will load in the private keys (file names are the key name) from a specified directory, but the decryptors must be manually specified in code.

Look at tests for an example.

# Note

1. In the future maybe there will be a public cli for encryption and setting the data in the config yaml files.