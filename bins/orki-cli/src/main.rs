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
        "pack" | "preview" | "sign" | "inspect" | "diff" | "test" | "release" => {
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
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-o" | "--out" => out = it.next().cloned(),
            "--name" => name = it.next().cloned(),
            "--id" => id = it.next().cloned(),
            "--version" => version = it.next().cloned(),
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

    let mut builder = orki_pack::PackBuilder::new(0);
    let dest_name = format!("{stem}.exe");
    if let Err(e) = builder.add_file(&dest_name, &data) {
        eprintln!("wrap: {e}");
        return 1;
    }
    let payload = builder.finish(&orki_pack::AppMeta::new(app_id, app_name, app_version));
    let out_path = out
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("Setup.exe"));
    if let Err(e) = std::fs::write(&out_path, &payload) {
        eprintln!("wrap: cannot write {}: {e}", out_path.display());
        return 1;
    }
    println!(
        "wrote {} ({} bytes, payload only, no stub attached)",
        out_path.display(),
        payload.len()
    );
    0
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
