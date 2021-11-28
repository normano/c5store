# Value Providers

Value providers are the defacto way to extend configuration outside of the initial (seed) configuration files.

# C5Store File Value Provider

This standard provider provides values stored in a file into a path specified in the seed file. It will publish changes when the file changes.

**Initial Instance parameters**

- provider name
- root directory where it will read files from.
- refresh period in secs

The configuration in a seed file looks like the below.

## Seed yaml (example: common.yaml)
        some_key:
          .provider: "app_instance"
          path: "mysql.yaml"
          encoding: "utf8"
          format: "yaml"

### Key and description

**path**: is the relative path from the initial root directory from above.

**format**: Supports "raw", "yaml" and "json" by default. Can be extended to support more by the user.

**encoding** [default: utf8]: encoding such as "utf8", etc. Depends on the file reader used in each language.