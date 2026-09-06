use c5_core::document::{Document, Format};
use c5_core::path::parse_path;
use c5_core::value::Value;

fn secret() -> Value {
  Value::Map(vec![(
    ".c5encval".to_owned(),
    Value::Array(vec![Value::from("ecies_x25519"), Value::from("prod"), Value::from("Y2lwaGVy==")]),
  )])
}

fn set(text: &str, format: Format, path: &str, value: &Value) -> String {
  let mut document = Document::parse(text, format).unwrap();
  document.set(&parse_path(path).unwrap(), value).unwrap();
  document.text().to_owned()
}

fn get(text: &str, format: Format, path: &str) -> Option<Value> {
  Document::parse(text, format).unwrap().get(&parse_path(path).unwrap()).unwrap()
}

// --- YAML ---

const YAML: &str = "\
# the database this service talks to
db:
    host: localhost      # not in production
    password: changeme

# left alone entirely
cache:
    ttl: 60
";

#[test]
fn yaml_replacing_a_value_keeps_every_other_byte() {
  let out = set(YAML, Format::Yaml, "db.password", &secret());
  assert_eq!(
    out,
    "\
# the database this service talks to
db:
    host: localhost      # not in production
    password:
        \".c5encval\":
            - ecies_x25519
            - prod
            - Y2lwaGVy==

# left alone entirely
cache:
    ttl: 60
"
  );
}

#[test]
fn yaml_keeps_the_files_own_indent_width() {
  let two = "db:\n  host: local\n  password: changeme\n";
  let out = set(two, Format::Yaml, "db.password", &Value::from("secret"));
  assert_eq!(out, "db:\n  host: local\n  password: secret\n");
}

/// YAML forbids a tab as indentation, so a file that uses one is not a file
/// this can edit; it says so rather than guessing what was meant.
#[test]
fn yaml_refuses_a_tab_indented_file() {
  let tabbed = "db:\n\thost: local\n";
  assert!(Document::parse(tabbed, Format::Yaml).is_err());
}

#[test]
fn json_keeps_a_tab_indent() {
  let tabbed = "{\n\t\"db\": {\n\t\t\"host\": \"local\"\n\t}\n}\n";
  let out = set(tabbed, Format::Json, "db.token", &Value::from("t"));
  assert_eq!(out, "{\n\t\"db\": {\n\t\t\"host\": \"local\",\n\t\t\"token\": \"t\"\n\t}\n}\n");
}

#[test]
fn yaml_adds_a_key_to_an_existing_block() {
  let out = set(YAML, Format::Yaml, "db.token", &Value::from("t"));
  assert!(out.contains("    password: changeme\n    token: t\n"), "{out}");
  assert!(out.contains("# left alone entirely"), "{out}");
}

#[test]
fn yaml_creates_the_maps_a_path_names_and_nothing_else() {
  let out = set(YAML, Format::Yaml, "auth.bootstrap.user", &Value::from("root"));
  assert!(out.contains("auth:\n    bootstrap:\n        user: root"), "{out}");
  assert!(out.starts_with("# the database this service talks to\n"), "{out}");
}

#[test]
fn yaml_writes_into_an_empty_document() {
  let out = set("", Format::Yaml, "a.b", &Value::from("c"));
  assert_eq!(out, "a:\n  b: c\n");
}

#[test]
fn yaml_writes_through_an_index_and_a_query() {
  let text = "users:\n  - name: alice\n    token: old\n  - name: bob\n    token: old\n";
  let by_index = set(text, Format::Yaml, "users[1].token", &Value::from("new"));
  assert!(by_index.contains("  - name: bob\n    token: new"), "{by_index}");
  assert!(by_index.contains("  - name: alice\n    token: old"), "{by_index}");

  let by_query = set(text, Format::Yaml, "users[name=\"alice\"].token", &Value::from("new"));
  assert!(by_query.contains("  - name: alice\n    token: new"), "{by_query}");
  assert!(by_query.contains("  - name: bob\n    token: old"), "{by_query}");
}

#[test]
fn yaml_refuses_flow_style_by_name() {
  let mut document = Document::parse("db: {password: changeme}\n", Format::Yaml).unwrap();
  let error = document.set(&parse_path("db").unwrap(), &secret()).unwrap_err().to_string();
  assert!(error.contains("flow style"), "{error}");
  assert!(error.contains("db"), "{error}");
}

