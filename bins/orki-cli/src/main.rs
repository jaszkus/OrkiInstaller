use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    exit(run(&args));
}

fn run(args: &[String]) -> i32 {
    let Some(cmd) = args.first().map(String::as_str) else {
        print_usage();
        return 2;
    };
    let rest = &args[1..];
    match cmd {
        "wrap" => cmd_wrap(rest),
        "init" => cmd_init(rest),
        "check" => cmd_check(rest),
        "schema" => cmd_schema(rest),
        "doctor" => cmd_doctor(),
        "pack" => cmd_pack(rest),
        "preview" => cmd_preview(rest),
        "inspect" => cmd_inspect(rest),
        "sign" | "diff" | "test" | "release" => {
            eprintln!("{cmd}: not implemented yet (roadmap phase 1)");
            69
        }
        "help" | "--help" | "-h" => {
            print_usage();
            0
        }
        other => {
            eprintln!("unknown command: {other}");
            print_usage();
            2
        }
    }
}

fn print_usage() {
    println!("orki — installer builder");
    println!();
    println!("  wrap <app.exe> -o <Setup.exe> [--name N] [--id ID] [--version V]");
    println!("  pack <dir> -o <out.orkipack> [--name N] [--id ID] [--version V]");
    println!("      [--codec auto|store|lzma2|brotli] [--stub <orki-stub.exe>]");
    println!("  wrap <app.exe> accepts the same optional flags as pack");
    println!("  preview <out.orkipack>");
    println!("  inspect <Setup.exe|out.orkipack>");
    println!("  init [--tauri <tauri.conf.json>]");
    println!("  check [orki.toml]");
    println!("  schema");
    println!("  doctor");
}

fn cmd_wrap(rest: &[String]) -> i32 {
    let mut positional: Vec<String> = Vec::new();
    let mut out: Option<String> = None;
    let mut name: Option<String> = None;
    let mut id: Option<String> = None;
    let mut version: Option<String> = None;
    let mut stub: Option<String> = None;
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-o" | "--out" => out = it.next().cloned(),
            "--name" => name = it.next().cloned(),
            "--id" => id = it.next().cloned(),
            "--version" => version = it.next().cloned(),
            "--stub" => stub = it.next().cloned(),
            other => positional.push(other.to_string()),
        }
    }
    let Some(source) = positional.first() else {
        eprintln!("wrap: source path required");
        return 2;
    };
    let src = PathBuf::from(source);
    if !src.is_file() {
        eprintln!("wrap: not a file: {}", src.display());
        return 1;
    }
    let data = match std::fs::read(&src) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("wrap: cannot read {}: {e}", src.display());
            return 1;
        }
    };
    let stem = src
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("app")
        .to_string();
    let app_name = name.unwrap_or_else(|| stem.clone());
    let app_id = id.unwrap_or_else(|| format!("com.example.{stem}"));
    let app_version = version.unwrap_or_else(|| "0.0.0".to_string());

    let stub_bytes = match &stub {
        Some(p) => match std::fs::read(p) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("wrap: cannot read stub {p}: {e}");
                return 1;
            }
        },
        None => Vec::new(),
    };
    let mut builder = orki_pack::PackBuilder::new(0);
    let dest_name = format!("{stem}.exe");
    if let Err(e) = builder.add_file(&dest_name, &data) {
        eprintln!("wrap: {e}");
        return 1;
    }
    let payload = builder.finish(&orki_pack::AppMeta::new(app_id, app_name, app_version));
    let mut final_bytes = stub_bytes;
    final_bytes.extend_from_slice(&payload);
    let out_path = out
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("Setup.exe"));
    if let Err(e) = std::fs::write(&out_path, &final_bytes) {
        eprintln!("wrap: cannot write {}: {e}", out_path.display());
        return 1;
    }
    let m = match orki_pack::read_manifest(&final_bytes) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("wrap: self-check failed: {e}");
            return 1;
        }
    };
    if let Err(e) = orki_pack::verify_integrity(&final_bytes, &m) {
        eprintln!("wrap: self-check failed: {e}");
        return 1;
    }
    let mode = if stub.is_some() {
        "stub attached"
    } else {
        "payload only, no stub"
    };
    println!(
        "wrote {} ({} bytes, {mode})",
        out_path.display(),
        final_bytes.len()
    );
    0
}

