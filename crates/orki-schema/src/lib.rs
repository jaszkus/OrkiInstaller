use serde_json::{Map, Value, json};

pub fn manifest_schema_json() -> String {
    let mut root = Map::new();
    root.insert(
        "$schema".into(),
        json!("https://json-schema.org/draft/2020-12/schema"),
    );
    root.insert("title".into(), json!("Orki Installer Manifest"));
    root.insert("type".into(), json!("object"));
    root.insert("additionalProperties".into(), json!(false));

    let mut props = Map::new();
    props.insert("schema".into(), json!({ "type": "integer", "const": 1 }));

    props.insert(
        "app".into(),
        object(&[
            ("id", string_prop()),
            ("name", string_prop()),
            ("version", string_prop()),
            ("publisher", string_prop()),
            ("url", string_prop()),
            ("support_url", string_prop()),
            ("description", string_prop()),
            ("icon", string_prop()),
            ("license_file", string_prop()),
        ]),
    );

    props.insert(
        "package".into(),
        object(&[
            ("source", string_prop()),
            (
                "compression",
                enum_prop(&["lzma2", "brotli", "zstd", "store"]),
            ),
            (
                "level",
                json!({ "type": "integer", "minimum": 0, "maximum": 22 }),
            ),
            ("encrypt", json!({ "type": "boolean" })),
            ("stub", enum_prop(&["full", "lite", "headless"])),
        ]),
    );

    props.insert(
        "install".into(),
        object(&[
            ("scope", enum_prop(&["user", "machine", "auto"])),
            ("allow_dir_change", json!({ "type": "boolean" })),
            ("min_os", string_prop()),
            (
                "arch",
                json!({ "type": "array", "items": { "enum": ["x64", "arm64"] } }),
            ),
            (
                "on_existing",
                enum_prop(&["upgrade", "repair", "ask", "refuse"]),
            ),
            ("single_instance_mutex", string_prop()),
            (
                "default_dir",
                object(&[("user", string_prop()), ("machine", string_prop())]),
            ),
            (
                "close_running",
                object(&[
                    (
                        "by",
                        json!({ "type": "array", "items": { "type": "string" } }),
                    ),
                    (
                        "strategy",
                        enum_prop(&["restart-manager", "terminate", "prompt"]),
                    ),
                    ("timeout_s", json!({ "type": "integer" })),
                ]),
            ),
        ]),
    );

    props.insert(
        "components".into(),
        array_of(object(&[
            ("id", string_prop()),
            ("name", string_prop()),
            ("default", json!({ "type": "boolean" })),
            ("required", json!({ "type": "boolean" })),
            ("size_hint", string_prop()),
            ("add_to_path", json!({ "type": "boolean" })),
            (
                "files",
                array_of(object(&[("from", string_prop()), ("to", string_prop())])),
            ),
        ])),
    );

    props.insert(
        "shortcuts".into(),
        array_of(object(&[
            ("location", enum_prop(&["start_menu", "desktop"])),
            ("name", string_prop()),
            ("target", string_prop()),
            ("default", json!({ "type": "boolean" })),
            ("optional", json!({ "type": "boolean" })),
            ("folder", string_prop()),
        ])),
    );

    props.insert(
        "registry".into(),
        array_of(object(&[
            ("hive", enum_prop(&["HKCU", "HKLM"])),
            ("key", string_prop()),
            (
                "values",
                json!({ "type": "object", "additionalProperties": { "type": "string" } }),
            ),
        ])),
    );

    props.insert(
        "file_associations".into(),
        array_of(object(&[
            ("ext", string_prop()),
            ("prog_id", string_prop()),
            ("description", string_prop()),
            ("icon", string_prop()),
        ])),
    );

    props.insert(
        "protocols".into(),
        array_of(object(&[
            ("scheme", string_prop()),
            ("description", string_prop()),
        ])),
    );

    props.insert(
        "env".into(),
        array_of(object(&[
            ("name", string_prop()),
            ("value", string_prop()),
            ("scope", enum_prop(&["user", "machine"])),
        ])),
    );

    props.insert(
        "prerequisites".into(),
        array_of(object(&[
            ("id", string_prop()),
            ("detect", string_prop()),
            (
                "mode",
                enum_prop(&[
                    "download-bootstrapper",
                    "embed-bootstrapper",
                    "offline",
                    "fixed",
                    "skip",
                ]),
            ),
            ("url", string_prop()),
            (
                "args",
                json!({ "type": "array", "items": { "type": "string" } }),
            ),
            (
                "reboot_codes",
                json!({ "type": "array", "items": { "type": "integer" } }),
            ),
        ])),
    );

    props.insert(
        "uninstall".into(),
        object(&[
            ("keep_data_prompt", json!({ "type": "boolean" })),
            (
                "data_paths",
                json!({ "type": "array", "items": { "type": "string" } }),
            ),
            ("remove_empty_dirs", json!({ "type": "boolean" })),
        ]),
    );

    props.insert(
        "update".into(),
        object(&[
            ("tauri_compat", json!({ "type": "boolean" })),
            ("mode", enum_prop(&["none", "static", "github", "folder"])),
            ("channel", string_prop()),
            ("endpoint", string_prop()),
            ("pubkey", string_prop()),
        ]),
    );

    props.insert(
        "ui".into(),
        object(&[
            (
                "layout",
                enum_prop(&[
                    "wizard-classic",
                    "wizard-modern",
                    "oneclick",
                    "splash",
                    "minimal",
                ]),
            ),
            ("theme", string_prop()),
            ("mode", enum_prop(&["auto", "light", "dark"])),
            (
                "languages",
                json!({ "type": "array", "items": { "type": "string" } }),
            ),
            ("default_language", string_prop()),
            ("show_language_picker", json!({ "type": "boolean" })),
            (
                "pages",
                json!({ "type": "array", "items": { "type": "string" } }),
            ),
            (
                "size",
                object(&[
                    ("w", json!({ "type": "integer" })),
                    ("h", json!({ "type": "integer" })),
                    ("resizable", json!({ "type": "boolean" })),
                ]),
            ),
            (
                "window",
                object(&[
                    ("frameless", json!({ "type": "boolean" })),
                    ("corner", enum_prop(&["round", "square"])),
                    ("backdrop", enum_prop(&["mica", "acrylic"])),
                    ("shadow", json!({ "type": "boolean" })),
                ]),
            ),
            (
                "shader",
                object(&[
                    ("background", string_prop()),
                    ("progress", string_prop()),
                    ("fps_cap", json!({ "type": "integer" })),
                    ("quality", enum_prop(&["auto", "low", "high", "off"])),
                    ("params", json!({ "type": "object" })),
                ]),
            ),
        ]),
    );

    props.insert(
        "hooks".into(),
        object(&[
            ("pre_install", hook_prop()),
            ("post_install", hook_prop()),
            ("pre_uninstall", hook_prop()),
        ]),
    );

    props.insert(
        "signing".into(),
        object(&[("command", string_prop()), ("update_key", string_prop())]),
    );

    props.insert(
        "logging".into(),
        object(&[
            ("level", string_prop()),
            ("file", string_prop()),
            ("telemetry", json!({ "type": "boolean" })),
        ]),
    );

    props.insert(
        "remote".into(),
        object(&[
            ("manifest", string_prop()),
            (
                "allow",
                json!({ "type": "array", "items": { "type": "string" } }),
            ),
            ("schema", json!({ "type": "object" })),
            ("require_signature", json!({ "type": "boolean" })),
            ("timeout_s", json!({ "type": "integer" })),
            ("on_failure", string_prop()),
        ]),
    );

    root.insert("properties".into(), Value::Object(props));
    serde_json::to_string_pretty(&Value::Object(root)).unwrap_or_else(|_| "{}".to_string())
}

