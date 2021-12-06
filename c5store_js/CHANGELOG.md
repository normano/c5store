# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0-prerelease.5]

### Added
- buildFlatMap function and is public for any value providers to use to smash down objects into dot notation
- HydrateContext.pushValueToDataStore is public so value providers can send their deserialized objects to the data store for merging

### Changed
- File Value Provider now merges objects into the data store. Functionality before this was that an object would be put into the data store which get would return an object.
- Example project was changed to reflect the file value provider's merging functionality.

## [1.0.0-prerelease.4]

### Added
- Secrets decryption with ECIES 25519 library.

### Changed
- Tags are now <string, any> instead of <string, string> to reflect the idea that tags can be different datatypes.