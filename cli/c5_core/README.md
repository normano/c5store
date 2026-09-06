# C5Store Core Utilities

**C5Store** (ConfigStore) is a library providing a **unified, traceable, and dynamic store for configuration and secrets** across multiple programming languages.

`c5_core` is the library beneath [`c5cli`](../c5cli/README.md): ECIES encryption and key generation, the c5store secret array, and configuration documents edited **in place**.

A document is YAML, TOML or JSON, chosen by the file's extension. A write replaces the bytes of one value and touches nothing else, so comments, blank lines, key order and the file's own indentation survive an edit. See [API_REFERENCE.md](API_REFERENCE.md) for the surface.