#[test]
fn yaml_a_file_with_no_trailing_newline_keeps_none() {
  let out = set("a: 1", Format::Yaml, "a", &Value::Int(2));
  assert_eq!(out, "a: 2");
}

#[test]
fn yaml_reads_back_what_it_wrote() {
  let out = set(YAML, Format::Yaml, "db.password", &secret());
  assert_eq!(get(&out, Format::Yaml, "db.password"), Some(secret()));
}

// --- TOML ---

const TOML: &str = "\
# the database this service talks to
[db]
host   = \"localhost\"  # aligned on purpose
password = \"changeme\"

[cache]
ttl = 60
";

#[test]
fn toml_replacing_a_value_keeps_comments_and_alignment() {
  let out = set(TOML, Format::Toml, "db.password", &secret());
  assert!(out.starts_with("# the database this service talks to\n[db]\nhost   = \"localhost\"  # aligned on purpose\n"), "{out}");
  assert!(out.contains("password = { \".c5encval\" = [\"ecies_x25519\", \"prod\", \"Y2lwaGVy==\"] }"), "{out}");
  assert!(out.contains("[cache]\nttl = 60\n"), "{out}");
}

#[test]
fn toml_creates_a_table_the_path_names() {
  let out = set(TOML, Format::Toml, "auth.bootstrap.user", &Value::from("root"));
  assert!(out.contains("root"), "{out}");
  assert_eq!(get(&out, Format::Toml, "auth.bootstrap.user"), Some(Value::from("root")));
  assert!(out.contains("ttl = 60"), "{out}");
}

#[test]
fn toml_reads_back_what_it_wrote() {
  let out = set(TOML, Format::Toml, "db.password", &secret());
  assert_eq!(get(&out, Format::Toml, "db.password"), Some(secret()));
}

// --- JSON ---

const JSON: &str = "{\n    \"db\": {\n        \"host\": \"localhost\",\n        \"password\": \"changeme\"\n    },\n    \"cache\": {\n        \"ttl\": 60\n    }\n}\n";

#[test]
fn json_replacing_a_value_keeps_the_documents_own_layout() {
  let out = set(JSON, Format::Json, "db.password", &secret());
  assert_eq!(
    out,
    "{\n    \"db\": {\n        \"host\": \"localhost\",\n        \"password\": {\n            \".c5encval\": [\n                \"ecies_x25519\",\n                \"prod\",\n                \"Y2lwaGVy==\"\n            ]\n        }\n    },\n    \"cache\": {\n        \"ttl\": 60\n    }\n}\n"
  );
}

#[test]
fn json_adds_a_key_beside_the_last_one() {
  let out = set(JSON, Format::Json, "db.token", &Value::from("t"));
  assert!(out.contains("\"password\": \"changeme\",\n        \"token\": \"t\""), "{out}");
  assert!(out.contains("\"cache\": {\n        \"ttl\": 60"), "{out}");
}

#[test]
fn json_creates_the_objects_a_path_names() {
  let out = set(JSON, Format::Json, "auth.bootstrap.user", &Value::from("root"));
  assert_eq!(get(&out, Format::Json, "auth.bootstrap.user"), Some(Value::from("root")));
}

#[test]
fn json_writes_into_an_empty_object() {
  let out = set("{}\n", Format::Json, "a.b", &Value::from("c"));
  assert_eq!(get(&out, Format::Json, "a.b"), Some(Value::from("c")));
}

#[test]
fn json_reads_back_what_it_wrote() {
  let out = set(JSON, Format::Json, "db.password", &secret());
  assert_eq!(get(&out, Format::Json, "db.password"), Some(secret()));
}

// --- every format ---

#[test]
fn a_format_is_chosen_by_extension() {
  assert_eq!(Format::of(std::path::Path::new("a/b.toml")), Format::Toml);
  assert_eq!(Format::of(std::path::Path::new("a/b.json")), Format::Json);
  assert_eq!(Format::of(std::path::Path::new("a/b.yaml")), Format::Yaml);
  assert_eq!(Format::of(std::path::Path::new("a/b.yml")), Format::Yaml);
  assert_eq!(Format::of(std::path::Path::new("secrets")), Format::Yaml);
}

#[test]
fn a_path_that_names_nothing_reads_as_nothing() {
  assert_eq!(get(YAML, Format::Yaml, "db.absent"), None);
  assert_eq!(get(TOML, Format::Toml, "db.absent"), None);
  assert_eq!(get(JSON, Format::Json, "db.absent"), None);
}
