use std::fs;
use std::path::PathBuf;
use std::process::Command;

use orki_pack::{AppMeta, PackBuilder, extract_file, read_manifest};

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let path =
            std::env::temp_dir().join(format!("orki-cli-{}-{nanos}-{tag}", std::process::id()));
        fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn repetitive(len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    let mut x: u64 = 0x0451;
    while out.len() < len {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let byte = (x >> 33) as u8;
        for _ in 0..7 {
            out.push(byte);
        }
    }
    out.truncate(len);
    out
}

fn incompressible(len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    let mut x: u64 = 0x9E3779B97F4A7C15;
    while out.len() < len {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        out.push((x >> 33) as u8);
    }
    out
}

fn run_cli(args: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_orki"))
        .args(args)
        .current_dir(std::env::temp_dir())
        .output()
        .expect("spawn orki");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.code().unwrap_or(-1), text)
}

fn stub_exe() -> String {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");
    let exe = if cfg!(windows) {
        "orki-stub.exe"
    } else {
        "orki-stub"
    };
    let path = workspace.join("target").join("debug").join(exe);
    if !path.is_file() {
        let status = Command::new(env!("CARGO"))
            .args(["build", "-p", "orki-stub"])
            .current_dir(workspace)
            .status()
            .expect("run cargo build");
        assert!(status.success(), "cargo build -p orki-stub failed");
    }
    assert!(path.is_file(), "stub not built: {}", path.display());
    path.to_string_lossy().to_string()
}

#[test]
fn pack_and_preview_roundtrip() {
    let tmp = TempDir::new("pack");
    let src = tmp.0.join("src");
    fs::create_dir_all(src.join("bin")).unwrap();
    fs::create_dir_all(src.join("assets")).unwrap();
    let app = repetitive(700 * 1024);
    fs::write(src.join("bin/app.exe"), &app).unwrap();
    let readme = b"welcome to orki\n".repeat(2048);
    fs::write(src.join("assets/readme.txt"), &readme).unwrap();
    let noise = incompressible(96 * 1024);
    fs::write(src.join("assets/noise.bin"), &noise).unwrap();
    fs::write(src.join("assets/empty.lock"), b"").unwrap();

    let out_path = tmp.0.join("out.orkipack");
    let out_str = out_path.to_string_lossy().to_string();
    let src_str = src.to_string_lossy().to_string();

    let (code, text) = run_cli(&["pack", &src_str, "-o", &out_str]);
    assert_eq!(code, 0, "pack failed: {text}");
    assert!(out_path.is_file(), "package missing: {text}");
    assert!(
        text.contains("ratio 0."),
        "compression summary missing: {text}"
    );

    let packed = fs::read(&out_path).unwrap();
    let m = read_manifest(&packed).expect("manifest readable");
    assert_eq!(m.app_id, "com.example.src");
    assert_eq!(m.files.len(), 4);

    let by_path = |p: &str| m.files.iter().find(|f| f.path == p).expect("entry");
    let app_entry = by_path("bin/app.exe");
    assert!(app_entry.chunk_count > 1, "expected multi-chunk file");
    assert_eq!(extract_file(&packed, &m, app_entry).unwrap(), app);
    let readme_entry = by_path("assets/readme.txt");
    assert_eq!(extract_file(&packed, &m, readme_entry).unwrap(), readme);
    let noise_entry = by_path("assets/noise.bin");
    assert_eq!(extract_file(&packed, &m, noise_entry).unwrap(), noise);
    let empty_entry = by_path("assets/empty.lock");
    assert_eq!(extract_file(&packed, &m, empty_entry).unwrap(), b"");
    assert!(m.chunks.iter().any(|c| c.codec == orki_pack::CODEC_STORE));

    let (code, text) = run_cli(&["preview", &out_str]);
    assert_eq!(code, 0, "preview failed: {text}");
    assert!(text.contains("package: src"), "header missing: {text}");
    assert!(text.contains("bin/app.exe"), "file list missing: {text}");
    assert!(text.contains("codec "), "codec summary missing: {text}");
}

#[test]
fn pack_reports_missing_dir() {
    let (code, text) = run_cli(&["pack", "does-not-exist-dir"]);
    assert_eq!(code, 1, "{text}");
}

