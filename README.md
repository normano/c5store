# C5Store

Stands for ConfigStore. This library provides a unified store for configuration and secrets.

# Background

Reading a "map" from a yaml file is great, but how does an application read from outside datasources? How does it respond to value changes? Keep writing custom handlers or use a framework to do this? Passing these maps of maps (tree) around is riddled with mistakes. Where are you in the tree? Just never made sense to me every time I started on a new project at home or work.

What if could embed my pet peeves into a library. One where I could pass around a store where I could do the things I was doing with a map get data,check if the key exists and pass around sub maps? Bonus would be that I could subscribe to changes and reconfigure the application without intrusive restarts. C5Store is what I call it. Now I can just pick this library and not ask what to use, where or how to use a configuration store.

# Concept

Applications desire to get configuration from multiple sources and formats (file, network, database, etc.). C5Store enables every application to have their own configuration store where data is fed in by Value Providers. Value Providers bring in data from multiple sources and feed into the configuration store.
- Look at the [javascript implementation](c5store_js) for an example.

Also, C5Store enables applications to reconfigure if a value changes via subscription.
- [Tech Example] For example, if a secret was entered into a file and it changed every so often, then we'd want to make sure our configs dynamically update with the latest data. First, you'd want to create a WatchedFileValueProvider and have it watch the secrets file. Whenever the file changes, you'd update the internal datastore of the provider for when the provider is called to push data to the configuration store will provide the new value.

