use orki_core::manifest::{Compression, Manifest, PrereqMode, StubVariant};

const SAMPLE: &str = r#"
schema = 1

[app]
id = "com.example.myapp"
name = "My App"
version = "1.4.2"
publisher = "Example Sp. z o.o."
url = "https://example.com"
description = "Test app"
icon = "assets/app.ico"
license_file = "LICENSE.md"

[package]
source = "target/release/my-app.exe"
compression = "lzma2"
level = 9
encrypt = false
stub = "full"

[install]
scope = "auto"
allow_dir_change = true
min_os = "10.0.17763"
arch = ["x64", "arm64"]
on_existing = "upgrade"

[install.default_dir]
user = "{{env.LOCALAPPDATA}}/Programs/{{app.name}}"
machine = "{{env.PROGRAMFILES}}/{{app.name}}"

[install.close_running]
by = ["my-app.exe"]
strategy = "restart-manager"
timeout_s = 20

[[components]]
id = "core"
name = "Aplikacja"
required = true

[[components.files]]
from = "target/release/my-app.exe"
to = "my-app.exe"

[[components]]
id = "cli"
name = "Narzędzie CLI"
default = false
add_to_path = true

[[components.files]]
from = "bin/mycli.exe"
to = "bin/mycli.exe"

[[shortcuts]]
location = "start_menu"
name = "{{app.name}}"
target = "my-app.exe"

[[registry]]
hive = "HKCU"
key = "Software/Example/MyApp"

[registry.values]
InstallDir = "{{install.dir}}"

[[protocols]]
scheme = "myapp"
description = "Deep link My App"

[[prerequisites]]
id = "webview2"
detect = "reg('HKLM/SOFTWARE/WOW6432Node/Microsoft/EdgeUpdate/Clients/{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}','pv') >= '110.0'"
mode = "download-bootstrapper"
url = "https://go.microsoft.com/fwlink/p/?LinkId=2124703"
args = ["/silent", "/install"]
reboot_codes = [3010]

[uninstall]
keep_data_prompt = true
data_paths = ["{{env.APPDATA}}/{{app.id}}"]
remove_empty_dirs = true

[update]
tauri_compat = true
mode = "github"
channel = "stable"
endpoint = "https://example.com/updates/{{target}}/{{current_version}}"
pubkey = "dW50cnVzdGVkIGNvbW1lbnQ6"

[ui]
layout = "wizard-modern"
theme = "forge"
languages = ["pl", "en"]
default_language = "en"
pages = ["welcome", "license", "components", "location", "ready", "progress", "finish"]

[ui.size]
w = 760
h = 480
resizable = false

[ui.window]
frameless = true
corner = "round"
backdrop = "mica"
shadow = true

[ui.shader]
background = "shaders/aurora.wgsl"
progress = "builtin:glow-bar"
fps_cap = 60
quality = "auto"

[ui.shader.params]
speed = 0.35
intensity = 0.8

[signing]
command = "signtool sign /fd sha256 /tr http://timestamp.digicert.com /td sha256 /a {file}"

[logging]
level = "info"
telemetry = false
"#;

#[test]
fn parses_full_sample() {
    let m = Manifest::parse(SAMPLE).expect("valid manifest");
    assert_eq!(m.schema, 1);
    assert_eq!(m.app.name, "My App");
    assert_eq!(m.package.compression, Compression::Lzma2);
    assert_eq!(m.package.stub, StubVariant::Full);
    assert_eq!(m.components.len(), 2);
    assert_eq!(m.components[0].id, "core");
    assert!(m.components[0].required);
    assert_eq!(
        m.install
            .close_running
            .as_ref()
            .expect("close_running")
            .timeout_s,
        20
    );
    assert_eq!(m.prerequisites[0].mode, PrereqMode::DownloadBootstrapper);
    assert_eq!(m.ui.shader.as_ref().expect("shader").fps_cap, Some(60));
    assert_eq!(m.shortcuts.len(), 1);
    assert_eq!(m.registry.len(), 1);
    assert_eq!(m.update.channel, "stable");
}
#[test]
fn rejects_bad_schema() {
    let bad = SAMPLE.replace("schema = 1", "schema = 2");
    assert!(Manifest::parse(&bad).is_err());
}

#[test]
fn rejects_zstd_compression() {
    let bad = SAMPLE.replace("compression = \"lzma2\"", "compression = \"zstd\"");
    let err = Manifest::parse(&bad).expect_err("zstd must be rejected");
    assert!(
        err.to_string().contains("zstd"),
        "error should mention zstd: {err}"
    );
}

#[test]
fn rejects_unknown_fields() {
    let bad = SAMPLE.replace(
        "telemetry = false",
        "telemetry = false\ntelemetry_extra = true",
    );
    assert!(Manifest::parse(&bad).is_err());
}

#[test]
fn rejects_duplicate_component_ids() {
    let bad = SAMPLE.replace("id = \"cli\"", "id = \"core\"");
    assert!(Manifest::parse(&bad).is_err());
}

#[test]
fn rejects_empty_app_id() {
    let bad = SAMPLE.replace("id = \"com.example.myapp\"", "id = \"\"");
    assert!(Manifest::parse(&bad).is_err());
}
