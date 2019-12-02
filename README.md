# C5Store

Stands for ConfigStore. This library provides a unified store for configuration and secrets.


# Concept

Every application desires to get configuration from multiple sources and formats (file, network, database, etc.), so with C5Store every application has configuration store and data is fed in by Value Providers. A Value Provider, when registered, fetches data from a source and feeds it into the configuration store on a scheduled basis.
- For example, if a secret was entered into a file and it changed every so often, then we'd want to make sure our configs dynamically update with the latest data. First, you'd want to create a WatchedFileValueProvider and have it watch the secrets file. Whenever the file changes, you'd update the internal datastore of the provider for when the provider is called to push data to the configuration store will provide the new value.

Library is still WORK IN PROGRESS.
