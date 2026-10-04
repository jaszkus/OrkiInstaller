mod pe;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::process::exit;

const VARIANTS: [&str; 3] = ["full", "lite", "headless"];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = run(&args);
    exit(code);
}

fn run(args: &[String]) -> i32 {
    let Some(cmd) = args.first().map(String::as_str) else {
        print_help();
        return 2;
    };
    let rest = &args[1..];
    match cmd {
        "fmt" => cmd_fmt(rest),
        "check" => cmd_check(rest),
        "lint" => cmd_lint(rest),
        "test" => cmd_test(rest),
        "build" => cmd_build(rest),
        "size-budget" => cmd_size_budget(rest),
        "audit-imports" => cmd_audit_imports(rest),
        "e2e" => cmd_e2e(rest),
        "ci" => cmd_ci(rest),
        "doctor" => cmd_doctor(rest),
        "help" | "--help" | "-h" => {
            print_help();
            0
        }
        other => {
            eprintln!("unknown command: {other}");
            print_help();
            2
        }
    }
}

fn print_help() {
    println!("xtask — one source of truth for CI and local steps");
    println!();
    println!("  fmt [--check]");
    println!("  check [--fast]");
    println!("  lint");
    println!("  test [--fast]");
    println!(
        "  build --variant <full|lite|headless> [--profile <dev|ci|release>] [--target <triple>]"
    );
    println!("  size-budget --variant <V> [--profile <P>] [--target <T>]");
    println!("  audit-imports --variant <V> [--profile <P>] [--target <T>]");
    println!("  e2e");
    println!("  ci");
    println!("  doctor");
}

fn cmd_fmt(rest: &[String]) -> i32 {
    if has_flag(rest, "--check") {
        run_cargo(&["fmt", "--all", "--", "--check"])
    } else {
        run_cargo(&["fmt", "--all"])
    }
}

fn cmd_check(rest: &[String]) -> i32 {
    let fast = has_flag(rest, "--fast");
    if !fast && cmd_fmt(&["--check".to_string()]) != 0 {
        return 1;
    }
    if fast {
        run_cargo(&["check", "--workspace"])
    } else {
        run_cargo(&["check", "--workspace", "--all-targets"])
    }
}

fn cmd_lint(_rest: &[String]) -> i32 {
    if cmd_fmt(&["--check".to_string()]) != 0 {
        return 1;
    }
    if run_cargo(&[
        "clippy",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ]) != 0
    {
        return 1;
    }
    if tool_available("cargo", &["deny", "--version"]) {
        if run_cargo(&["deny", "check"]) != 0 {
            return 1;
        }
    } else {
        println!("cargo-deny not installed, skipping (CI installs it)");
    }
    0
}

fn cmd_test(rest: &[String]) -> i32 {
    let fast = has_flag(rest, "--fast");
    if tool_available("cargo", &["nextest", "--version"]) {
        let mut args: Vec<&str> = vec!["nextest", "run", "--workspace"];
        if fast {
            args.extend(["--lib", "--bins", "--tests"]);
        }
        run_cargo(&args)
    } else {
        let mut args: Vec<&str> = vec!["test", "--workspace", "--quiet"];
        if fast {
            args.extend(["--lib", "--bins", "--tests"]);
        }
        run_cargo(&args)
    }
}

