use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ImportError {
    #[error("cannot read tauri.conf.json: {0}")]
    Read(String),
    #[error("invalid tauri.conf.json: {0}")]
    Parse(String),
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TauriConfig {
    #[serde(default)]
    product_name: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    identifier: Option<String>,
    #[serde(default)]
    bundle: TauriBundle,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct TauriBundle {
    #[serde(default)]
    publisher: Option<String>,
    #[serde(default)]
    homepage: Option<String>,
    #[serde(default)]
    icon: Vec<String>,
    #[serde(default)]
    license_file: Option<String>,
    #[serde(default)]
    resources: Option<serde_json::Value>,
    #[serde(default)]
    windows: Option<TauriWindows>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct TauriWindows {
    #[serde(default)]
    webview_install_mode: Option<serde_json::Value>,
    #[serde(default)]
    nsis: Option<TauriNsis>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct TauriNsis {
    #[serde(default)]
    install_mode: Option<String>,
    #[serde(default)]
    languages: Option<Vec<String>>,
    #[serde(default)]
    display_language_selector: Option<bool>,
}

pub struct ImportResult {
    pub orki_toml: String,
    pub warnings: Vec<String>,
    pub icons: Vec<String>,
    pub resources: Option<serde_json::Value>,
    pub webview_install_mode: Option<serde_json::Value>,
    pub display_language_selector: bool,
}

pub fn import_from_str(json: &str) -> Result<ImportResult, ImportError> {
    let cfg: TauriConfig =
        serde_json::from_str(json).map_err(|e| ImportError::Parse(e.to_string()))?;
    let mut warnings = Vec::new();

    let name = cfg.product_name.clone().unwrap_or_default();
    if name.is_empty() {
        warnings.push("productName missing; set [app].name manually".to_string());
    }
    if cfg.version.is_none() {
        warnings.push("version missing; set [app].version manually".to_string());
    }
    let id = cfg.identifier.clone().unwrap_or_default();
    if id.is_empty() {
        warnings.push("identifier missing; set [app].id manually".to_string());
    }

    let install_mode = cfg
        .bundle
        .windows
        .as_ref()
        .and_then(|w| w.nsis.as_ref())
        .and_then(|n| n.install_mode.as_deref())
        .unwrap_or("currentUser");

    let scope = match install_mode {
        "perMachine" => "machine",
        "both" => "auto",
        _ => "user",
    };

    let nsis_languages = cfg
        .bundle
        .windows
        .as_ref()
        .and_then(|w| w.nsis.as_ref())
        .and_then(|n| n.languages.clone())
        .unwrap_or_default();

    let languages: Vec<String> = nsis_languages
        .iter()
        .map(|l| match l.as_str() {
            "Polish" => "pl".to_string(),
            "English" => "en".to_string(),
            "German" => "de".to_string(),
            "Ukrainian" => "uk".to_string(),
            other => {
                warnings.push(format!("unmapped NSIS language: {other}"));
                other.to_lowercase()
            }
        })
        .collect();

    let mut toml_out = String::new();
    toml_out.push_str("schema = 1\n\n[app]\n");
    toml_out.push_str(&format!("id = {}\n", toml_quote(&id)));
    toml_out.push_str(&format!("name = {}\n", toml_quote(&name)));
    toml_out.push_str(&format!(
        "version = {}\n",
        toml_quote(cfg.version.as_deref().unwrap_or("0.0.0"))
    ));
    if let Some(p) = &cfg.bundle.publisher {
        toml_out.push_str(&format!("publisher = {}\n", toml_quote(p)));
    }
    if let Some(h) = &cfg.bundle.homepage {
        toml_out.push_str(&format!("url = {}\n", toml_quote(h)));
    }
    if let Some(l) = &cfg.bundle.license_file {
        toml_out.push_str(&format!("license_file = {}\n", toml_quote(l)));
    }
    toml_out.push_str("\n[install]\n");
    toml_out.push_str(&format!("scope = \"{scope}\"\n"));
    toml_out.push_str("\n[ui]\n");
    let mut langs = languages.clone();
    if langs.is_empty() {
        langs.push("en".to_string());
    }
    toml_out.push_str(&format!(
        "languages = [{}]\n",
        langs
            .iter()
            .map(|l| toml_quote(l))
            .collect::<Vec<_>>()
            .join(", ")
    ));

    let icons = cfg.bundle.icon.clone();
    if icons.is_empty() {
        warnings.push("bundle.icon missing; set [app].icon manually".to_string());
    }
    let display_language_selector = cfg
        .bundle
        .windows
        .as_ref()
        .and_then(|w| w.nsis.as_ref())
        .and_then(|n| n.display_language_selector)
        .unwrap_or(false);

    Ok(ImportResult {
        orki_toml: toml_out,
        warnings,
        icons,
        resources: cfg.bundle.resources.clone(),
        webview_install_mode: cfg
            .bundle
            .windows
            .as_ref()
            .and_then(|w| w.webview_install_mode.clone()),
        display_language_selector,
    })
}

fn toml_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

pub fn latest_json(
    version: &str,
    notes: &str,
    url: &str,
    signature: &str,
) -> BTreeMap<String, serde_json::Value> {
    let mut root = BTreeMap::new();
    root.insert(
        "version".to_string(),
        serde_json::Value::String(version.to_string()),
    );
    root.insert(
        "notes".to_string(),
        serde_json::Value::String(notes.to_string()),
    );
    root.insert(
        "pub_date".to_string(),
        serde_json::Value::String(now_rfc3339()),
    );
    let mut platform = BTreeMap::new();
    platform.insert(
        "url".to_string(),
        serde_json::Value::String(url.to_string()),
    );
    platform.insert(
        "signature".to_string(),
        serde_json::Value::String(signature.to_string()),
    );
    let mut platforms = BTreeMap::new();
    platforms.insert(
        "windows-x86_64".to_string(),
        serde_json::Value::Object(platform.into_iter().collect()),
    );
    root.insert(
        "platforms".to_string(),
        serde_json::Value::Object(platforms.into_iter().collect()),
    );
    root
}

fn now_rfc3339() -> String {
    "1970-01-01T00:00:00Z".to_string()
}

#[cfg(test)]
mod tests {
    use super::{import_from_str, latest_json};

    const SAMPLE: &str = r#"{
      "productName": "My App",
      "version": "1.4.2",        "identifier": "com.example.myapp",
        "bundle": {
        "publisher": "Example Sp. z o.o.",
        "homepage": "https://example.com",
        "licenseFile": "LICENSE.md",
        "icon": ["icons/icon.ico", "icons/32x32.png"],
        "windows": {
          "nsis": { "installMode": "perMachine", "languages": ["English", "Polish"] }
        }
      }
    }"#;

    #[test]
    fn imports_full_config() {
        let r = import_from_str(SAMPLE).expect("import ok");
        assert!(r.warnings.is_empty(), "warnings: {:?}", r.warnings);
        assert!(!r.display_language_selector);
        assert_eq!(r.icons, vec!["icons/icon.ico", "icons/32x32.png"]);
        assert!(r.orki_toml.contains("id = \"com.example.myapp\""));
        assert!(r.orki_toml.contains("name = \"My App\""));
        assert!(r.orki_toml.contains("version = \"1.4.2\""));
        assert!(r.orki_toml.contains("scope = \"machine\""));
        assert!(r.orki_toml.contains("\"pl\""));
        assert!(r.orki_toml.contains("publisher = \"Example Sp. z o.o.\""));
    }

    #[test]
    fn warns_on_missing_fields() {
        let r = import_from_str("{}").expect("empty import ok");
        assert!(r.warnings.len() >= 4);
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(import_from_str("{broken").is_err());
    }

    #[test]
    fn latest_json_shape() {
        let j = latest_json("1.5.0", "notes", "https://x/Setup.exe", "sig==");
        assert_eq!(j["version"], "1.5.0");
        let s = serde_json::to_string(&j).unwrap();
        assert!(s.contains("windows-x86_64"));
        assert!(s.contains("signature"));
    }
}
