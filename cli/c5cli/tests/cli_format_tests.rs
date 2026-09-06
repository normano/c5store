//! Encrypt and decrypt over each format the tool understands, and what the
//! file looks like afterwards. The point of every assertion here is that the
//! parts of the document nobody asked to change came out byte for byte.

use assert_cmd::prelude::*;
use serial_test::serial;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::fs;
use tempfile::tempdir;

fn c5cli_cmd() -> Command {
  Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap()
}

fn keys(dir: &Path, prefix: &str) -> (PathBuf, PathBuf) {
  let mut cmd = c5cli_cmd();
  cmd.current_dir(dir).arg("gen").arg("kp").arg(prefix).arg("--output-dir").arg(".");
  cmd.assert().success();
  (dir.join(format!("{prefix}.c5.pub.pem")), dir.join(format!("{prefix}.c5.key.pem")))
}

fn at(text: &str, format: c5_core::Format, path: &str) -> Option<c5_core::Value> {
  c5_core::Document::parse(text, format).unwrap().get(&c5_core::parse_path(path).unwrap()).unwrap()
}

/// Encrypts `plaintext` into `path` of a config named `file`, then decrypts it
/// back. Answers the file's contents afterwards and what came back out.
fn round_trip(name: &str, file: &str, contents: &str, path: &str, plaintext: &str) -> (String, String) {
  let dir = tempdir().unwrap();
  let root = dir.path().join("config");
  fs::create_dir_all(&root).unwrap();
  let (public, _) = keys(dir.path(), name);
  let config = root.join(file);
  fs::write(&config, contents).unwrap();

  c5cli_cmd()
    .arg("encrypt")
    .arg(file)
    .arg(public.file_name().unwrap())
    .arg(path)
    .arg("-v")
    .arg(plaintext)
    .arg("--config-root-dir")
    .arg(&root)
    .arg("--public-key-dir")
    .arg(dir.path())
    .arg("--commit")
    .assert()
    .success();

  let written = fs::read_to_string(&config).unwrap();

  let out = dir.path().join("plain.txt");
  c5cli_cmd()
    .arg("decrypt")
    .arg(file)
    .arg(path)
    .arg(format!("{name}.c5.key.pem"))
    .arg(&out)
    .arg("--config-root-dir")
    .arg(&root)
    .arg("--private-key-dir")
    .arg(dir.path())
    .assert()
    .success();

  (written, fs::read_to_string(&out).unwrap())
}

const YAML: &str = "\
# kept
db:
    host: localhost   # kept, aligned
    password: changeme

cache:
    ttl: 60
";

const TOML: &str = "\
# kept
[db]
host     = \"localhost\"  # kept, aligned
password = \"changeme\"

[cache]
ttl = 60
";

const JSON: &str = "{\n\t\"db\": {\n\t\t\"host\": \"localhost\",\n\t\t\"password\": \"changeme\"\n\t},\n\t\"cache\": {\n\t\t\"ttl\": 60\n\t}\n}\n";

#[test]
#[serial]
fn yaml_round_trips_and_keeps_the_rest_of_the_file() {
  let (written, plain) = round_trip("fmt_yaml", "app.yaml", YAML, "db.password", "s3cr3t");
  assert_eq!(plain, "s3cr3t");
  assert!(written.starts_with("# kept\ndb:\n    host: localhost   # kept, aligned\n"), "{written}");
  assert!(written.ends_with("\ncache:\n    ttl: 60\n"), "{written}");
  assert!(written.contains("\n    password:\n        \".c5encval\":\n            - ecies_x25519\n"), "{written}");
}

#[test]
#[serial]
fn toml_round_trips_and_keeps_its_comments_and_alignment() {
  let (written, plain) = round_trip("fmt_toml", "app.toml", TOML, "db.password", "s3cr3t");
  assert_eq!(plain, "s3cr3t");
  assert!(written.starts_with("# kept\n[db]\nhost     = \"localhost\"  # kept, aligned\n"), "{written}");
  assert!(written.contains("[cache]\nttl = 60\n"), "{written}");
  assert_eq!(
    at(&written, c5_core::Format::Toml, "db.host"),
    Some(c5_core::Value::String("localhost".to_owned()))
  );
}

#[test]
#[serial]
fn json_round_trips_and_keeps_its_tabs() {
  let (written, plain) = round_trip("fmt_json", "app.json", JSON, "db.password", "s3cr3t");
  assert_eq!(plain, "s3cr3t");
  assert!(written.starts_with("{\n\t\"db\": {\n\t\t\"host\": \"localhost\",\n"), "{written}");
  assert!(written.ends_with("\t\"cache\": {\n\t\t\"ttl\": 60\n\t}\n}\n"), "{written}");
  assert!(written.contains("\t\t\t\".c5encval\": ["), "{written}");
}

#[test]
#[serial]
fn a_key_created_in_each_format_reads_back() {
  for (name, file, contents, format) in [
    ("new_yaml", "app.yaml", YAML, c5_core::Format::Yaml),
    ("new_toml", "app.toml", TOML, c5_core::Format::Toml),
    ("new_json", "app.json", JSON, c5_core::Format::Json),
  ] {
    let (written, plain) = round_trip(name, file, contents, "auth.bootstrap.token", "t0ken");
    assert_eq!(plain, "t0ken", "{file}");
    let secret = at(&written, format, "auth.bootstrap.token").unwrap_or_else(|| panic!("{file}: {written}"));
    assert!(secret.get(".c5encval").is_some(), "{file}: {written}");
    assert!(written.contains("ttl"), "{file}: the rest of the document survived");
  }
}