fn cmd_build(rest: &[String]) -> i32 {
    let Some(variant) = flag_value(rest, "--variant") else {
        eprintln!("--variant <full|lite|headless> is required");
        return 2;
    };
    if !VARIANTS.contains(&variant.as_str()) {
        eprintln!("unknown variant: {variant}");
        return 2;
    }
    let profile = flag_value(rest, "--profile").unwrap_or_else(|| "dev".to_string());
    let target = flag_value(rest, "--target");
    let mut args = vec![
        "build".to_string(),
        "-p".to_string(),
        "orki-stub".to_string(),
        "--no-default-features".to_string(),
        "--features".to_string(),
        variant.clone(),
        "--profile".to_string(),
        profile.clone(),
    ];
    if let Some(t) = &target {
        args.push("--target".to_string());
        args.push(t.clone());
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let code = run_cargo(&refs);
    if code == 0 {
        println!(
            "artifact: {}",
            artifact_path(&profile, target.as_deref()).display()
        );
    }
    code
}

fn cmd_size_budget(rest: &[String]) -> i32 {
    let Some(variant) = flag_value(rest, "--variant") else {
        eprintln!("--variant is required");
        return 2;
    };
    let profile = flag_value(rest, "--profile").unwrap_or_else(|| "dev".to_string());
    let target = flag_value(rest, "--target");
    let budgets = match parse_budgets("xtask/budgets.toml") {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let Some(max_mib) = budgets.get(&variant).copied() else {
        eprintln!("no budget for variant: {variant}");
        return 2;
    };
    let path = artifact_path(&profile, target.as_deref());
    if !path.exists() {
        eprintln!("artifact not found: {} (build first)", path.display());
        return 1;
    }
    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let mib = size as f64 / (1024.0 * 1024.0);
    println!("stub {variant}: {mib:.2} MiB, budget: {max_mib} MiB");
    if mib > max_mib as f64 {
        eprintln!("size budget exceeded for {variant}");
        return 1;
    }
    0
}

fn cmd_audit_imports(rest: &[String]) -> i32 {
    let Some(variant) = flag_value(rest, "--variant") else {
        eprintln!("--variant is required");
        return 2;
    };
    let profile = flag_value(rest, "--profile").unwrap_or_else(|| "dev".to_string());
    let target = flag_value(rest, "--target");
    let path = artifact_path(&profile, target.as_deref());
    if !path.exists() {
        eprintln!("artifact not found: {} (build first)", path.display());
        return 1;
    }
    let data = match std::fs::read(&path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("cannot read {}: {e}", path.display());
            return 1;
        }
    };
    let dlls = match pe::import_dlls(&data) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("import table audit failed: {e}");
            return 1;
        }
    };
    if dlls.is_empty() {
        println!("audit-imports {}: no static imports found", path.display());
        return 0;
    }
    let mut failed = false;
    for dll in &dlls {
        let name = dll.to_ascii_lowercase();
        if is_denied(&name) {
            eprintln!("FORBIDDEN import: {name}");
            failed = true;
        } else if !is_allowed(&name) {
            eprintln!("unknown import, extend the allowlist or fix the build: {name}");
            failed = true;
        } else {
            println!("ok: {name}");
        }
    }
    if failed {
        eprintln!("import table audit failed for {variant}");
        return 1;
    }
    println!(
        "audit-imports {}: {} imports, all allowed",
        path.display(),
        dlls.len()
    );
    0
}

fn cmd_e2e(_rest: &[String]) -> i32 {
    let build_args = [
        "--variant".to_string(),
        "full".to_string(),
        "--profile".to_string(),
        "ci".to_string(),
    ];
    if cmd_build(&build_args) != 0 {
        return 1;
    }
    let stub = artifact_path("ci", None);
    if !stub.exists() {
        eprintln!("stub not found: {}", stub.display());
        return 1;
    }
    match Command::new(&stub).arg("--version").output() {
        Ok(o) if o.status.success() => {
            println!(
                "version smoke: ok ({})",
                String::from_utf8_lossy(&o.stdout).trim()
            );
        }
        _ => {
            eprintln!("version smoke failed");
            return 1;
        }
    }
    match Command::new(&stub).arg("--list").output() {
        Ok(o) if o.status.code() == Some(1620) => {
            println!("list smoke: ok (bad package exit code 1620 as expected)");
        }
        _ => {
            eprintln!("list smoke failed: expected exit code 1620");
            return 1;
        }
    }
    println!("e2e smoke passed");
    0
}

fn cmd_ci(_rest: &[String]) -> i32 {
    if cmd_lint(&[]) != 0 {
        return 1;
    }
    if cmd_test(&[]) != 0 {
        return 1;
    }
    for variant in VARIANTS {
        let a: Vec<String> = vec![
            "--variant".to_string(),
            variant.to_string(),
            "--profile".to_string(),
            "ci".to_string(),
        ];
        if cmd_build(&a) != 0 {
            return 1;
        }
        if cmd_size_budget(&a) != 0 {
            return 1;
        }
        if cmd_audit_imports(&a) != 0 {
            return 1;
        }
    }
    if cmd_e2e(&[]) != 0 {
        return 1;
    }
    println!("ci passed");
    0
}

