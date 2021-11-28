# C5Store Java

Simple and very easy to use configuration store in Java. Never ask yourself how to load in application configuration again.

Read about it [here](https://github.com/normano/c5store).

# Usage

Library is published with Maven POM metadata, so it is compatible with any package manager that supports it.

Snapshots are posted to OSS Sonatype.

Releases are published on Central.

## Gradle

Including the dependency

    ext {
      c5StoreVersion = '1.0.0-SNAPSHOT'
    }

    dependencies {
      implementation group: 'com.excsn.c5store', name: 'core', version: c5StoreVersion
    }

### Snapshot repo
Use if the version you want to use is snapshot

    repositories {
      maven {
        url "https://oss.sonatype.org/content/repositories/snapshots"
        mavenContent {
          snapshotsOnly()
        }
      }
    }
