# Concepts

## Configuration Data
It is inherently key and value where the value can be key and value, eventually forming a tree of data. A path is formed using a chain of keys (ex. key1+key12+key123 where key123 is a leaf node). Path can be of any chain of keys such that a tree of data may be a result.

Configuration data is hierarchical meaning data is inherited and overridden. A value provider's leaf node may override the node of another value provider depending on the order of priority.

## C5Value Provider
The interface that provides the data when asked. Something like a YAMLFileValueProvider would provide data from a yaml. This is the concept that allows multiple sources to bring in their data.

## C5Store
This is the sharable store that one can use to get data. It is a readonly view on the data. A user would ask for data immediately using a path or subscribe for changes to a path.

## C5StoreManager
Is responsible for hydrating the internal store and scheduling the continuous hydration of the internal store shared with C5Store. Value providers are registered with the manager.