fn cmd_doctor(_rest: &[String]) -> i32 {
    let mut ok = true;
    for (bin, args) in [
        ("cargo", ["--version"]),
        ("rustc", ["--version"]),
        ("git", ["--version"]),
        ("gh", ["--version"]),
    ] {
        let available = tool_available(bin, &args);
        println!("{bin}: {}", if available { "ok" } else { "missing" });
        ok &= available;
    }
    println!(
        "cargo-deny: {}",
        yes_no(tool_available("cargo", &["deny", "--version"]))
    );
    println!(
        "nextest: {}",
        yes_no(tool_available("cargo", &["nextest", "--version"]))
    );
    if ok { 0 } else { 1 }
}

fn yes_no(v: bool) -> &'static str {
    if v { "ok" } else { "missing" }
}

const ALLOWED_DLLS: [&str; 27] = [
    "kernel32.dll",
    "user32.dll",
    "gdi32.dll",
    "shell32.dll",
    "ole32.dll",
    "oleaut32.dll",
    "advapi32.dll",
    "ntdll.dll",
    "ws2_32.dll",
    "bcrypt.dll",
    "bcryptprimitives.dll",
    "userenv.dll",
    "dbghelp.dll",
    "sync.dll",
    "d3d12.dll",
    "dxgi.dll",
    "d3dcompiler_47.dll",
    "winhttp.dll",
    "vulkan-1.dll",
    "opengl32.dll",
    "imm32.dll",
    "comdlg32.dll",
    "shlwapi.dll",
    "propsys.dll",
    "uxtheme.dll",
    "dwmapi.dll",
    "wintrust.dll",
];

fn is_allowed(name: &str) -> bool {
    if ALLOWED_DLLS.contains(&name) {
        return true;
    }
    name.starts_with("api-ms-win-") || name.starts_with("ext-ms-")
}

fn is_denied(name: &str) -> bool {
    [
        "vcruntime",
        "msvcp",
        "msvcrt",
        "ucrtbase",
        "api-ms-win-crt-",
    ]
    .iter()
    .any(|p| name.starts_with(p))
}

fn artifact_path(profile: &str, target: Option<&str>) -> PathBuf {
    let dir = match profile {
        "dev" => "debug",
        p => p,
    };
    let mut p = PathBuf::from("target");
    if let Some(t) = target {
        p.push(t);
    }
    p.push(dir);
    p.push(if cfg!(windows) {
        "orki-stub.exe"
    } else {
        "orki-stub"
    });
    p
}

fn parse_budgets(path: &str) -> Result<BTreeMap<String, u64>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let mut map = BTreeMap::new();
    let mut section: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = Some(name.to_string());
        } else if let Some(sec) = &section
            && let Some((k, v)) = line.split_once('=')
            && k.trim() == "max_mib"
        {
            let n: u64 = v
                .trim()
                .parse()
                .map_err(|e| format!("bad max_mib in [{sec}]: {e}"))?;
            map.insert(sec.clone(), n);
        }
    }
    Ok(map)
}

fn run_cargo(args: &[&str]) -> i32 {
    println!("> cargo {}", args.join(" "));
    match Command::new("cargo").args(args).status() {
        Ok(s) if s.success() => 0,
        Ok(s) => s.code().unwrap_or(1),
        Err(e) => {
            eprintln!("failed to run cargo: {e}");
            1
        }
    }
}

fn tool_available(bin: &str, args: &[&str]) -> bool {
    Command::new(bin)
        .args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn has_flag(rest: &[String], name: &str) -> bool {
    rest.iter().any(|a| a == name)
}

fn flag_value(rest: &[String], name: &str) -> Option<String> {
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        if a == name {
            return it.next().cloned();
        }
        let prefix = format!("{name}=");
        if let Some(v) = a.strip_prefix(prefix.as_str()) {
            return Some(v.to_string());
        }
    }
    None
}