fn cmd_pack(rest: &[String]) -> i32 {
    let mut positional: Vec<String> = Vec::new();
    let mut out: Option<String> = None;
    let mut name: Option<String> = None;
    let mut id: Option<String> = None;
    let mut version: Option<String> = None;
    let mut codec_arg: Option<String> = None;
    let mut stub: Option<String> = None;
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-o" | "--out" => out = it.next().cloned(),
            "--name" => name = it.next().cloned(),
            "--id" => id = it.next().cloned(),
            "--version" => version = it.next().cloned(),
            "--codec" => codec_arg = it.next().cloned(),
            "--stub" => stub = it.next().cloned(),
            other => positional.push(other.to_string()),
        }
    }
    let Some(source) = positional.first() else {
        eprintln!("pack: source directory required");
        return 2;
    };
    let dir = PathBuf::from(source);
    if !dir.is_dir() {
        eprintln!("pack: not a directory: {}", dir.display());
        return 1;
    }
    let policy = match codec_arg.as_deref() {
        None | Some("auto") => orki_pack::CodecPolicy::Auto,
        Some("store") => orki_pack::CodecPolicy::Store,
        Some("lzma2") => orki_pack::CodecPolicy::Lzma2,
        Some("brotli") => orki_pack::CodecPolicy::Brotli,
        Some(c) => {
            eprintln!("pack: unknown codec: {c} (auto|store|lzma2|brotli)");
            return 2;
        }
    };
    let stem = dir
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("app")
        .to_string();
    let app_name = name.unwrap_or_else(|| stem.clone());
    let app_id = id.unwrap_or_else(|| format!("com.example.{stem}"));
    let app_version = version.unwrap_or_else(|| "0.0.0".to_string());

    let out_path = out
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("{stem}.orkipack")));
    let mut files = Vec::new();
    if let Err(e) = collect_files(&dir, &dir, &mut files) {
        eprintln!("pack: cannot walk {}: {e}", dir.display());
        return 1;
    }
    files.retain(|(_, abs)| *abs != out_path);
    if files.is_empty() {
        eprintln!("pack: no files in {}", dir.display());
        return 1;
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let stub_bytes = match &stub {
        Some(p) => match std::fs::read(p) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("pack: cannot read stub {p}: {e}");
                return 1;
            }
        },
        None => Vec::new(),
    };
    let mut builder = orki_pack::PackBuilder::new(0).codec(policy);
    let mut raw_total = 0u64;
    for (rel, abs) in &files {
        let data = match std::fs::read(abs) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("pack: cannot read {}: {e}", abs.display());
                return 1;
            }
        };
        raw_total += data.len() as u64;
        if let Err(e) = builder.add_file(rel, &data) {
            eprintln!("pack: {e}");
            return 1;
        }
    }
    let payload = builder.finish(&orki_pack::AppMeta::new(app_id, app_name, app_version));
    let mut final_bytes = stub_bytes;
    final_bytes.extend_from_slice(&payload);
    if let Err(e) = orki_pack::read_manifest(&final_bytes)
        .and_then(|m| orki_pack::verify_integrity(&final_bytes, &m))
    {
        eprintln!("pack: self-check failed: {e}");
        return 1;
    }
    if let Err(e) = std::fs::write(&out_path, &final_bytes) {
        eprintln!("pack: cannot write {}: {e}", out_path.display());
        return 1;
    }
    let ratio = if raw_total > 0 {
        payload.len() as f64 / raw_total as f64
    } else {
        1.0
    };
    let mode = if stub.is_some() {
        format!("stub attached, {} bytes total", final_bytes.len())
    } else {
        format!("{} bytes", final_bytes.len())
    };
    println!(
        "wrote {} ({} files, {raw_total} bytes raw, ratio {ratio:.2}, {mode})",
        out_path.display(),
        files.len(),
    );
    0
}

fn collect_files(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(String, PathBuf)>,
) -> Result<(), std::io::Error> {
    let mut entries: Vec<std::fs::DirEntry> = std::fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        if p.is_dir() {
            collect_files(root, &p, out)?;
        } else if p.is_file() {
            let rel = p
                .strip_prefix(root)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/");
            out.push((rel, p));
        }
    }
    Ok(())
}

fn cmd_preview(rest: &[String]) -> i32 {
    let positional: Vec<&String> = rest.iter().filter(|a| !a.starts_with('-')).collect();
    let Some(source) = positional.first() else {
        eprintln!("preview: package path required");
        return 2;
    };
    let data = match std::fs::read(source) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("preview: cannot read {source}: {e}");
            return 1;
        }
    };
    let m = match orki_pack::read_manifest(&data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("preview: bad package: {e}");
            return 1;
        }
    };
    let raw: u64 = m.files.iter().map(|f| f.size).sum();
    let packed: u64 = m.chunks.iter().map(|c| c.comp_len as u64).sum();
    println!("package: {} {} ({})", m.app_name, m.app_version, m.app_id);
    println!("files: {}, chunks: {}", m.files.len(), m.chunks.len());
    let mut codec_counts: BTreeMap<u8, u32> = BTreeMap::new();
    for c in &m.chunks {
        *codec_counts.entry(c.codec).or_insert(0) += 1;
    }
    for (codec, n) in &codec_counts {
        println!("codec {}: {} chunks", codec_name(*codec), n);
    }
    let ratio = if raw > 0 {
        packed as f64 / raw as f64
    } else {
        1.0
    };
    println!("raw: {raw} bytes, packed: {packed} bytes, ratio: {ratio:.2}");
    println!();
    for f in &m.files {
        println!("  {:>12}  {} ({} chunks)", f.size, f.path, f.chunk_count);
    }
    0
}