fn object(fields: &[(&str, Value)]) -> Value {
    let mut m = Map::new();
    m.insert("type".into(), json!("object"));
    let mut props = Map::new();
    for (k, v) in fields {
        props.insert((*k).to_string(), v.clone());
    }
    m.insert("properties".into(), Value::Object(props));
    m.insert("additionalProperties".into(), json!(false));
    Value::Object(m)
}

fn array_of(item: Value) -> Value {
    json!({ "type": "array", "items": item })
}

fn string_prop() -> Value {
    json!({ "type": "string" })
}

fn enum_prop(values: &[&str]) -> Value {
    json!({ "enum": values })
}

fn hook_prop() -> Value {
    object(&[
        ("run", string_prop()),
        ("exec", string_prop()),
        (
            "args",
            json!({ "type": "array", "items": { "type": "string" } }),
        ),
        ("as", string_prop()),
        ("wait", json!({ "type": "boolean" })),
    ])
}

pub struct SchemaInfo {
    pub title: String,
    pub schema_version: u32,
}

pub fn schema_info() -> SchemaInfo {
    SchemaInfo {
        title: "Orki Installer Manifest".to_string(),
        schema_version: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::{manifest_schema_json, schema_info};

    #[test]
    fn schema_generation() {
        let s = manifest_schema_json();
        assert!(s.contains("Orki Installer Manifest"));
        assert!(s.contains("\"lzma2\""));
        assert!(s.contains("wizard-modern"));
        assert!(s.contains("\"HKCU\""));
        assert_eq!(schema_info().schema_version, 1);
    }
}
