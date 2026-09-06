# Changelog
All notable changes to `c5_core` and `c5cli` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/x.y.z/).

Rolling a release turns what has accumulated under `[Unreleased]` into a version. Insert a `## [x.y.z] - YYYY-MM-DD` heading directly below `[Unreleased]`, move the sections that have entries down under that heading, and leave the empty section headings behind. `[Unreleased]` therefore always stays at the top with its full set of sections, ready for the next change, and version headings run newest first with every released version keeping an entry.

Between rolls, an entry goes under `[Unreleased]` in the section matching its change type: `Added` for new features, `Changed` for changes to existing behaviour, `Deprecated` for features about to go, `Removed` for features now gone, `Fixed` for bug fixes and `Security` for vulnerabilities. A section with no entries is left empty rather than deleted.

## [Unreleased]

### Added
- c5cli reads and writes TOML and JSON as well as YAML, chosen by the file's extension.
- c5_core::Document, a configuration document edited in place: a write replaces one value and leaves comments, blank lines, key order and indentation as they were.
- c5_core::Value and Format, and parse_path moved here from c5cli.

### Changed
- c5cli no longer re-emits the document it writes to, which stripped every comment and reflowed the file.
- format_c5_secret_array returns Value and parse_c5_secret_array takes one, in place of yaml_rust2::Yaml.
- A YAML value written in flow style is refused by name rather than reflowed around.
- A path that names nothing says which segment it stopped at.
- A new key is quoted only where the format requires it.

### Deprecated

### Removed
- c5_core::yaml_utils, which Document answers.
- serde_yaml and serde_yaml2, only ever used to re-emit a document.

### Fixed

### Security