fn cmd_inspect(rest: &[String]) -> i32 {
    let positional: Vec<&String> = rest.iter().filter(|a| !a.starts_with('-')).collect();
    let Some(source) = positional.first() else {
        eprintln!("inspect: package path required");
        return 2;
    };
    let data = match std::fs::read(source) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("inspect: cannot read {source}: {e}");
            return 1;
        }
    };
    let m = match orki_pack::read_manifest(&data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("inspect: FAIL manifest: {e}");
            return 1;
        }
    };
    let (overlay, header) = match orki_pack::locate(&data) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("inspect: FAIL header: {e}");
            return 1;
        }
    };
    match orki_pack::verify_integrity(&data, &m) {
        Ok(report) => {
            let signed = header.flags & orki_pack::FLAG_SIGNED != 0;
            println!(
                "format: {}, overlay: {}, manifest: ok, signed: {}",
                header.format_version, overlay, signed
            );
            println!("app: {} {} ({})", m.app_name, m.app_version, m.app_id);
            println!("files: {}, chunks: {}", m.files.len(), m.chunks.len());
            println!(
                "integrity: ok ({} chunks, {} bytes verified)",
                report.chunks_checked, report.bytes_checked
            );
            0
        }
        Err(e) => {
            eprintln!("inspect: FAIL integrity: {e}");
            1
        }
    }
}

fn codec_name(codec: u8) -> &'static str {
    match codec {
        orki_pack::CODEC_STORE => "store",
        orki_pack::CODEC_LZMA2 => "lzma2",
        orki_pack::CODEC_BROTLI => "brotli",
        orki_pack::CODEC_ZSTD => "zstd",
        _ => "unknown",
    }
}

fn cmd_init(rest: &[String]) -> i32 {
    if let Some(pos) = rest.iter().position(|a| a == "--tauri") {
        let Some(path) = rest.get(pos + 1) else {
            eprintln!("init: --tauri requires a path to tauri.conf.json");
            return 2;
        };
        return init_from_tauri(Path::new(path));
    }
    let content = default_template();
    write_orki_toml(&content)
}

fn init_from_tauri(path: &Path) -> i32 {
    let json = match std::fs::read_to_string(path) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("init: cannot read {}: {e}", path.display());
            return 1;
        }
    };
    match orki_tauri::import_from_str(&json) {
        Ok(r) => {
            for w in &r.warnings {
                eprintln!("warning: {w}");
            }
            write_orki_toml(&r.orki_toml)
        }
        Err(e) => {
            eprintln!("init: {e}");
            1
        }
    }
}

fn write_orki_toml(content: &str) -> i32 {
    let path = Path::new("orki.toml");
    if path.exists() {
        eprintln!("init: orki.toml already exists");
        return 1;
    }
    if let Err(e) = std::fs::write(path, content) {
        eprintln!("init: {e}");
        return 1;
    }
    println!("wrote orki.toml");
    0
}

fn cmd_check(rest: &[String]) -> i32 {
    let path = rest
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("orki.toml"));
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("check: cannot read {}: {e}", path.display());
            return 1;
        }
    };
    match orki_core::manifest::Manifest::parse(&text) {
        Ok(m) => {
            println!("ok: {} {} ({})", m.app.name, m.app.version, m.app.id);
            println!(
                "components: {}, shortcuts: {}, registry: {}",
                m.components.len(),
                m.shortcuts.len(),
                m.registry.len()
            );
            0
        }
        Err(e) => {
            eprintln!("check: {e}");
            1
        }
    }
}

fn cmd_schema(_rest: &[String]) -> i32 {
    print!("{}", orki_schema::manifest_schema_json());
    0
}

fn cmd_doctor() -> i32 {
    for (bin, args) in [
        ("cargo", vec!["--version"]),
        ("rustc", vec!["--version"]),
        ("signtool", vec!["/?"]),
    ] {
        let ok = std::process::Command::new(bin)
            .args(&args)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        println!("{bin}: {}", if ok { "ok" } else { "missing" });
    }
    0
}

fn default_template() -> String {
    r#"schema = 1

[app]
id = "com.example.app"
name = "My App"
version = "0.1.0"

[package]
source = "target/release/my-app.exe"
compression = "lzma2"
stub = "full"

[install]
scope = "auto"
on_existing = "upgrade"
"#
    .to_string()
}