#[test]
fn preview_reports_bad_package() {
    let tmp = TempDir::new("bad");
    let bad = tmp.0.join("bad.orkipack");
    fs::write(&bad, b"not a package at all").unwrap();
    let bad_str = bad.to_string_lossy().to_string();
    let (code, text) = run_cli(&["preview", &bad_str]);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("bad package"), "{text}");
}

#[test]
fn packbuilder_still_wraps_single_file() {
    let mut b = PackBuilder::new(0);
    b.add_file("app.exe", b"app bytes").unwrap();
    let payload = b.finish(&AppMeta::new("a", "a", "0.1.0"));
    let m = read_manifest(&payload).unwrap();
    assert_eq!(m.files.len(), 1);
}

#[test]
fn inspect_reports_ok_and_corruption() {
    let tmp = TempDir::new("inspect");
    let src = tmp.0.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a.txt"), b"hello orki").unwrap();
    let out_str = tmp.0.join("p.orkipack").to_string_lossy().to_string();
    let src_str = src.to_string_lossy().to_string();
    let (code, text) = run_cli(&["pack", &src_str, "-o", &out_str]);
    assert_eq!(code, 0, "{text}");

    let (code, text) = run_cli(&["inspect", &out_str]);
    assert_eq!(code, 0, "{text}");
    assert!(text.contains("integrity: ok"), "{text}");

    let mut data = fs::read(&out_str).unwrap();
    let mid = data.len() / 2;
    data[mid] ^= 0xff;
    let corrupt = tmp.0.join("corrupt.orkipack");
    fs::write(&corrupt, &data).unwrap();
    let corrupt_str = corrupt.to_string_lossy().to_string();
    let (code, text) = run_cli(&["inspect", &corrupt_str]);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("FAIL"), "{text}");
}

#[test]
fn pack_with_stub_produces_installable_exe() {
    let tmp = TempDir::new("stub");
    let src = tmp.0.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("app.exe"), b"app bytes here").unwrap();
    let out = tmp.0.join("Setup.exe");
    let out_str = out.to_string_lossy().to_string();
    let src_str = src.to_string_lossy().to_string();
    let stub = stub_exe();
    let (code, text) = run_cli(&["pack", &src_str, "--stub", &stub, "-o", &out_str]);
    assert_eq!(code, 0, "{text}");
    assert!(text.contains("stub attached"), "{text}");

    let installer = fs::read(&out).unwrap();
    let m = read_manifest(&installer).expect("manifest readable after overlay");
    assert_eq!(
        extract_file(&installer, &m, &m.files[0]).unwrap(),
        b"app bytes here"
    );

    let headless = Command::new(&out)
        .arg("--version")
        .output()
        .expect("run Setup.exe");
    assert!(headless.status.success(), "--version failed");
    assert!(
        String::from_utf8_lossy(&headless.stdout).contains("orki-stub"),
        "version output: {}",
        String::from_utf8_lossy(&headless.stdout)
    );
    let list = Command::new(&out)
        .arg("--list")
        .output()
        .expect("run Setup.exe");
    assert!(list.status.success(), "--list failed on packed installer");
    assert!(
        String::from_utf8_lossy(&list.stdout).contains("app.exe"),
        "--list output: {}",
        String::from_utf8_lossy(&list.stdout)
    );
}

#[test]
fn stub_is_built_for_tests() {
    let p = stub_exe();
    assert!(PathBuf::from(&p).is_file(), "{p}");
}

#[test]
fn wrap_with_stub_attaches_overlay() {
    let tmp = TempDir::new("wrap");
    let app = tmp.0.join("hello.exe");
    fs::write(&app, b"hello binary").unwrap();
    let out = tmp.0.join("Setup.exe");
    let out_str = out.to_string_lossy().to_string();
    let app_str = app.to_string_lossy().to_string();
    let (code, text) = run_cli(&[
        "wrap",
        &app_str,
        "--stub",
        &stub_exe(),
        "-o",
        &out_str,
        "--name",
        "Hello",
    ]);
    assert_eq!(code, 0, "{text}");
    assert!(text.contains("stub attached"), "{text}");

    let installer = fs::read(&out).unwrap();
    let m = read_manifest(&installer).unwrap();
    assert_eq!(m.app_name, "Hello");
    assert_eq!(
        extract_file(&installer, &m, &m.files[0]).unwrap(),
        b"hello binary"
    );
}
