use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    #[serde(default)]
    pub app: App,
    #[serde(default)]
    pub package: Package,
    #[serde(default)]
    pub install: Install,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<Component>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shortcuts: Vec<Shortcut>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub registry: Vec<RegistryEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file_associations: Vec<FileAssociation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protocols: Vec<Protocol>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<EnvVar>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prerequisites: Vec<Prerequisite>,
    #[serde(default)]
    pub uninstall: Uninstall,
    #[serde(default)]
    pub update: Update,
    #[serde(default)]
    pub ui: Ui,
    #[serde(default)]
    pub hooks: Hooks,
    #[serde(default)]
    pub signing: Signing,
    #[serde(default)]
    pub logging: Logging,
    #[serde(default)]
    pub remote: Option<Remote>,
}

impl Manifest {
    pub fn parse(text: &str) -> Result<Manifest> {
        let m: Manifest = toml::from_str(text).map_err(|e| Error::Config(format!("toml: {e}")))?;
        m.validate()?;
        Ok(m)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != 1 {
            return Err(Error::Config(format!(
                "unsupported schema version: {}",
                self.schema
            )));
        }
        if self.package.compression == Compression::Zstd {
            return Err(Error::Config(
                "package.compression = zstd is reserved but not implemented yet (ADR-0002); use lzma2, brotli or store".to_string(),
            ));
        }
        if self.app.id.trim().is_empty() {
            return Err(Error::Config("app.id must not be empty".to_string()));
        }
        if self.app.name.trim().is_empty() {
            return Err(Error::Config("app.name must not be empty".to_string()));
        }
        if self.app.version.trim().is_empty() {
            return Err(Error::Config("app.version must not be empty".to_string()));
        }
        let mut ids: Vec<&str> = Vec::new();
        for c in &self.components {
            if c.id.trim().is_empty() {
                return Err(Error::Config("component id must not be empty".to_string()));
            }
            if ids.contains(&c.id.as_str()) {
                return Err(Error::Config(format!("duplicate component id: {}", c.id)));
            }
            ids.push(&c.id);
        }
        for s in &self.shortcuts {
            if s.target.trim().is_empty() {
                return Err(Error::Config(format!(
                    "shortcut {} has an empty target",
                    s.name
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct App {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub publisher: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub support_url: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub license_file: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub compression: Compression,
    #[serde(default)]
    pub level: Option<u8>,
    #[serde(default)]
    pub encrypt: bool,
    #[serde(default)]
    pub stub: StubVariant,
}

impl Default for Package {
    fn default() -> Self {
        Self {
            source: None,
            compression: Compression::Lzma2,
            level: None,
            encrypt: false,
            stub: StubVariant::Full,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Compression {
    #[default]
    Lzma2,
    Brotli,
    Zstd,
    Store,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StubVariant {
    #[default]
    Full,
    Lite,
    Headless,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Install {
    #[serde(default)]
    pub scope: ScopeAuto,
    #[serde(default)]
    pub default_dir: Option<DefaultDir>,
    #[serde(default = "default_true")]
    pub allow_dir_change: bool,
    #[serde(default)]
    pub min_os: Option<String>,
    #[serde(default)]
    pub arch: Vec<Arch>,
    #[serde(default)]
    pub close_running: Option<CloseRunning>,
    #[serde(default)]
    pub single_instance_mutex: Option<String>,
    #[serde(default)]
    pub on_existing: OnExisting,
}

impl Default for Install {
    fn default() -> Self {
        Self {
            scope: ScopeAuto::Auto,
            default_dir: None,
            allow_dir_change: true,
            min_os: None,
            arch: Vec::new(),
            close_running: None,
            single_instance_mutex: None,
            on_existing: OnExisting::Upgrade,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScopeAuto {
    #[default]
    Auto,
    User,
    Machine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Arch {
    X64,
    Arm64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefaultDir {
    pub user: String,
    pub machine: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloseRunning {
    #[serde(default)]
    pub by: Vec<String>,
    #[serde(default)]
    pub strategy: CloseStrategy,
    #[serde(default = "default_timeout")]
    pub timeout_s: u64,
}

impl Default for CloseRunning {
    fn default() -> Self {
        Self {
            by: Vec::new(),
            strategy: CloseStrategy::RestartManager,
            timeout_s: default_timeout(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CloseStrategy {
    #[default]
    RestartManager,
    Terminate,
    Prompt,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnExisting {
    #[default]
    Upgrade,
    Repair,
    Ask,
    Refuse,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub size_hint: Option<String>,
    #[serde(default)]
    pub files: Vec<FileRule>,
    #[serde(default)]
    pub add_to_path: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileRule {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shortcut {
    pub location: ShortcutLocation,
    pub name: String,
    pub target: String,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub folder: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutLocation {
    #[default]
    StartMenu,
    Desktop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryEntry {
    pub hive: Hive,
    pub key: String,
    #[serde(default)]
    pub values: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Hive {
    #[default]
    Hkcu,
    Hklm,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileAssociation {
    pub ext: String,
    pub prog_id: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Protocol {
    pub scheme: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvVar {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub scope: EnvScope,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EnvScope {
    #[default]
    User,
    Machine,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prerequisite {
    pub id: String,
    pub detect: String,
    #[serde(default)]
    pub mode: PrereqMode,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub reboot_codes: Vec<u32>,
}

impl Default for Prerequisite {
    fn default() -> Self {
        Self {
            id: String::new(),
            detect: String::new(),
            mode: PrereqMode::DownloadBootstrapper,
            url: None,
            args: Vec::new(),
            reboot_codes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrereqMode {
    #[default]
    DownloadBootstrapper,
    EmbedBootstrapper,
    Offline,
    Fixed,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Uninstall {
    #[serde(default = "default_true")]
    pub keep_data_prompt: bool,
    #[serde(default)]
    pub data_paths: Vec<String>,
    #[serde(default = "default_true")]
    pub remove_empty_dirs: bool,
}

impl Default for Uninstall {
    fn default() -> Self {
        Self {
            keep_data_prompt: true,
            data_paths: Vec::new(),
            remove_empty_dirs: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Update {
    #[serde(default)]
    pub tauri_compat: bool,
    #[serde(default)]
    pub mode: UpdateMode,
    #[serde(default = "default_channel")]
    pub channel: String,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub pubkey: Option<String>,
}

impl Default for Update {
    fn default() -> Self {
        Self {
            tauri_compat: false,
            mode: UpdateMode::Disabled,
            channel: default_channel(),
            endpoint: None,
            pubkey: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateMode {
    #[serde(rename = "none")]
    #[default]
    Disabled,
    Static,
    Github,
    Folder,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ui {
    #[serde(default)]
    pub layout: Layout,
    #[serde(default)]
    pub size: Option<WindowSize>,
    #[serde(default)]
    pub window: WindowOpts,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub mode: UiMode,
    #[serde(default = "default_languages")]
    pub languages: Vec<String>,
    #[serde(default = "default_language")]
    pub default_language: String,
    #[serde(default)]
    pub show_language_picker: bool,
    #[serde(default = "default_pages")]
    pub pages: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom: Vec<CustomPage>,
    #[serde(default)]
    pub shader: Option<ShaderConfig>,
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            layout: Layout::WizardModern,
            size: None,
            window: WindowOpts::default(),
            theme: default_theme(),
            mode: UiMode::Auto,
            languages: default_languages(),
            default_language: default_language(),
            show_language_picker: false,
            pages: default_pages(),
            custom: Vec::new(),
            shader: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Layout {
    WizardClassic,
    #[default]
    WizardModern,
    Oneclick,
    Splash,
    Minimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowSize {
    pub w: u32,
    pub h: u32,
    #[serde(default = "default_false")]
    pub resizable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowOpts {
    #[serde(default = "default_true")]
    pub frameless: bool,
    #[serde(default)]
    pub corner: Option<Corner>,
    #[serde(default)]
    pub backdrop: Option<Backdrop>,
    #[serde(default = "default_true")]
    pub shadow: bool,
}

impl Default for WindowOpts {
    fn default() -> Self {
        Self {
            frameless: true,
            corner: None,
            backdrop: None,
            shadow: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Corner {
    #[default]
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Backdrop {
    #[default]
    Mica,
    Acrylic,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiMode {
    #[default]
    Auto,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomPage {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub after: Option<String>,
    #[serde(default)]
    pub when: Option<String>,
    #[serde(default)]
    pub layout: Vec<toml::Table>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShaderConfig {
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default)]
    pub params: BTreeMap<String, toml::Value>,
    #[serde(default)]
    pub progress: Option<String>,
    #[serde(default)]
    pub fps_cap: Option<u32>,
    #[serde(default)]
    pub quality: Quality,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    #[default]
    Auto,
    Low,
    High,
    Off,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hooks {
    #[serde(default)]
    pub pre_install: Option<Hook>,
    #[serde(default)]
    pub post_install: Option<Hook>,
    #[serde(default)]
    pub pre_uninstall: Option<Hook>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hook {
    #[serde(default)]
    pub run: Option<String>,
    #[serde(default)]
    pub exec: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default, rename = "as")]
    pub as_user: Option<String>,
    #[serde(default)]
    pub wait: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Signing {
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub update_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Logging {
    #[serde(default = "default_level")]
    pub level: String,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub telemetry: bool,
}

impl Default for Logging {
    fn default() -> Self {
        Self {
            level: default_level(),
            file: None,
            telemetry: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Remote {
    pub manifest: String,
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub schema: BTreeMap<String, String>,
    #[serde(default = "default_true")]
    pub require_signature: bool,
    #[serde(default = "default_remote_timeout")]
    pub timeout_s: u64,
    #[serde(default = "default_remote_failure")]
    pub on_failure: String,
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

fn default_timeout() -> u64 {
    20
}

fn default_channel() -> String {
    "stable".to_string()
}

fn default_theme() -> String {
    "forge".to_string()
}

fn default_language() -> String {
    "en".to_string()
}

fn default_languages() -> Vec<String> {
    vec!["en".to_string()]
}

fn default_pages() -> Vec<String> {
    [
        "welcome",
        "license",
        "components",
        "location",
        "ready",
        "progress",
        "finish",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn default_level() -> String {
    "info".to_string()
}

fn default_remote_timeout() -> u64 {
    3
}

fn default_remote_failure() -> String {
    "use_cached_then_defaults".to_string()
}
