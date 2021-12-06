# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.2]

### Added
- build_flat_map function and is public for any value providers to use to smash down objects into dot notation
- HydrateContext.push_value_to_data_store is public so value providers can send their deserialized objects to the data store for merging

### Changed
- File Value Provider now merges objects into the data store. Functionality before this was that an object would be put into the data store which get would return an C5Value::Map.

## [0.2.1]

### Changed
- Set SecretOptions fields to public

## [0.2.0]

### Added
- Secrets decryption with ECIES 25519 library.

### Changed
- Tags are now <string, TagValue> instead of <string, string> to reflect the idea that tags can be different datatypes.